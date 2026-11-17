//! A part's solid, as triangles: the closed surface of the prisms it is made of, and what that surface measures.
//!
//! A host's solid is prisms (`walls::prisms`, `Model::floor_prisms`, `Model::roof_prisms`): rings in plan, each from a
//! bottom plane up to a top one, standing side by side where a door, a shaft or the roof a wall follows cuts the host.
//! [`mesh`] makes one closed surface of them: the faces two prisms share are left out, so nothing lies inside the solid,
//! and where one prism's corner lies on another's edge both get the corner, so every edge of every triangle is another
//! triangle's the other way round. That is what a solid cut by another asks for, what IFC calls a closed shell, and what
//! lets an area be read off its faces.

use std::collections::{BTreeMap, HashMap};

use crate::geom::{self, P2};
use crate::view3d::{P3, cross, dot, sub};
use crate::walls::Plane;

/// A piece of a solid: a ring in plan, from a bottom plane up to a top one.
#[derive(Clone, Debug, PartialEq)]
pub struct Prism {
    pub ring: Vec<P2>,
    pub bottom: Plane,
    pub top: Plane,
}

/// How near two points in plan, or two heights, are one, mm.
const WELD: f64 = 1e-6;

/// What welding may cost, in corners looked at: past it the prisms are meshed each on its own, which is still a solid,
/// with the faces they share inside it.
const MAX_WORK: usize = 20_000_000;

/// The corners of the prisms in plan, each once: its place, and the heights that meet at it.
#[derive(Default)]
struct Corners {
    at: Vec<P2>,
    cells: HashMap<(i64, i64), Vec<usize>>,
    heights: Vec<Vec<f64>>,
}

impl Corners {
    /// The corner at `q`: one within [`WELD`] of it, or a new one.
    fn index(&mut self, q: P2) -> usize {
        // Cells a thousand times the tolerance wide, so a corner near `q` is in its cell or one beside it.
        let cell = |v: f64| (v / (1e3 * WELD)).floor() as i64;
        let (cx, cy) = (cell(q.0), cell(q.1));
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(found) = self
                    .cells
                    .get(&(cx + dx, cy + dy))
                    .and_then(|c| c.iter().copied().find(|i| self.at.get(*i).is_some_and(|p| p.dist(q) <= WELD)))
                {
                    return found;
                }
            }
        }
        self.at.push(q);
        self.heights.push(Vec::new());
        self.cells.entry((cx, cy)).or_default().push(self.at.len() - 1);
        self.at.len() - 1
    }

    /// Height `z` noted at corner `i`.
    fn note(&mut self, i: usize, z: f64) {
        if let Some(h) = self.heights.get_mut(i)
            && !h.iter().any(|x| (x - z).abs() <= WELD)
        {
            h.push(z);
        }
    }

    fn point(&self, i: usize) -> P2 {
        self.at.get(i).copied().unwrap_or_default()
    }

    /// The heights noted at corner `i` from `lo` up to `hi`, those two among them, in order.
    fn between(&self, i: usize, lo: f64, hi: f64) -> Vec<f64> {
        let mut zs: Vec<f64> =
            self.heights.get(i).into_iter().flatten().copied().filter(|z| *z > lo + WELD && *z < hi - WELD).chain([lo, hi]).collect();
        zs.sort_by(f64::total_cmp);
        zs.dedup_by(|a, b| (*a - *b).abs() <= WELD);
        zs
    }
}

/// A plane between two corners of an edge: its heights at the edge's start and at its end.
type Line = (f64, f64);

/// The closed surface of `prisms`, which stand side by side and never overlap: triangles in model coordinates (mm, x
/// east, y north, z up), each counter-clockwise seen from outside the solid, so its normal points out. A prism with no
/// area in plan or no height anywhere is left out. None, no triangles.
pub fn mesh(prisms: &[Prism]) -> Vec<[P3; 3]> {
    let prisms: Vec<Prism> = prisms.iter().filter_map(usable).collect();
    welded(&prisms).unwrap_or_else(|| prisms.iter().flat_map(|p| alone(&p.ring, p)).collect())
}

