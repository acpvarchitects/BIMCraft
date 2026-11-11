//! A surface as corners and the triangles between them: corners within [`WELD`] made one, so an edge is the same two
//! numbers for both triangles that share it. What a surface holds, whether it is closed, the lumps it falls into, and
//! the mending of what a binary space partition leaves: a corner of one triangle on another's edge.

use std::collections::HashMap;

use super::{DivideError, Mesh, WELD, Work};
use crate::view3d::{P3, add, cross, dot, mul, sub, unit};

/// How far off an edge, mm, a corner still lies on it.
const ON_EDGE: f64 = 5e-5;
/// The side of the cells corners are looked for in, mm.
const CELL: f64 = 4.0 * WELD;

/// What a closed surface holds, mm³: positive while its triangles turn outward (counter-clockwise seen from outside).
pub fn volume(mesh: &[[P3; 3]]) -> f64 {
    let o = mesh.first().map_or([0.0; 3], |t| t[0]);
    mesh.iter().map(|[a, b, c]| dot(sub(*a, o), cross(sub(*b, o), sub(*c, o))) / 6.0).sum()
}

/// A surface's area, mm².
pub fn area(mesh: &[[P3; 3]]) -> f64 {
    mesh.iter().map(|[a, b, c]| length(cross(sub(*b, *a), sub(*c, *a))) / 2.0).sum()
}

/// The area, mm², of the triangles of `mesh` that look the way `normal` points, within `tolerance` degrees of it (use
/// [`super::FACING`], one degree, for flat faces): a part's main face, a wall's elevation or a floor's top, net of its
/// openings, of the holes cut in it and of what gaps and offsets took. Nothing for a normal with no length.
pub fn area_facing(mesh: &[[P3; 3]], normal: P3, tolerance: f64) -> f64 {
    let n = unit(normal);
    if dot(n, n) < 0.5 || !tolerance.is_finite() {
        return 0.0;
    }
    let least = tolerance.clamp(0.0, 180.0).to_radians().cos() - 1e-12;
    mesh.iter()
        .map(|[a, b, c]| {
            let c = cross(sub(*b, *a), sub(*c, *a));
            let twice = length(c);
            if twice > 0.0 && dot(c, n) / twice >= least { twice / 2.0 } else { 0.0 }
        })
        .sum()
}

/// Whether a surface is closed: with corners within [`WELD`] (a hundred-thousandth of a millimetre) made one, every
/// edge is shared by two triangles that run along it opposite ways (by four, two each way, where two parts of a solid
/// touch along an edge). A surface with no triangles is not.
pub fn is_closed(mesh: &[[P3; 3]]) -> bool {
    mesh.len() <= 4 * crate::csg::MAX_TRIANGLES && Soup::weld(mesh).is_closed()
}

fn length(v: P3) -> f64 {
    dot(v, v).sqrt()
}

/// Corners, and triangles as three of them each.
#[derive(Clone, Debug, Default)]
pub(crate) struct Soup {
    pub pts: Vec<P3>,
    pub tris: Vec<[u32; 3]>,
}

/// Corners found by the cell they are in.
#[derive(Default)]
pub(crate) struct Welder {
    pub pts: Vec<P3>,
    cells: HashMap<[i64; 3], Vec<u32>>,
}

impl Welder {
    fn cell(p: P3) -> [i64; 3] {
        p.map(|v| (v / CELL).floor() as i64)
    }

    /// The corner at `p`: the first within [`WELD`] of it, or a new one.
    pub fn add(&mut self, p: P3) -> u32 {
        let c = Welder::cell(p);
        let mut found: Option<u32> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(list) = self.cells.get(&[c[0].saturating_add(dx), c[1].saturating_add(dy), c[2].saturating_add(dz)]) else {
                        continue;
                    };
                    for &i in list {
                        let near = self.pts.get(i as usize).is_some_and(|q| length(sub(*q, p)) <= WELD);
                        if near && found.is_none_or(|f| i < f) {
                            found = Some(i);
                        }
                    }
                }
            }
        }
        found.unwrap_or_else(|| {
            let i = self.pts.len() as u32;
            self.pts.push(p);
            self.cells.entry(c).or_default().push(i);
            i
        })
    }
}

