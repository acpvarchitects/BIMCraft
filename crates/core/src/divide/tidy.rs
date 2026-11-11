//! A piece's surface made of as few triangles as its faces need, for drawing it and for measuring it. Cutting leaves a
//! flat face in many triangles (a partition splits it by every plane it meets, mending cuts its edges) and some with no
//! area at all. Here the triangles that lie in one plane and touch along edges are one face again: its outline is
//! traced, the corners that only stand on a straight edge between the same two faces are left out, on both of them, and
//! the face is filled anew. The result is kept only if it is still closed and holds what the piece held; otherwise the
//! piece stays as it was.

use std::collections::HashMap;

use super::mesh::Soup;
use super::{DivideError, Work, cap, clip};
use crate::geom::P2;
use crate::view3d::{P3, cross, dot, sub, unit};

/// A triangle lower than this on its longest edge, mm, has no area worth a face.
const FLAT: f64 = 1e-6;
/// How far off a face's plane, mm, a corner still lies in it.
const IN_PLANE: f64 = 1e-6;

/// `soup` with each flat face filled anew from its outline; `soup` itself where that doesn't give the same solid.
pub(crate) fn tidy(soup: &Soup, work: &mut Work) -> Result<Soup, DivideError> {
    let n = soup.tris.len();
    work.spend(n * 8)?;
    let normals: Vec<Option<P3>> = soup
        .tris
        .iter()
        .map(|t| {
            let [a, b, c] = soup.corners(t);
            let across = cross(sub(b, a), sub(c, a));
            let longest = [sub(b, a), sub(c, b), sub(a, c)].iter().map(|e| dot(*e, *e).sqrt()).fold(0.0, f64::max);
            (longest > 0.0 && dot(across, across).sqrt() / longest >= FLAT).then(|| unit(across))
        })
        .collect();
    let normal = |i: usize| normals.get(i).copied().flatten();
    let mut by_edge: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, t) in soup.tris.iter().enumerate() {
        if normal(i).is_some() {
            for k in 0..3 {
                by_edge.entry((t[k], t[(k + 1) % 3])).or_default().push(i);
            }
        }
    }
    // The faces: from each triangle not yet in one, its neighbours across edges that lie in its plane.
    let mut face_of: Vec<usize> = vec![usize::MAX; n];
    let mut faces: Vec<(P3, Vec<usize>)> = Vec::new();
    for seed in 0..n {
        let Some(m) = normal(seed).filter(|_| face_of.get(seed) == Some(&usize::MAX)) else { continue };
        let Some(first) = soup.tris.get(seed) else { continue };
        let w = dot(m, soup.at(first[0]));
        let id = faces.len();
        let (mut members, mut stack) = (vec![seed], vec![seed]);
        if let Some(slot) = face_of.get_mut(seed) {
            *slot = id;
        }
        while let Some(i) = stack.pop() {
            let Some(t) = soup.tris.get(i) else { continue };
            for k in 0..3 {
                for &j in by_edge.get(&(t[(k + 1) % 3], t[k])).into_iter().flatten() {
                    let level = normal(j).is_some_and(|o| dot(o, m) >= 1.0 - 1e-9)
                        && soup.tris.get(j).is_some_and(|o| o.iter().all(|&v| (dot(m, soup.at(v)) - w).abs() <= IN_PLANE));
                    if level && face_of.get(j) == Some(&usize::MAX) {
                        if let Some(slot) = face_of.get_mut(j) {
                            *slot = id;
                        }
                        members.push(j);
                        stack.push(j);
                    }
                }
            }
        }
        members.sort_unstable();
        faces.push((m, members));
    }
    // Each face's rim: the edges of its triangles that no other of them runs back along.
    let mut rims: Vec<Vec<(u32, u32)>> = Vec::with_capacity(faces.len());
    let mut met: HashMap<u32, Vec<u32>> = HashMap::new();
    for (_, members) in &faces {
        let edges = || members.iter().filter_map(|&i| soup.tris.get(i)).flat_map(|t| (0..3).map(move |k| (t[k], t[(k + 1) % 3])));
        let mut balance: HashMap<(u32, u32), i32> = HashMap::new();
        for (a, b) in edges() {
            *balance.entry((a.min(b), a.max(b))).or_default() += if a < b { 1 } else { -1 };
        }
        let mut rim = Vec::new();
        for (a, b) in edges() {
            let Some(left) = balance.get_mut(&(a.min(b), a.max(b))) else { continue };
            if (a < b && *left > 0) || (a > b && *left < 0) {
                *left -= if a < b { 1 } else { -1 };
                rim.push((a, b));
                for (v, o) in [(a, b), (b, a)] {
                    let others = met.entry(v).or_default();
                    if !others.contains(&o) {
                        others.push(o);
                    }
                }
            }
        }
        rims.push(rim);
    }
    // A corner two rim edges meet at in a straight line says nothing: it stands on an edge between the same two faces.
    let idle = |v: u32| {
        let Some(&[a, b]) = met.get(&v).map(Vec::as_slice) else { return false };
        let (p, q) = (unit(sub(soup.at(a), soup.at(v))), unit(sub(soup.at(b), soup.at(v))));
        let off = cross(p, q);
        dot(p, q) < 0.0 && dot(off, off).sqrt() < 1e-7
    };
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(n);
    for ((m, members), rim) in faces.iter().zip(&rims) {
        work.spend(rim.len() + members.len())?;
        let keep = |tris: &mut Vec<[u32; 3]>| tris.extend(members.iter().filter_map(|&i| soup.tris.get(i)));
        // The face's corners, numbered for it, in its plane seen from outside.
        let (u, mut local, mut flat, mut global): (P3, HashMap<u32, u32>, Vec<P2>, Vec<u32>) =
            (clip::square_to(*m), HashMap::new(), Vec::new(), Vec::new());
        let v = cross(*m, u);
        let mut number = |i: u32| {
            *local.entry(i).or_insert_with(|| {
                let p = soup.at(i);
                flat.push(P2(dot(p, u), dot(p, v)));
                global.push(i);
                (flat.len() - 1) as u32
            })
        };
        let edges: Vec<(u32, u32)> = rim.iter().map(|&(a, b)| (number(a), number(b))).collect();
        let Some(rings) = cap::rings(&flat, &edges) else {
            keep(&mut tris);
            continue;
        };
        let rings: Vec<Vec<u32>> = rings
            .into_iter()
            .map(|r| r.into_iter().filter(|&i| !global.get(i as usize).is_some_and(|&g| idle(g))).collect::<Vec<u32>>())
            .collect();
        if rings.iter().any(|r| r.len() < 3) {
            keep(&mut tris);
            continue;
        }
        let filled = cap::fill(&flat, &rings, work)?;
        tris.extend(filled.iter().map(|t| t.map(|i| global.get(i as usize).copied().unwrap_or(0))));
    }
    let mut out = Soup { pts: soup.pts.clone(), tris }.packed();
    out.mend(work)?;
    let held = soup.volume();
    let same = out.is_closed() && (out.volume() - held).abs() <= 1e-9 * held.abs() + 1e-3;
    Ok(if same { out } else { soup.clone() })
}