/// `p` with its ring counter-clockwise and no point of it twice in a row; none for one that holds nothing.
fn usable(p: &Prism) -> Option<Prism> {
    let mut ring = p.ring.clone();
    ring.dedup_by(|a, b| a.dist(*b) <= WELD);
    if ring.len() > 1 && ring.first().zip(ring.last()).is_some_and(|(a, b)| a.dist(*b) <= WELD) {
        ring.pop();
    }
    let planes_ok = [p.bottom, p.top].iter().all(|pl| pl.z.is_finite() && pl.g.is_finite());
    if ring.len() < 3 || !planes_ok || !ring.iter().all(|q| q.is_finite()) || geom::area(&ring).abs() <= WELD {
        return None;
    }
    if geom::area(&ring) < 0.0 {
        ring.reverse();
    }
    ring.iter().any(|q| p.top.at(*q) - p.bottom.at(*q) > WELD).then_some(Prism { ring, ..p.clone() })
}

/// One prism's own closed surface: its top, its bottom and a side along each edge of `ring`, its ring with any corners
/// its neighbours put on its edges.
fn alone(ring: &[P2], p: &Prism) -> Vec<[P3; 3]> {
    let mut out = caps(ring, p);
    let at = |q: P2, plane: &Plane| [q.0, q.1, plane.at(q)];
    for (a, b) in geom::ring_edges(ring) {
        let (a0, b0, a1, b1) = (at(a, &p.bottom), at(b, &p.bottom), at(a, &p.top), at(b, &p.top));
        out.extend([[a0, b0, b1], [a0, b1, a1]].into_iter().filter(|t| area(t) > 0.0));
    }
    out
}

/// A prism's top and bottom over `ring`, counter-clockwise in plan: the top's triangles turned up, the bottom's down.
fn caps(ring: &[P2], p: &Prism) -> Vec<[P3; 3]> {
    let pt = |i: usize| ring.get(i).copied().unwrap_or_default();
    let at = |q: P2, plane: &Plane| [q.0, q.1, plane.at(q)];
    let mut out = Vec::new();
    for [a, b, c] in geom::triangulate(ring) {
        out.push([at(pt(a), &p.top), at(pt(b), &p.top), at(pt(c), &p.top)]);
        out.push([at(pt(a), &p.bottom), at(pt(c), &p.bottom), at(pt(b), &p.bottom)]);
    }
    out
}