impl Soup {
    /// A surface with its corners made one where they are within [`WELD`]; a triangle left with two corners the same is
    /// left out.
    pub fn weld(mesh: &[[P3; 3]]) -> Soup {
        let mut welder = Welder::default();
        let mut tris = Vec::with_capacity(mesh.len());
        for t in mesh {
            let k = t.map(|p| welder.add(p));
            if k[0] != k[1] && k[1] != k[2] && k[2] != k[0] {
                tris.push(k);
            }
        }
        Soup { pts: welder.pts, tris }
    }

    pub fn at(&self, i: u32) -> P3 {
        self.pts.get(i as usize).copied().unwrap_or_default()
    }

    pub fn corners(&self, t: &[u32; 3]) -> [P3; 3] {
        t.map(|i| self.at(i))
    }

    pub fn mesh(&self) -> Mesh {
        self.tris.iter().map(|t| self.corners(t)).collect()
    }

    pub fn volume(&self) -> f64 {
        let o = self.tris.first().map_or([0.0; 3], |t| self.at(t[0]));
        self.tris.iter().map(|t| self.corners(t)).map(|[a, b, c]| dot(sub(a, o), cross(sub(b, o), sub(c, o))) / 6.0).sum()
    }

    pub fn area(&self) -> f64 {
        self.tris.iter().map(|t| self.corners(t)).map(|[a, b, c]| length(cross(sub(b, a), sub(c, a))) / 2.0).sum()
    }

    /// The centre of what it holds; of its corners, when it holds nothing.
    pub fn centre(&self) -> P3 {
        let o = self.tris.first().map_or([0.0; 3], |t| self.at(t[0]));
        let (mut sum, mut whole) = ([0.0; 3], 0.0);
        for [a, b, c] in self.tris.iter().map(|t| self.corners(t)) {
            let (a, b, c) = (sub(a, o), sub(b, o), sub(c, o));
            let v = dot(a, cross(b, c)) / 6.0;
            sum = add(sum, mul(add(a, add(b, c)), v / 4.0));
            whole += v;
        }
        if whole.abs() > 1e-9 {
            return add(o, mul(sum, 1.0 / whole));
        }
        let n = self.pts.len().max(1) as f64;
        mul(self.pts.iter().fold([0.0; 3], |s, p| add(s, *p)), 1.0 / n)
    }

