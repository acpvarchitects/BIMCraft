//! Join cleanup in a section's cut (T23). Every cut that meets another is one outline already ([`super::cut_edges`]): one
//! poché, its patterns in one lattice, no line where the two meet. Two elements Join Geometry joins draw a thin line where
//! they meet only where the layers touching there differ: another material, unless the two cut alike (the same fill
//! pattern, and their categories cut in the same pen), as two layers of one compound element have a line between them. Only
//! a pair the section cuts both of does any of this work: a model with no joins draws as it did.

use std::collections::BTreeMap;

use super::{Drawing, INK, Part, edge_pieces, style_of};
use crate::geom::{self, P2};
use crate::model::{Element, Function, Id, Model, Pattern};
use crate::pen::Pen;

/// How far past an edge, outward, a point is looked for in the other element's layers: mm, as [`super::cut_edges`] looks.
const PAST: f64 = 1.0;

#[cfg(test)]
thread_local! {
    static WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many pieces of edge the cleanup has looked at on this thread since [`joint_work_reset`].
#[cfg(test)]
pub(crate) fn joint_work() -> usize {
    WORK.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn joint_work_reset() {
    WORK.with(|w| w.set(0));
}

/// The joined pairs the section cuts both of: (the one cut, the one cutting it).
pub(super) fn pairs(m: &Model, cuts: &BTreeMap<Id, Vec<Vec<P2>>>) -> Vec<(Id, Id)> {
    let mut out = Vec::new();
    for &a in cuts.keys() {
        let Some(e) = m.elements.get(a) else { continue };
        for g in e.joined_by() {
            if let Some(b) = m.elements.id(*g).filter(|b| *b != a && cuts.contains_key(b)) {
                out.push((a, b));
            }
        }
    }
    out
}

/// The layers of the elements in `pairs`, where the section cuts them: (owner, material, ring), as they were drawn.
#[derive(Default)]
pub(super) struct Layers {
    owners: Vec<Id>,
    rings: Vec<(Id, String, Vec<P2>)>,
}

impl Layers {
    pub(super) fn of(pairs: &[(Id, Id)]) -> Self {
        let mut owners: Vec<Id> = pairs.iter().flat_map(|(a, b)| [*a, *b]).collect();
        owners.sort_unstable();
        owners.dedup();
        Layers { owners, rings: Vec::new() }
    }

    /// Layer `material` of `owner` over `ring`, kept if `owner` is one of a pair.
    pub(super) fn add(&mut self, owner: Id, material: &str, ring: &[P2]) {
        if ring.len() >= 3 && self.owners.binary_search(&owner).is_ok() {
            self.rings.push((owner, material.to_string(), ring.to_vec()));
        }
    }

    /// `owner`'s layers; for one whose layers weren't drawn (a type of none, or a shape the section's layers don't follow),
    /// its cut pieces, each its structure's material (none for a type of no layers).
    fn of_owner<'a>(&'a self, m: &'a Model, owner: Id, cuts: &'a BTreeMap<Id, Vec<Vec<P2>>>) -> Vec<(&'a str, &'a [P2])> {
        let own: Vec<(&str, &[P2])> =
            self.rings.iter().filter(|(o, ..)| *o == owner).map(|(_, mat, r)| (mat.as_str(), r.as_slice())).collect();
        if !own.is_empty() {
            return own;
        }
        let layers = match m.elements.get(owner) {
            Some(Element::Wall(w)) => m.type_layers("wall", &w.type_name),
            Some(Element::Floor(f)) => m.type_layers("floor", &f.type_name),
            Some(Element::Roof(r)) => m.type_layers("roof", &r.type_name),
            Some(Element::Ceiling(c)) => m.type_layers("ceiling", &c.type_name),
            _ => &[],
        };
        let material = layers.iter().find(|l| l.function == Function::Structure).or(layers.first()).map_or("", |l| l.material.as_str());
        cuts.get(&owner).into_iter().flatten().map(|r| (material, r.as_slice())).collect()
    }
}

/// Whether layer `x` of element `a` and layer `y` of element `b` read as one where they meet: one material, or two that cut
/// alike, the same fill pattern (one that shows) in the same cut pen.
fn alike(m: &Model, (a, x): (Id, &str), (b, y): (Id, &str)) -> bool {
    if x == y {
        return true;
    }
    let (Some(p), Some(q)) = (m.materials.get(x), m.materials.get(y)) else { return false };
    let pattern = match (&p.cut_pattern, &q.cut_pattern) {
        (Some(f), Some(g)) => f == g,
        (None, None) => p.cut != Pattern::None && p.cut == q.cut,
        _ => false,
    };
    pattern && style_of(m, Some(a), None).cut == style_of(m, Some(b), None).cut
}

/// The line between each joined pair where the layers meeting differ, thin, as between two layers of one element, in the
/// one cut's name.
pub(super) fn seams(d: &mut Drawing, m: &Model, pairs: &[(Id, Id)], cuts: &BTreeMap<Id, Vec<Vec<P2>>>, layers: &Layers) {
    for &(a, b) in pairs {
        let theirs = layers.of_owner(m, b, cuts);
        let rings: Vec<&[P2]> = theirs.iter().map(|(_, r)| *r).collect();
        let mut runs: Vec<Vec<P2>> = Vec::new();
        for (material, ring) in layers.of_owner(m, a, cuts) {
            let ccw = geom::area(ring) > 0.0;
            for (p, q) in geom::ring_edges(ring) {
                if p.dist(q) <= 0.0 {
                    continue;
                }
                let out_of = ((q - p).perp() * if ccw { -1.0 } else { 1.0 }).unit() * PAST;
                for (s, e, against) in edge_pieces(p, q, &rings, out_of) {
                    #[cfg(test)]
                    WORK.with(|w| w.set(w.get() + 1));
                    let Some((other, _)) = against.and_then(|k| theirs.get(k)) else { continue };
                    if alike(m, (a, material), (b, other)) {
                        continue;
                    }
                    match runs.last_mut() {
                        Some(run) if run.last().is_some_and(|l| l.dist(s) < 1e-9) => run.push(e),
                        _ => runs.push(vec![s, e]),
                    }
                }
            }
        }
        if runs.is_empty() {
            continue;
        }
        d.owner = Some(a);
        let part = std::mem::replace(&mut d.part, Part::Pattern);
        for run in runs {
            d.line(&run, Pen::PROJECTION, INK, false);
        }
        d.part = part;
    }
}