/// [`mesh`], the faces the prisms share left out; none when that would cost too much.
fn welded(prisms: &[Prism]) -> Option<Vec<[P3; 3]>> {
    let mut corners = Corners::default();
    let rings: Vec<Vec<usize>> = prisms.iter().map(|p| p.ring.iter().map(|q| corners.index(*q)).collect()).collect();
    // Each corner in order of x, so the corners that may lie on an edge are those between its ends' x.
    let mut by_x: Vec<usize> = (0..corners.at.len()).collect();
    by_x.sort_by(|a, b| corners.point(*a).0.total_cmp(&corners.point(*b).0));
    let xs: Vec<f64> = by_x.iter().map(|i| corners.point(*i).0).collect();
    let mut work = 0usize;
    // Each ring with the corners of the others that lie on its edges: where two prisms meet, they share every corner.
    let mut refined: Vec<Vec<usize>> = Vec::with_capacity(rings.len());
    for ring in &rings {
        let mut out = Vec::with_capacity(ring.len());
        for (k, &a) in ring.iter().enumerate() {
            let b = ring.get((k + 1) % ring.len()).copied().unwrap_or(a);
            out.push(a);
            if a == b {
                continue;
            }
            let (pa, pb) = (corners.point(a), corners.point(b));
            let (lo, hi) = (pa.0.min(pb.0) - WELD, pa.0.max(pb.0) + WELD);
            let (from, to) = (xs.partition_point(|x| *x < lo), xs.partition_point(|x| *x <= hi));
            work += to - from;
            if work > MAX_WORK {
                return None;
            }
            let (d, len) = ((pb - pa).unit(), pa.dist(pb));
            let mut on: Vec<(f64, usize)> = by_x
                .get(from..to)
                .into_iter()
                .flatten()
                .filter(|i| **i != a && **i != b)
                .filter_map(|&i| {
                    let q = corners.point(i) - pa;
                    let along = q.dot(d);
                    (along > WELD && along < len - WELD && q.cross(d).abs() <= WELD).then_some((along, i))
                })
                .collect();
            on.sort_by(|x, y| x.0.total_cmp(&y.0));
            out.extend(on.into_iter().map(|(_, i)| i));
        }
        out.dedup();
        refined.push(out);
    }
    // The heights that meet at each corner: where a side's upright edge is cut, so its neighbour's ends are its own.
    for (ring, p) in refined.iter().zip(prisms) {
        for &i in ring {
            let q = corners.point(i);
            corners.note(i, p.bottom.at(q));
            corners.note(i, p.top.at(q));
        }
    }
    // Along each edge, one way, what stands there: the prisms on the side it is theirs.
    let mut along: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (k, ring) in refined.iter().enumerate() {
        for (j, &a) in ring.iter().enumerate() {
            let b = ring.get((j + 1) % ring.len()).copied().unwrap_or(a);
            along.entry((a, b)).or_default().push(k);
        }
    }
    let mut out = Vec::new();
    for (k, (ring, p)) in refined.iter().zip(prisms).enumerate() {
        let points: Vec<P2> = ring.iter().map(|i| corners.point(*i)).collect();
        let tops = caps(&points, p);
        // A ring the corners on its edges left no way to cut into triangles keeps its own corners on its top and bottom:
        // still closed to the eye, with a neighbour's corner on an edge of it.
        out.extend(if tops.len() == 2 * points.len().saturating_sub(2) { tops } else { caps(&p.ring, p) });
        for (j, &a) in ring.iter().enumerate() {
            let b = ring.get((j + 1) % ring.len()).copied().unwrap_or(a);
            let (pa, pb) = (corners.point(a), corners.point(b));
            let line = |plane: &Plane| (plane.at(pa), plane.at(pb));
            let mine = (line(&p.bottom), line(&p.top));
            let others: Vec<(Line, Line)> = along
                .get(&(b, a))
                .into_iter()
                .flatten()
                .filter(|n| **n != k)
                .filter_map(|n| prisms.get(*n))
                .map(|n| (line(&n.bottom), line(&n.top)))
                .collect();
            work += others.len() + 1;
            if work > MAX_WORK {
                return None;
            }
            for (lo, hi) in bare(mine, &others) {
                side(&mut out, &corners, (a, b), lo, hi);
            }
        }
    }
    Some(out)
}

/// What of a prism's side along an edge, from `mine.0` up to `mine.1`, no neighbour across the edge covers: strips, each
/// from one line up to another. Where the lines cross between the edge's ends there is no telling them apart by their
/// ends, and all of the side is given: a face inside the solid, which a solid may have.
fn bare(mine: (Line, Line), others: &[(Line, Line)]) -> Vec<(Line, Line)> {
    if others.is_empty() {
        return vec![mine];
    }
    let mid = |l: Line| (l.0 + l.1) / 2.0;
    let mut lines: Vec<Line> = [mine.0, mine.1].into_iter().chain(others.iter().flat_map(|(lo, hi)| [*lo, *hi])).collect();
    lines.sort_by(|a, b| mid(*a).total_cmp(&mid(*b)));
    lines.dedup_by(|a, b| (a.0 - b.0).abs() <= WELD && (a.1 - b.1).abs() <= WELD);
    if lines.windows(2).any(|w| matches!(w, [a, b] if b.0 < a.0 - WELD || b.1 < a.1 - WELD)) {
        return vec![mine];
    }
    let within = |(lo, hi): (Line, Line), (a, b): (Line, Line)| mid(lo) <= mid(a) + WELD && mid(b) <= mid(hi) + WELD;
    lines
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some((*a, *b)),
            _ => None,
        })
        .filter(|strip| within(mine, *strip) && !others.iter().any(|o| within(*o, *strip)))
        .collect()
}

