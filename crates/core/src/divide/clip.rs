//! A solid cut by a plane: each triangle clipped on its own, and each side closed with the section filled in. A corner
//! within [`WELD`] of the plane lies on it; where an edge crosses the plane the new corner is made once, for the edge,
//! so the two triangles that share the edge share the corner. The cap of a side is made from the edges its triangles
//! leave open, so it fits them whatever lay in the plane already (a face of the solid there goes to the side it faces
//! away from).

use std::collections::HashMap;

use super::mesh::Soup;
use super::{DivideError, WELD, Work, cap};
use crate::geom::P2;
use crate::view3d::{P3, add, cross, dot, mul, sub, unit};

/// A direction square to `n`: level where there is one.
pub(crate) fn square_to(n: P3) -> P3 {
    let level = cross([0.0, 0.0, 1.0], n);
    if dot(level, level) < 1e-12 { [1.0, 0.0, 0.0] } else { unit(level) }
}

/// `soup`, a closed surface, cut by the plane `n · p = w` (`n` of length one): what lies the way `n` points and the
/// rest, each closed; one of them empty where the plane passes it by.
pub(crate) fn split(soup: &Soup, n: P3, w: f64, work: &mut Work) -> Result<(Soup, Soup), DivideError> {
    work.spend(soup.pts.len() + soup.tris.len())?;
    let off: Vec<f64> = soup.pts.iter().map(|p| dot(n, *p) - w).collect();
    let side: Vec<i8> = off
        .iter()
        .map(|&d| {
            if d > WELD {
                1
            } else if d < -WELD {
                -1
            } else {
                0
            }
        })
        .collect();
    if !side.contains(&1) {
        return Ok((Soup::default(), soup.clone()));
    }
    if !side.contains(&-1) {
        return Ok((soup.clone(), Soup::default()));
    }
    let side_of = |i: u32| side.get(i as usize).copied().unwrap_or(0);
    let mut pts = soup.pts.clone();
    let mut made: HashMap<(u32, u32), u32> = HashMap::new();
    let (mut front, mut back): (Vec<[u32; 3]>, Vec<[u32; 3]>) = (Vec::new(), Vec::new());
    let fan = |v: &[u32], out: &mut Vec<[u32; 3]>| {
        for k in 1..v.len().saturating_sub(1) {
            let t = [v[0], v[k], v[k + 1]];
            if t[0] != t[1] && t[1] != t[2] && t[2] != t[0] {
                out.push(t);
            }
        }
    };
    for t in &soup.tris {
        let s = t.map(side_of);
        match (s.contains(&1), s.contains(&-1)) {
            (true, false) => front.push(*t),
            (false, true) => back.push(*t),
            (false, false) => {
                let [a, b, c] = soup.corners(t);
                if dot(cross(sub(b, a), sub(c, a)), n) > 0.0 { back.push(*t) } else { front.push(*t) }
            }
            (true, true) => {
                let (mut f, mut b) = (Vec::with_capacity(4), Vec::with_capacity(4));
                for k in 0..3 {
                    let (i, j, si, sj) = (t[k], t[(k + 1) % 3], s[k], s[(k + 1) % 3]);
                    if si >= 0 {
                        f.push(i);
                    }
                    if si <= 0 {
                        b.push(i);
                    }
                    if si * sj < 0 {
                        let (lo, hi) = (i.min(j), i.max(j));
                        let x = *made.entry((lo, hi)).or_insert_with(|| {
                            let (pa, pb) = (soup.at(lo), soup.at(hi));
                            let (da, db) = (off.get(lo as usize).copied().unwrap_or(0.0), off.get(hi as usize).copied().unwrap_or(0.0));
                            pts.push(add(pa, mul(sub(pb, pa), da / (da - db))));
                            (pts.len() - 1) as u32
                        });
                        f.push(x);
                        b.push(x);
                    }
                }
                fan(&f, &mut front);
                fan(&b, &mut back);
            }
        }
    }
    let on = |i: u32| side.get(i as usize).is_none_or(|&s| s == 0);
    let u = square_to(n);
    let mut close = |tris: Vec<[u32; 3]>, out: P3| -> Result<Soup, DivideError> {
        let mut side = Soup { pts: std::mem::take(&mut pts), tris };
        let open = side.open_edges();
        if open.iter().any(|&(a, b)| !on(a) || !on(b)) {
            return Err(not_closed());
        }
        // The cap looks the way `out` points: its plane's axes turn that way, and its edges run against the open ones.
        let v = cross(out, u);
        let flat: Vec<P2> = side.pts.iter().map(|p| P2(dot(*p, u), dot(*p, v))).collect();
        let edges: Vec<(u32, u32)> = open.iter().map(|&(a, b)| (b, a)).collect();
        work.spend(edges.len())?;
        let rings = cap::rings(&flat, &edges).ok_or_else(not_closed)?;
        side.tris.extend(cap::fill(&flat, &rings, work)?);
        pts = side.pts.clone();
        Ok(side.packed())
    };
    let ahead = close(front, mul(n, -1.0))?;
    let behind = close(back, n)?;
    Ok((ahead, behind))
}

pub(crate) fn not_closed() -> DivideError {
    DivideError::Input(
        "the solid isn't a closed surface: every edge must be shared by two triangles, each turned outward (counter-clockwise seen from outside)"
            .into(),
    )
}