    /// The edges that lack their twin: `(a, b)` is an edge some triangle runs along from `a` to `b` with none running
    /// back. In order.
    pub fn open_edges(&self) -> Vec<(u32, u32)> {
        let mut balance: HashMap<(u32, u32), i32> = HashMap::new();
        for t in &self.tris {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                *balance.entry((a.min(b), a.max(b))).or_default() += if a < b { 1 } else { -1 };
            }
        }
        let mut open = Vec::new();
        for (&(lo, hi), &n) in &balance {
            for _ in 0..n.unsigned_abs() {
                open.push(if n > 0 { (lo, hi) } else { (hi, lo) });
            }
        }
        open.sort_unstable();
        open
    }

    pub fn is_closed(&self) -> bool {
        !self.tris.is_empty() && self.open_edges().is_empty()
    }

    /// The seams a partition leaves closed: where a corner lies on a triangle's edge that lacks its twin, the triangle
    /// is cut there, into as many as the corners on it make. Again while that mends anything; then the pinholes.
    pub fn mend(&mut self, work: &mut Work) -> Result<(), DivideError> {
        for _ in 0..16 {
            let mut open = self.open_edges();
            open.dedup();
            if open.is_empty() {
                return Ok(());
            }
            let mut ends: Vec<u32> = open.iter().flat_map(|&(a, b)| [a, b]).collect();
            ends.sort_unstable();
            ends.dedup();
            work.spend(open.len().saturating_mul(ends.len()))?;
            let mut cuts: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
            for &(a, b) in &open {
                let (pa, pb) = (self.at(a), self.at(b));
                let d = sub(pb, pa);
                let len2 = dot(d, d);
                if len2 < WELD * WELD {
                    continue;
                }
                let mut on: Vec<(f64, u32)> = ends
                    .iter()
                    .filter(|&&v| v != a && v != b)
                    .filter_map(|&v| {
                        let p = self.at(v);
                        let t = dot(sub(p, pa), d) / len2;
                        let off = length(sub(p, add(pa, mul(d, t))));
                        (t > 0.0 && t < 1.0 && off <= ON_EDGE).then_some((t, v))
                    })
                    .collect();
                if !on.is_empty() {
                    on.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
                    cuts.insert((a, b), on.into_iter().map(|(_, v)| v).collect());
                }
            }
            if cuts.is_empty() {
                break;
            }
            let mut tris = Vec::with_capacity(self.tris.len() + cuts.len());
            for t in &self.tris {
                let hit = (0..3).find_map(|k| cuts.get(&(t[k], t[(k + 1) % 3])).map(|on| (k, on)));
                let Some((k, on)) = hit else {
                    tris.push(*t);
                    continue;
                };
                let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                let mut from = a;
                for &to in on.iter().chain(std::iter::once(&b)) {
                    if to != c && from != c && to != from {
                        tris.push([from, to, c]);
                    }
                    from = to;
                }
            }
            self.tris = tris;
        }
        self.plug(work)
    }

    /// The pinholes left: a partition drops a triangle too small to have a plane, and where no corner lies on its
    /// edges nothing mends that. The open edges are chained into rings, and a ring that bounds next to no area (a
    /// ten-thousandth of a square millimetre) is closed with triangles of its own corners. A larger hole stays: the
    /// surface is then not closed, and says so.
    fn plug(&mut self, work: &mut Work) -> Result<(), DivideError> {
        let open = self.open_edges();
        if open.is_empty() {
            return Ok(());
        }
        work.spend(open.len() * 4)?;
        let mut used = vec![false; open.len()];
        for start in 0..open.len() {
            if used.get(start).copied().unwrap_or(true) {
                continue;
            }
            let Some(&(first, _)) = open.get(start) else { continue };
            let (mut ring, mut e, mut whole) = (Vec::new(), start, false);
            while ring.len() <= open.len() {
                let Some(&(a, b)) = open.get(e) else { break };
                if let Some(slot) = used.get_mut(e) {
                    *slot = true;
                }
                ring.push(a);
                if b == first {
                    whole = true;
                    break;
                }
                let from = open.partition_point(|x| x.0 < b);
                let next = (from..open.len()).take_while(|&k| open.get(k).is_some_and(|x| x.0 == b)).find(|&k| used.get(k) == Some(&false));
                match next {
                    Some(k) => e = k,
                    None => break,
                }
            }
            if !whole || ring.len() < 3 {
                continue;
            }
            // The missing triangles run against the open edges: a fan from the ring's first corner, turned round.
            let fan: Vec<[u32; 3]> = (1..ring.len() - 1).map(|i| [ring[0], ring[i + 1], ring[i]]).collect();
            let size: f64 = fan.iter().map(|t| self.corners(t)).map(|[a, b, c]| length(cross(sub(b, a), sub(c, a))) / 2.0).sum();
            if size <= 1e-4 {
                self.tris.extend(fan);
            }
        }
        Ok(())
    }

    /// Its triangles by the corners they share, each group in the order of its first triangle.
    fn shells(&self) -> Vec<Vec<[u32; 3]>> {
        let mut root: Vec<u32> = (0..self.pts.len() as u32).collect();
        fn find(root: &mut [u32], mut x: u32) -> u32 {
            while let Some(&up) = root.get(x as usize).filter(|&&up| up != x) {
                let over = root.get(up as usize).copied().unwrap_or(up);
                if let Some(slot) = root.get_mut(x as usize) {
                    *slot = over;
                }
                x = up;
            }
            x
        }
        for t in &self.tris {
            let a = find(&mut root, t[0]);
            for k in [t[1], t[2]] {
                let b = find(&mut root, k);
                if let Some(slot) = root.get_mut(a.max(b) as usize) {
                    *slot = a.min(b);
                }
            }
        }
        let mut place: HashMap<u32, usize> = HashMap::new();
        let mut shells: Vec<Vec<[u32; 3]>> = Vec::new();
        for t in &self.tris {
            let r = find(&mut root, t[0]);
            let k = *place.entry(r).or_insert_with(|| {
                shells.push(Vec::new());
                shells.len() - 1
            });
            if let Some(shell) = shells.get_mut(k) {
                shell.push(*t);
            }
        }
        shells
    }

    /// The lumps it falls into: each a surface whose triangles share corners, with the hollows inside it (surfaces that
    /// hold less than nothing go to the smallest lump round them).
    pub fn lumps(&self, work: &mut Work) -> Result<Vec<Soup>, DivideError> {
        let shells = self.shells();
        if shells.len() <= 1 {
            return Ok(if self.tris.is_empty() { Vec::new() } else { vec![self.clone()] });
        }
        let of = |tris: &[[u32; 3]]| Soup { pts: self.pts.clone(), tris: tris.to_vec() };
        let held: Vec<f64> = shells.iter().map(|s| of(s).volume()).collect();
        let mut lumps: Vec<(f64, Vec<[u32; 3]>)> = Vec::new();
        let mut homes: Vec<usize> = Vec::new();
        for (s, &v) in shells.iter().zip(&held) {
            if v > 0.0 {
                homes.push(lumps.len());
                lumps.push((v, s.clone()));
            } else {
                homes.push(usize::MAX);
            }
        }
        for (s, &v) in shells.iter().zip(&held) {
            let Some(probe) = s.first().map(|t| self.at(t[0])).filter(|_| v < 0.0) else { continue };
            let mut home: Option<usize> = None;
            for (k, (h, &hv)) in shells.iter().zip(&held).enumerate() {
                work.spend(h.len())?;
                let better = home.and_then(|j| held.get(j)).is_none_or(|&best| hv < best);
                if hv > 0.0 && better && self.holds(h, probe) {
                    home = Some(k);
                }
            }
            if let Some(lump) = home.and_then(|k| homes.get(k)).and_then(|&k| lumps.get_mut(k)) {
                lump.1.extend(s.iter().copied());
            }
        }
        Ok(lumps.into_iter().map(|(_, tris)| of(&tris).packed()).collect())
    }

    /// Whether `p` is inside the closed surface `tris`: a ray from it crosses the surface an odd number of times.
    fn holds(&self, tris: &[[u32; 3]], p: P3) -> bool {
        // A direction no face of a building runs along.
        let d = unit([0.531_726_4, 0.217_934_1, 0.818_392_7]);
        let mut crossed = 0;
        for [a, b, c] in tris.iter().map(|t| self.corners(t)) {
            let (e1, e2) = (sub(b, a), sub(c, a));
            let h = cross(d, e2);
            let det = dot(e1, h);
            if det.abs() < 1e-12 {
                continue;
            }
            let s = sub(p, a);
            let u = dot(s, h) / det;
            let q = cross(s, e1);
            let v = dot(d, q) / det;
            if u >= 0.0 && v >= 0.0 && u + v <= 1.0 && dot(e2, q) / det > 1e-9 {
                crossed += 1;
            }
        }
        crossed % 2 == 1
    }

    /// Several surfaces as one.
    pub fn join(parts: &[Soup]) -> Soup {
        let mut out = Soup::default();
        for part in parts {
            let from = out.pts.len() as u32;
            out.pts.extend_from_slice(&part.pts);
            out.tris.extend(part.tris.iter().map(|t| t.map(|i| i + from)));
        }
        out
    }

    /// The same surface with only the corners its triangles use.
    pub fn packed(&self) -> Soup {
        let mut new: Vec<u32> = vec![u32::MAX; self.pts.len()];
        let mut out = Soup::default();
        for t in &self.tris {
            let k = t.map(|i| {
                let slot = new.get(i as usize).copied().unwrap_or(u32::MAX);
                if slot != u32::MAX {
                    return slot;
                }
                let n = out.pts.len() as u32;
                out.pts.push(self.at(i));
                if let Some(s) = new.get_mut(i as usize) {
                    *s = n;
                }
                n
            });
            out.tris.push(k);
        }
        out
    }
}
