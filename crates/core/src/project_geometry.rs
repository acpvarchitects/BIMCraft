//! What Cut Geometry and Uncut Geometry pick in a plan (T23), whether or not the component draws anything there: a family that
//! is only a void shows no forms and no symbols, so the tool picks through the voids' own outlines. For Cut, every component with
//! voids that cut a host ([`Model::placed_host_voids`]); for Uncut, the components a wall's `void_cuts` names, each even while its
//! tie sleeps (its family's flag off), and one whose voids are gone by its placement point alone.

use std::collections::BTreeMap;

use crate::geom::{self, P2};
use crate::model::{Element, Id, Model};
use crate::view3d::{P3, cross, sub};

/// A component the tool can pick: the outline in plan of its voids, as segments, and the point it is placed at, which picks it
/// too (and alone, where it has no voids left).
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub id: Id,
    pub edges: Vec<[P2; 2]>,
    pub anchor: P2,
}

/// What the second pick on wall `wall` can take: with `uncut`, the components that cut it by Cut Geometry; else every
/// component with voids that cut a host and doesn't cut it yet. Whether Cut takes one is the command's to say.
pub fn cutters(m: &Model, wall: Id, uncut: bool) -> Vec<Candidate> {
    let Some(w) = m.wall(wall) else { return Vec::new() };
    let ids: Vec<Id> = if uncut {
        w.void_cuts.iter().filter_map(|g| m.elements.id(*g)).collect()
    } else {
        m.elements
            .iter()
            .filter(|(id, e)| matches!(e, Element::Component(_)) && !w.void_cuts.contains(&m.guid(**id)))
            .map(|(id, _)| *id)
            .collect()
    };
    ids.into_iter()
        .filter_map(|id| {
            let Some(Element::Component(c)) = m.elements.get(id) else { return None };
            let voids = m.placed_host_voids(c);
            if !uncut && voids.is_empty() {
                return None;
            }
            Some(Candidate { id, edges: voids.iter().flat_map(|v| outline(v)).collect(), anchor: c.at })
        })
        .collect()
}

/// The candidate nearest `at` within `tol` mm of one of its edges or its anchor.
pub fn pick(candidates: &[Candidate], at: P2, tol: f64) -> Option<Id> {
    candidates
        .iter()
        .map(|c| {
            let edge = c.edges.iter().map(|[a, b]| geom::dist_to_segment(at, *a, *b)).fold(f64::INFINITY, f64::min);
            (c.id, edge.min(at.dist(c.anchor)))
        })
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// A point in plan to the hundredth of a millimetre, which an edge two triangles share is found by.
type Key = (i64, i64);

/// A void's outline in plan: the edges its upward faces don't share, which for a void standing straight up is its profile; for
/// one with no upward face (seen edge on), the hull of its corners.
fn outline(tris: &[[P3; 3]]) -> Vec<[P2; 2]> {
    let key = |p: P3| ((p[0] * 100.0).round() as i64, (p[1] * 100.0).round() as i64);
    let mut edges: BTreeMap<(Key, Key), (usize, [P2; 2])> = BTreeMap::new();
    for [a, b, c] in tris {
        let n = cross(sub(*b, *a), sub(*c, *a));
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len <= 0.0 || n[2] < 0.5 * len {
            continue;
        }
        for (p, q) in [(a, b), (b, c), (c, a)] {
            let (kp, kq) = (key(*p), key(*q));
            let k = if kp <= kq { (kp, kq) } else { (kq, kp) };
            edges.entry(k).or_insert((0, [P2(p[0], p[1]), P2(q[0], q[1])])).0 += 1;
        }
    }
    let out: Vec<[P2; 2]> = edges.into_values().filter(|(n, _)| *n == 1).map(|(_, e)| e).collect();
    if !out.is_empty() {
        return out;
    }
    let hull = geom::hull(tris.iter().flatten().map(|p| P2(p[0], p[1])).collect());
    geom::ring_edges(&hull).map(|(a, b)| [a, b]).collect()
}