/// The side along edge `a` to `b` from line `lo` up to line `hi`, its upright edges cut at every height that meets
/// them, turned out: to the right of the edge, which runs counter-clockwise round its prism.
fn side(out: &mut Vec<[P3; 3]>, corners: &Corners, (a, b): (usize, usize), lo: Line, hi: Line) {
    let (pa, pb) = (corners.point(a), corners.point(b));
    // Never upside down: where the lines cross at an end, it has no height there.
    let hi = (hi.0.max(lo.0), hi.1.max(lo.1));
    let left: Vec<P3> = corners.between(a, lo.0, hi.0).into_iter().map(|z| [pa.0, pa.1, z]).collect();
    let right: Vec<P3> = corners.between(b, lo.1, hi.1).into_iter().map(|z| [pb.0, pb.1, z]).collect();
    let (Some(&first), Some(&last)) = (left.first(), right.last()) else { return };
    // A fan from its bottom left corner up its right edge, and one from its top right corner up its left edge.
    let fans = right.windows(2).map(|w| [first, w[0], w[1]]).chain(left.windows(2).map(|w| [w[0], last, w[1]]));
    out.extend(fans.filter(|t| area(t) > 0.0));
}

/// A triangle's area, mm²: 0 for one with none worth a face.
fn area(t: &[P3; 3]) -> f64 {
    let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    let a = dot(n, n).sqrt() / 2.0;
    if a > WELD * WELD { a } else { 0.0 }
}

/// How much a closed surface of triangles holds, mm³: each triangle a tetrahedron with one point, signed by the way it
/// faces (the divergence theorem). The point is the surface's first corner, so far-off coordinates keep their precision.
pub fn volume(tris: &[[P3; 3]]) -> f64 {
    let Some(o) = tris.first().map(|t| t[0]) else { return 0.0 };
    tris.iter().map(|t| dot(sub(t[0], o), cross(sub(t[1], o), sub(t[2], o)))).sum::<f64>() / 6.0
}

/// The way a part's main face looks: what [`facing`] takes the area of.
#[derive(Clone, Debug, PartialEq)]
pub enum Facing {
    /// Along one of these unit vectors: a straight wall's exterior side, the top planes of a floor's or a roof's part.
    Along(Vec<P3>),
    /// Square to a curved wall, at each point of it: toward the arc's centre, or away from it.
    Round { centre: P2, inward: bool },
}

/// The area of the faces of `tris` that look the way `to` says, mm²: those square to it within a hundredth of a degree
/// (a curved wall's, cut into flat pieces, within the few degrees each piece turns).
pub fn facing(tris: &[[P3; 3]], to: &Facing) -> f64 {
    tris.iter()
        .filter_map(|t| {
            let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            let len = dot(n, n).sqrt();
            if len <= 2.0 * WELD * WELD {
                return None;
            }
            let n = [n[0] / len, n[1] / len, n[2] / len];
            let looks = match to {
                Facing::Along(ways) => ways.iter().any(|w| dot(n, *w) >= 1.0 - 1.5e-8),
                Facing::Round { centre, inward } => {
                    let mid = P2((t[0][0] + t[1][0] + t[2][0]) / 3.0, (t[0][1] + t[1][1] + t[2][1]) / 3.0);
                    let out = (mid - *centre).unit() * if *inward { -1.0 } else { 1.0 };
                    dot(n, [out.0, out.1, 0.0]) >= 0.995
                }
            };
            looks.then_some(len / 2.0)
        })
        .sum()
}

/// The unit vector square to `plane`, up.
pub fn normal(plane: &Plane) -> P3 {
    let len = (plane.g.0 * plane.g.0 + plane.g.1 * plane.g.1 + 1.0).sqrt();
    [-plane.g.0 / len, -plane.g.1 / len, 1.0 / len]
}

/// Whether `tris` is a closed surface: every edge of a triangle is another's the other way round, as often.
pub fn closed(tris: &[[P3; 3]]) -> bool {
    let key = |p: P3| p.map(|v| (v * 1e4).round() as i64);
    let mut edges: HashMap<([i64; 3], [i64; 3]), i64> = HashMap::new();
    for t in tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (key(a), key(b));
            if a == b {
                continue;
            }
            *edges.entry((a, b)).or_default() += 1;
            *edges.entry((b, a)).or_default() -= 1;
        }
    }
    !tris.is_empty() && edges.values().all(|n| *n == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<P2> {
        vec![P2(x0, y0), P2(x1, y0), P2(x1, y1), P2(x0, y1)]
    }

    fn block(ring: Vec<P2>, bottom: f64, top: f64) -> Prism {
        Prism { ring, bottom: Plane::flat(bottom), top: Plane::flat(top) }
    }

    /// The area of the triangles of `tris` whose normal is `way`.
    fn looking(tris: &[[P3; 3]], way: P3) -> f64 {
        facing(tris, &Facing::Along(vec![way]))
    }

    #[test]
    fn one_prism_is_a_closed_box_turned_out() {
        let m = mesh(&[block(rect(0.0, 0.0, 4000.0, 500.0), 0.0, 3000.0)]);
        assert_eq!(m.len(), 12);
        assert!(closed(&m));
        assert!((volume(&m) - 4000.0 * 500.0 * 3000.0).abs() < 1e-3, "{}", volume(&m));
        for (way, area) in [
            ([0.0, 0.0, 1.0], 4000.0 * 500.0),
            ([0.0, 0.0, -1.0], 4000.0 * 500.0),
            ([0.0, 1.0, 0.0], 4000.0 * 3000.0),
            ([0.0, -1.0, 0.0], 4000.0 * 3000.0),
            ([1.0, 0.0, 0.0], 500.0 * 3000.0),
            ([-1.0, 0.0, 0.0], 500.0 * 3000.0),
        ] {
            assert!((looking(&m, way) - area).abs() < 1e-6, "{way:?}: {}", looking(&m, way));
        }
        // Given clockwise, the same box.
        let mut ring = rect(0.0, 0.0, 4000.0, 500.0);
        ring.reverse();
        let turned = mesh(&[block(ring, 0.0, 3000.0)]);
        assert!(closed(&turned) && (volume(&turned) - volume(&m)).abs() < 1e-3);
    }

    #[test]
    fn prisms_side_by_side_are_one_solid_with_nothing_inside_it() {
        // A wall with a window: the full-height pieces either side, and the pieces under and over the window between.
        let wall = [
            block(rect(0.0, 0.0, 1500.0, 500.0), 0.0, 3000.0),
            block(rect(1500.0, 0.0, 2500.0, 500.0), 0.0, 900.0),
            block(rect(1500.0, 0.0, 2500.0, 500.0), 2100.0, 3000.0),
            block(rect(2500.0, 0.0, 4000.0, 500.0), 0.0, 3000.0),
        ];
        let m = mesh(&wall);
        assert!(closed(&m), "every edge has its twin");
        let net = 4000.0 * 3000.0 - 1000.0 * 1200.0;
        assert!((volume(&m) - net * 500.0).abs() < 1e-3, "{}", volume(&m));
        // Its faces: each side less the window, and the window's reveals, which look along the wall and up and down.
        assert!((looking(&m, [0.0, 1.0, 0.0]) - net).abs() < 1e-6);
        assert!((looking(&m, [0.0, -1.0, 0.0]) - net).abs() < 1e-6);
        assert!((looking(&m, [1.0, 0.0, 0.0]) - (500.0 * 3000.0 + 500.0 * 1200.0)).abs() < 1e-6, "an end and a jamb");
        assert!((looking(&m, [0.0, 0.0, 1.0]) - (4000.0 * 500.0 + 1000.0 * 500.0)).abs() < 1e-6, "the top and the sill");
        // Each on its own they'd hold the same, with the faces between them inside: more surface than the solid has.
        let apart: Vec<[P3; 3]> = wall.iter().flat_map(|p| alone(&p.ring, p)).collect();
        assert!((volume(&apart) - volume(&m)).abs() < 1e-3);
        assert!(looking(&apart, [1.0, 0.0, 0.0]) > looking(&m, [1.0, 0.0, 0.0]) + 1.0);
    }

    #[test]
    fn a_corner_on_a_neighbours_edge_is_the_neighbours_too() {
        // A floor in three pieces round an opening's corner: the long piece's edge has the other two's corner on it.
        let floor = [
            block(rect(0.0, 0.0, 6000.0, 2000.0), -200.0, 0.0),
            block(rect(0.0, 2000.0, 2500.0, 4000.0), -200.0, 0.0),
            block(rect(2500.0, 2000.0, 6000.0, 4000.0), -200.0, 0.0),
        ];
        let m = mesh(&floor);
        assert!(closed(&m));
        assert!((volume(&m) - 6000.0 * 4000.0 * 200.0).abs() < 1e-3);
        assert!((looking(&m, [0.0, 0.0, 1.0]) - 6000.0 * 4000.0).abs() < 1e-6);
        // No side between the pieces: the sides are the outline's, 20 m round and 200 mm deep.
        let sides: f64 = [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0]].into_iter().map(|w| looking(&m, w)).sum();
        assert!((sides - 20_000.0 * 200.0).abs() < 1e-6, "{sides}");
    }

    #[test]
    fn prisms_under_planes_that_meet_along_their_edge_close_too() {
        // Two halves of a wall under a gable: each half's top rises to the ridge between them.
        let rise = Plane { z: 3000.0, g: P2(0.5, 0.0) };
        let fall = Plane { z: 3000.0 + 0.5 * 4000.0, g: P2(-0.5, 0.0) };
        let gable = [
            Prism { ring: rect(0.0, 0.0, 2000.0, 300.0), bottom: Plane::flat(0.0), top: rise },
            Prism { ring: rect(2000.0, 0.0, 4000.0, 300.0), bottom: Plane::flat(0.0), top: fall },
        ];
        let m = mesh(&gable);
        assert!(closed(&m));
        let side = 4000.0 * 3000.0 + 4000.0 * 1000.0 / 2.0;
        assert!((volume(&m) - side * 300.0).abs() < 1e-3);
        assert!((looking(&m, [0.0, -1.0, 0.0]) - side).abs() < 1e-6);
        assert_eq!(looking(&m, [1.0, 0.0, 0.0]), 300.0 * 3000.0, "nothing stands at the ridge");
        // The tops look the way their planes do.
        let up = facing(&m, &Facing::Along(vec![normal(&rise), normal(&fall)]));
        assert!((up - 2.0 * 2000.0 * 300.0 * 1.25f64.sqrt()).abs() < 1e-6, "{up}");
    }

    #[test]
    fn what_holds_nothing_makes_no_triangles_and_hostile_numbers_no_panic() {
        assert_eq!(mesh(&[]), Vec::<[P3; 3]>::new());
        assert!(!closed(&[]));
        assert_eq!(volume(&[]), 0.0);
        let flat = block(rect(0.0, 0.0, 1000.0, 1000.0), 100.0, 100.0);
        let line = block(vec![P2(0.0, 0.0), P2(1000.0, 0.0), P2(2000.0, 0.0)], 0.0, 100.0);
        let nan = block(vec![P2(0.0, 0.0), P2(f64::NAN, 0.0), P2(0.0, 1000.0)], 0.0, 100.0);
        let endless = Prism { ring: rect(0.0, 0.0, 1000.0, 1000.0), bottom: Plane::flat(0.0), top: Plane::flat(f64::INFINITY) };
        assert_eq!(mesh(&[flat, line, nan, endless]), Vec::<[P3; 3]>::new());
        // A wedge, its top meeting its bottom along one edge, is closed without a side there.
        let wedge = Prism { ring: rect(0.0, 0.0, 1000.0, 1000.0), bottom: Plane::flat(0.0), top: Plane { z: 0.0, g: P2(0.1, 0.0) } };
        let m = mesh(&[wedge]);
        assert!(closed(&m) && (volume(&m) - 1000.0 * 1000.0 * 100.0 / 2.0).abs() < 1e-3);
    }
}
