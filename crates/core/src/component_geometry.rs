//! Per-operation reuse of family geometry, bound to an immutable model snapshot.
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use crate::family::{Fineness, Piece};
use crate::geom::P2;
use crate::model::{Component, Id, Model};

/// What a shape is the shape of: its family, its type, whether it is mirrored, and the values its family evaluates.
pub(crate) type Key = (String, String, bool, String);

/// One local family shape and every component that places it. Mirroring and instance parameters
/// are part of the shape key; rotation and translation belong to the placement.
pub(crate) struct SharedComponents {
    pub(crate) shapes: Vec<Vec<Piece>>,
    /// Local triangle bounds for each shape, used to bound placed instances without expanding triangles.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) bounds: Vec<Option<[[f64; 3]; 2]>>,
    pub(crate) instances: Vec<ComponentInstance>,
    /// Each shape's key, and where it is in `shapes`.
    pub(crate) keys: BTreeMap<Key, usize>,
}

/// A copy of a local shape: `owner`'s, at `at`, its shape's axes taken to `axes` (each square to the others, any length, so a
/// copy may be turned, and stretched along an axis as a run's tube is along its line).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComponentInstance {
    pub(crate) owner: Id,
    pub(crate) shape: usize,
    pub(crate) at: [f64; 3],
    pub(crate) axes: [[f64; 3]; 3],
}

impl ComponentInstance {
    /// A component's, where [`Model::component_placement`] puts it.
    pub(crate) fn placed(owner: Id, shape: usize, p: &crate::hosting::Placement) -> Self {
        Self { owner, shape, at: p.origin, axes: p.axes }
    }

    /// `p` placed. For a turn this is, to the last bit, `at + (x cos - y sin, x sin + y cos, z)` as a component's pieces are
    /// placed: what the extra terms add is zero.
    pub(crate) fn place(&self, p: [f64; 3]) -> [f64; 3] {
        let [a, b, c] = self.axes;
        std::array::from_fn(|i| self.at[i] + p[0] * a[i] + p[1] * b[i] + p[2] * c[i])
    }

    /// A normal (unit) of the shape, of the copy: square to the copy's surfaces however it is stretched.
    pub(crate) fn normal(&self, n: [f64; 3]) -> [f64; 3] {
        let [a, b, c] = self.axes;
        let w = |v: [f64; 3]| 1.0 / (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        let (wa, wb, wc) = (n[0] * w(a), n[1] * w(b), n[2] * w(c));
        crate::view3d::unit(std::array::from_fn(|i| a[i] * wa + b[i] * wb + c[i] * wc))
    }

    /// The other way: a normal of the copy, of its shape.
    pub(crate) fn local_normal(&self, n: [f64; 3]) -> [f64; 3] {
        let dot = |v: [f64; 3]| v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
        let [a, b, c] = self.axes;
        crate::view3d::unit([dot(a), dot(b), dot(c)])
    }

    /// Whether its axes span space, so a normal can be placed.
    pub(crate) fn solid(&self) -> bool {
        self.axes.iter().all(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).is_normal()) && self.at.iter().all(|v| v.is_finite())
    }

    pub(crate) fn bounds(&self, local: [[f64; 3]; 2]) -> [[f64; 3]; 2] {
        let mut world = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
        for k in 0..8 {
            let corner = std::array::from_fn(|axis| local[(k >> axis) & 1][axis]);
            let p = self.place(corner);
            for axis in 0..3 {
                world[0][axis] = world[0][axis].min(p[axis]);
                world[1][axis] = world[1][axis].max(p[axis]);
            }
        }
        world
    }
}

impl SharedComponents {
    pub(crate) fn key(model: &Model, c: &Component) -> Key {
        model.shape_key(c)
    }

    /// Build each local shape once. No triangles are placed or copied for the instances here.
    pub(crate) fn new(model: &Model, cancelled: &(dyn Fn() -> bool + Sync)) -> Option<Self> {
        let mut shapes = Vec::new();
        let mut bounds = Vec::new();
        let mut indices = BTreeMap::new();
        let mut instances = Vec::new();
        for (i, (owner, element)) in model.elements.iter().enumerate() {
            if i % 256 == 0 && cancelled() {
                return None;
            }
            let crate::model::Element::Component(c) = element else { continue };
            let key = Self::key(model, c);
            let shape = match indices.get(&key) {
                Some(&n) => n,
                None => {
                    let n = shapes.len();
                    let pieces = model.component_local_pieces(c);
                    let local = pieces.iter().flat_map(|piece| piece.tris.iter().flatten());
                    let extent = local.fold(None, |state: Option<[[f64; 3]; 2]>, p| {
                        let [mut lo, mut hi] = state.unwrap_or([*p, *p]);
                        for axis in 0..3 {
                            lo[axis] = lo[axis].min(p[axis]);
                            hi[axis] = hi[axis].max(p[axis]);
                        }
                        Some([lo, hi])
                    });
                    bounds.push(extent);
                    shapes.push(pieces);
                    indices.insert(key, n);
                    n
                }
            };
            instances.push(ComponentInstance::placed(*owner, shape, &model.component_placement(c)));
        }
        (!cancelled()).then_some(Self { shapes, bounds, instances, keys: indices })
    }
}

/// The local form envelope of each family and type that components with no values of their own use, found once for a whole
/// pass over the model: most of a project's, each found without a key made for it, and no chunk of the pass finding it again.
pub(crate) type PlainEnvelopes<'a> = std::collections::HashMap<(&'a str, &'a str), Option<[[f64; 3]; 2]>>;

/// [`PlainEnvelopes`] of `components`. None when `cancelled`.
pub(crate) fn plain_envelopes<'a>(
    m: &Model,
    components: impl Iterator<Item = &'a Component>,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Option<PlainEnvelopes<'a>> {
    let mut plain = PlainEnvelopes::new();
    for (i, c) in components.filter(|c| c.params.is_empty()).enumerate() {
        if i % 256 == 0 && cancelled() {
            return None;
        }
        plain.entry((c.family.as_str(), c.type_name.as_str())).or_insert_with(|| local_envelope(m, c));
    }
    Some(plain)
}

fn local_envelope(m: &Model, c: &Component) -> Option<[[f64; 3]; 2]> {
    let family = m.families.get(&c.family)?;
    let values = family.values(&c.type_name, &c.params).ok()?;
    family.conservative_bounds(&values)
}

pub(crate) struct Components<'a> {
    model: &'a Model,
    local: BTreeMap<(Key, Fineness), Arc<Vec<Piece>>>,
    /// GEO-2c: the fineness a view asks for, and the part of the plan it shows. A component reaching into it is built at
    /// that fineness; the rest, which the view doesn't show, at the default.
    fine: Option<(Fineness, [P2; 2])>,
    /// The heights of the model's components with no values of their own, over their base, by family, type and mirroring:
    /// most of a project's, found without a key made for each.
    plain: std::collections::HashMap<(&'a str, &'a str, bool), (f64, f64)>,
    /// The points of each geometry's triangles that may be its farthest any way across or up ([`Components::extremes`]).
    extremes: BTreeMap<Key, Vec<[f64; 3]>>,
    envelopes: BTreeMap<Key, Option<[[f64; 3]; 2]>>,
    /// Each piece's corners, each once, by geometry: what a tilted copy's heights and outline in plan are found from.
    corners: BTreeMap<Key, Vec<Vec<[f64; 3]>>>,
}

impl<'a> Components<'a> {
    pub(crate) fn new(model: &'a Model) -> Self {
        Self {
            model,
            local: BTreeMap::new(),
            fine: None,
            plain: Default::default(),
            extremes: BTreeMap::new(),
            envelopes: BTreeMap::new(),
            corners: BTreeMap::new(),
        }
    }

    /// For a plan drawn through `view`: what it shows built at the [`Fineness`] its scale asks for (GEO-2c).
    pub(crate) fn for_view(model: &'a Model, view: &crate::render::View) -> Self {
        let fineness = Fineness::for_view(view.scale);
        // Every point within half the screen's diagonal of its middle, however the view is turned.
        let half = f64::from(view.width).hypot(f64::from(view.height)) / 2.0 / view.scale;
        let window = [P2(view.cx - half, view.cy - half), P2(view.cx + half, view.cy + half)];
        let shown = fineness != Fineness::DEFAULT && window.iter().all(|p| p.0.is_finite() && p.1.is_finite());
        #[cfg(test)]
        let shown = shown && !COARSE.with(std::cell::Cell::get);
        Self { fine: shown.then_some((fineness, window)), ..Self::new(model) }
    }

    /// The fineness `c` is built at: the view's where it may reach into the view, or where its extent isn't known.
    fn fineness(&mut self, c: &Component) -> Fineness {
        let Some((fineness, [lo, hi])) = self.fine else { return Fineness::DEFAULT };
        // Its box as built at the default (which its heights are read from already), placed: a family's own box isn't known
        // for every kind of form, and one not known would be built finer wherever it stands.
        let mut local = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
        for p in self.local_at(c, Fineness::DEFAULT).iter().flat_map(|pc| pc.tris.iter().flatten()) {
            for k in 0..3 {
                local[0][k] = local[0][k].min(p[k]);
                local[1][k] = local[1][k].max(p[k]);
            }
        }
        if local[0][0] > local[1][0] {
            return Fineness::DEFAULT;
        }
        let placed = self.model.component_placement(c);
        let (mut a, mut b) = (P2(f64::INFINITY, f64::INFINITY), P2(f64::NEG_INFINITY, f64::NEG_INFINITY));
        for k in 0..8 {
            let p = placed.place(std::array::from_fn(|axis| local[(k >> axis) & 1][axis]));
            (a, b) = (P2(a.0.min(p[0]), a.1.min(p[1])), P2(b.0.max(p[0]), b.1.max(p[1])));
        }
        // NaN compares false either way: such a box isn't known to be out of view.
        if a.0 > hi.0 || b.0 < lo.0 || a.1 > hi.1 || b.1 < lo.1 { Fineness::DEFAULT } else { fineness }
    }

    /// Supported local form envelopes, placed without building any triangles.
    pub(crate) fn envelope(&mut self, c: &Component, plain: &PlainEnvelopes) -> Option<[[f64; 3]; 2]> {
        let model = self.model;
        let local = match plain.get(&(c.family.as_str(), c.type_name.as_str())).filter(|_| c.params.is_empty()) {
            Some(local) => *local,
            None => *self.envelopes.entry(self.key(c)).or_insert_with(|| local_envelope(model, c)),
        };
        match crate::bounds::place_component(model, c, local?) {
            crate::bounds::Bounds::Box(bounds) => Some(bounds),
            _ => None,
        }
    }

    /// [`Components::heights`] of a component of the model's.
    pub(crate) fn heights_of(&mut self, c: &'a Component) -> (f64, f64) {
        if !c.params.is_empty() || c.host.is_some() {
            return self.heights(c);
        }
        let base = self.model.elevation(c.level) + c.offset;
        let key = (c.family.as_str(), c.type_name.as_str(), c.mirrored);
        let (lo, hi) = match self.plain.get(&key) {
            Some(span) => *span,
            None => {
                let span = self
                    .local_at(c, Fineness::DEFAULT)
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.bottom), hi.max(p.top)));
                self.plain.insert(key, span);
                span
            }
        };
        (base + lo, base + hi)
    }

    fn key(&self, c: &Component) -> Key {
        self.model.shape_key(c)
    }

    fn local(&mut self, c: &Component) -> &[Piece] {
        let fineness = self.fineness(c);
        self.local_at(c, fineness)
    }

    /// `c`'s pieces at `fineness`; where they can't be built that finely (more than the cutting may take), as always.
    fn local_at(&mut self, c: &Component, fineness: Fineness) -> &[Piece] {
        let key = (self.key(c), fineness);
        if !self.local.contains_key(&key) {
            let model = self.model;
            let pieces = if fineness == Fineness::DEFAULT {
                Arc::new(model.component_local_pieces(c))
            } else if let Some(kept) = kept(model, &key) {
                kept
            } else {
                #[cfg(test)]
                FINE_BUILDS.with(|n| n.set(n.get() + 1));
                let fine = fineness.scoped(|| model.component_local_pieces(c));
                // Too fine to cut: as always, and remembered as such.
                let pieces = if fine.is_empty() { Arc::new(self.local_at(c, Fineness::DEFAULT).to_vec()) } else { Arc::new(fine) };
                keep(model, &key, &pieces);
                pieces
            };
            self.local.insert(key.clone(), pieces);
        }
        self.local.get(&key).map_or(&[], |p| p.as_slice())
    }

    /// The points of its pieces' triangles, where it is placed, that may be the farthest any way: across (along any line
    /// in plan) or up. Those within a micron of the hull of all of them in plan, and the lowest and highest: the same
    /// points [`Components::pieces`] places, by the same sums, so the farthest of them any way is the farthest of every
    /// point of its pieces, to the last bit.
    pub(crate) fn extremes(&mut self, c: &Component) -> Vec<[f64; 3]> {
        let key = self.key(c);
        if !self.extremes.contains_key(&key) {
            let points: Vec<[f64; 3]> =
                self.local_at(c, Fineness::DEFAULT).iter().flat_map(|pc| pc.tris.iter().flatten().copied()).collect();
            let finite = |p: &[f64; 3]| p.iter().all(|v| v.is_finite());
            let hull = crate::geom::hull(points.iter().filter(|p| finite(p)).map(|p| P2(p[0], p[1])).collect());
            let near_hull = |q: P2| match hull.len() {
                0 => true,
                1 => hull.first().is_some_and(|h| h.dist(q) <= 1e-6),
                n => (0..n).any(|i| crate::geom::dist_to_segment(q, hull[i], hull[(i + 1) % n]) <= 1e-6),
            };
            // The lowest and highest, of those that aren't NaN, as `f64::min` and `f64::max` leave NaN out.
            let (low, high) =
                points.iter().filter(|p| !p[2].is_nan()).fold((None, None), |(lo, hi): (Option<[f64; 3]>, Option<[f64; 3]>), p| {
                    (Some(lo.filter(|l| l[2] <= p[2]).unwrap_or(*p)), Some(hi.filter(|h| h[2] >= p[2]).unwrap_or(*p)))
                });
            // And any point not of finite numbers: more points never change the farthest. Each place in plan once.
            let mut kept = crate::view3d::extremes(points.iter().copied().filter(|p| !finite(p) || near_hull(P2(p[0], p[1]))));
            kept.extend(low);
            kept.extend(high);
            self.extremes.insert(key.clone(), kept);
        }
        let placed = self.model.component_placement(c);
        if !placed.upright {
            // Tilted, the farthest points in plan aren't those of its own plan: every point is.
            return self.local_at(c, Fineness::DEFAULT).iter().flat_map(|pc| pc.tris.iter().flatten().map(|p| placed.place(*p))).collect();
        }
        self.extremes.get(&key).map(|v| v.iter().map(|p| placed.place(*p)).collect()).unwrap_or_default()
    }

    /// Its geometry's heights over its base, lowest and highest: none (infinity, minus infinity) when it has no pieces.
    pub(crate) fn local_heights(&mut self, c: &Component) -> (f64, f64) {
        self.local_at(c, Fineness::DEFAULT).iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.bottom), hi.max(p.top)))
    }

    /// Its pieces' corners, each once ([`Components::corners`]).
    fn corners(&mut self, c: &Component) -> &[Vec<[f64; 3]>] {
        let key = self.key(c);
        if !self.corners.contains_key(&key) {
            // Its heights and a tilted copy's outline stay the default build's, as every height does (GEO-2c).
            let corners = self
                .local_at(c, Fineness::DEFAULT)
                .iter()
                .map(|pc| {
                    let mut bits: Vec<[u64; 3]> = pc.tris.iter().flatten().map(|p| p.map(f64::to_bits)).collect();
                    bits.sort_unstable();
                    bits.dedup();
                    let mut points: Vec<[f64; 3]> = bits.into_iter().map(|b| b.map(f64::from_bits)).collect();
                    // Collected in place, it would keep room for every triangle's.
                    points.shrink_to_fit();
                    points
                })
                .collect();
            self.corners.insert(key.clone(), corners);
        }
        self.corners.get(&key).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn heights(&mut self, c: &Component) -> (f64, f64) {
        let placed = self.model.component_placement(c);
        if !placed.upright {
            // Tilted, its lowest and highest corners where it is placed, as [`Placement::piece`] finds them: no triangles
            // placed, no outline found.
            return self
                .corners(c)
                .iter()
                .flatten()
                .map(|p| placed.place(*p)[2])
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), z| (lo.min(z), hi.max(z)));
        }
        let base = placed.origin[2];
        let local = self.local_at(c, Fineness::DEFAULT);
        local.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(base + p.bottom), hi.max(base + p.top)))
    }

    /// Its pieces' outlines in plan, as [`Components::pieces`] places them, without their triangles (upright).
    pub(crate) fn outlines(&mut self, c: &Component) -> Vec<P2> {
        let placed = self.model.component_placement(c);
        if !placed.upright {
            // Tilted, each piece's hull in plan of its corners placed, as [`Placement::piece`] finds it: a corner of several
            // triangles once, which the hull, dropping points it already has, comes to the same with.
            let outlines: Vec<P2> = self
                .corners(c)
                .iter()
                .flat_map(|points| {
                    let placed = points.iter().map(|p| placed.place(*p)).filter(|p| p.iter().all(|v| v.is_finite()));
                    crate::geom::hull(placed.map(|p| P2(p[0], p[1])).collect())
                })
                .collect();
            #[cfg(test)]
            assert_eq!(outlines, self.model.component_pieces(c).into_iter().flat_map(|p| p.outline).collect::<Vec<_>>());
            return outlines;
        }
        let outlines: Vec<P2> =
            self.local(c).iter().flat_map(|pc| pc.outline.iter().map(|q| placed.place([q.0, q.1, 0.0])).map(|p| P2(p[0], p[1]))).collect();
        #[cfg(test)]
        if self.fine.is_none() {
            assert_eq!(outlines, self.model.component_pieces(c).into_iter().flat_map(|p| p.outline).collect::<Vec<_>>());
        }
        outlines
    }

    /// Its pieces where it is placed, as the 3D view draws them ([`crate::hosting::Placement::solid`]): no outline in plan.
    pub(crate) fn solids(&mut self, c: &Component) -> Vec<Piece> {
        let placed = self.model.component_placement(c);
        self.local(c).iter().map(|pc| placed.solid(pc)).collect()
    }

    pub(crate) fn pieces(&mut self, c: &Component) -> Vec<Piece> {
        let placed = self.model.component_placement(c);
        self.local(c).iter().map(|pc| placed.piece(pc)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Session;
    use serde_json::json;

    #[test]
    fn repeated_components_keep_their_placement_mirroring_and_instance_parameters() {
        let mut s = Session::new();
        s.execute(
            "family",
            &json!({"name": "Probe", "parameters": [{"name": "Height", "instance": true, "value": 750}],
            "types": {"A": {}}, "forms": [{"profile": [[0,0],[800,0],[800,400],[0,400]], "to": "Height"}]}),
        )
        .unwrap();
        for (angle, mirrored, params) in
            [(0, false, json!({})), (45, false, json!({})), (90, true, json!({})), (180, false, json!({"Height": 1200}))]
        {
            s.execute(
                "component",
                &json!({"family":"Probe", "at":[angle*10,1000], "angle": angle,
                "offset": 100, "mirrored":mirrored, "params": params}),
            )
            .unwrap();
        }
        let first = s.model.elements.iter().find(|(_, e)| matches!(e, crate::model::Element::Component(_))).map(|(id, _)| *id).unwrap();
        s.execute("material", &json!({"name": "Test paint", "color": [90, 120, 150]})).unwrap();
        s.execute("paint", &json!({"id": first, "face": "all", "material": "Test paint"})).unwrap();
        let mut cache = Components::new(&s.model);
        for _ in 0..2 {
            for e in s.model.elements.values() {
                if let crate::model::Element::Component(c) = e {
                    let a = s.model.component_pieces(c);
                    let b = cache.pieces(c);
                    assert_eq!(a, b);
                    let heights = a.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.bottom), hi.max(p.top)));
                    assert_eq!(cache.heights(c), heights);
                }
            }
        }
        let shared = SharedComponents::new(&s.model, &|| false).unwrap();
        assert_eq!(shared.shapes.len(), 3);
        assert_eq!(shared.instances.len(), 4);
        for instance in &shared.instances {
            let Some(crate::model::Element::Component(c)) = s.model.elements.get(instance.owner) else { panic!("component") };
            let placed = s.model.component_pieces(c);
            let local = &shared.shapes[instance.shape];
            let bounds = instance.bounds(shared.bounds[instance.shape].unwrap());
            assert_eq!(placed.len(), local.len());
            for (p, shape) in placed.iter().zip(local) {
                assert_eq!(p.tris, shape.tris.iter().map(|t| t.map(|q| instance.place(q))).collect::<Vec<_>>());
                for (world, local) in p.tris.iter().zip(&shape.tris) {
                    for point in world {
                        for axis in 0..3 {
                            assert!(point[axis] >= bounds[0][axis] - 1e-9 && point[axis] <= bounds[1][axis] + 1e-9);
                        }
                    }
                    let normal = |[a, b, c]: &[[f64; 3]; 3]| {
                        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                        [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
                    };
                    let expected = instance.normal(normal(local));
                    let actual = crate::view3d::unit(normal(world));
                    for axis in 0..3 {
                        assert!((actual[axis] - expected[axis]).abs() <= expected[axis].abs().max(1.0) * 1e-9);
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "measures the large tower fixture"]
    fn shared_component_shape_count_in_the_ten_storey_tower() {
        let model = crate::sample::tower::annotated(crate::sample::tower::Plate::REFERENCE, 10).unwrap();
        let shared = SharedComponents::new(&model, &|| false).unwrap();
        let tris = |shape: &Vec<Piece>| shape.iter().map(|p| p.tris.len()).sum::<usize>();
        let unique = shared.shapes.iter().map(&tris).sum::<usize>();
        let placed = shared.instances.iter().map(|i| tris(&shared.shapes[i.shape])).sum::<usize>();
        eprintln!(
            "{} instances, {} shapes, {} placed triangles, {} unique triangles",
            shared.instances.len(),
            shared.shapes.len(),
            placed,
            unique
        );
        assert!(placed > unique * 10);
    }

    #[test]
    fn copies_differing_only_in_values_their_family_doesnt_read_share_one_shape_in_the_plan_and_the_check() {
        let mut s = Session::new();
        s.execute(
            "family",
            &json!({"name": "Probe", "parameters": [{"name": "Height", "instance": true, "value": 750}],
            "types": {"A": {}}, "forms": [{"profile": [[0,0],[800,0],[800,400],[0,400]], "to": "Height"}]}),
        )
        .unwrap();
        for k in 0..6 {
            s.execute("component", &json!({"family": "Probe", "at": [k * 1000, 0]})).unwrap();
        }
        s.execute("component", &json!({"family": "Probe", "at": [0, 5000], "params": {"Height": 1200}})).unwrap();
        let ids: Vec<_> = s
            .model
            .elements
            .iter()
            .filter(|(_, e)| matches!(e, crate::model::Element::Component(c) if c.params.is_empty()))
            .map(|(id, _)| *id)
            .collect();
        s.execute("material", &json!({"name": "Test paint", "color": [90, 120, 150]})).unwrap();
        s.execute("paint", &json!({"id": ids[0], "face": "all", "material": "Test paint"})).unwrap();
        // And each of the six its own value of a project's parameter: a bolt's name, as an IFC's copies bring.
        s.execute("parameter", &json!({"name": "Bolt Name", "kind": "text", "categories": ["component"]})).unwrap();
        for (k, id) in ids.iter().enumerate() {
            s.execute("modify", &json!({"id": id, "params": {"Bolt Name": format!("BOLTM{k}")}})).unwrap();
        }
        let model = &*s.model;
        let distinct: std::collections::BTreeSet<String> = ids
            .iter()
            .filter_map(|id| match model.elements.get(*id) {
                Some(crate::model::Element::Component(c)) => Some(serde_json::to_string(&c.params).unwrap()),
                _ => None,
            })
            .collect();
        assert_eq!(distinct.len(), 6);
        let mut cache = Components::new(model);
        for e in model.elements.values() {
            if let crate::model::Element::Component(c) = e {
                assert_eq!(cache.pieces(c), model.component_pieces(c));
            }
        }
        // Two shapes, as the 3D view has: the six alike, and the taller one.
        assert_eq!(cache.local.len(), 2);
        assert_eq!(SharedComponents::new(model, &|| false).unwrap().shapes.len(), 2);
        // The check builds each shape once by the same key: two of them.
        let keys: std::collections::BTreeSet<Key> = model
            .elements
            .values()
            .filter_map(|e| match e {
                crate::model::Element::Component(c) => Some(model.shape_key(c)),
                _ => None,
            })
            .collect();
        assert_eq!(keys.len(), 2);
        model.check().unwrap();
    }

    #[test]
    fn a_tilted_components_heights_and_outline_in_the_plan_are_those_of_its_placed_pieces() {
        let mut s = Session::new();
        s.execute(
            "family",
            &json!({"name": "Probe", "types": {"A": {}},
            "forms": [{"profile": [[0,0],[800,0],[800,400],[0,400]], "to": 300}, {"circle": {"centre": [400, 200], "radius": 90}, "from": 300, "to": 900}]}),
        )
        .unwrap();
        for k in 0..4 {
            s.execute("component", &json!({"family": "Probe", "at": [k * 2000, 500], "angle": 30 * k, "mirrored": k == 3})).unwrap();
        }
        let mut model = (*s.model).clone();
        let h = std::f64::consts::FRAC_1_SQRT_2;
        let tilts = [
            [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0]],
            [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0]],
            [[h, 0.0, h], [-h, 0.0, h]],
            [[0.6, 0.8, 0.0], [0.0, 0.0, -1.0]],
        ];
        let ids: Vec<_> =
            model.elements.iter().filter(|(_, e)| matches!(e, crate::model::Element::Component(_))).map(|(id, _)| *id).collect();
        for (id, tilt) in ids.iter().zip(tilts) {
            if let Some(crate::model::Element::Component(c)) = model.elements.get_mut(*id) {
                c.tilt = Some(tilt);
            }
        }
        model.check().unwrap();
        let mut cache = Components::new(&model);
        for _ in 0..2 {
            for id in &ids {
                let Some(crate::model::Element::Component(c)) = model.elements.get(*id) else { panic!("component") };
                assert!(!model.component_placement(c).upright);
                let placed = model.component_pieces(c);
                let heights = placed.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.bottom), hi.max(p.top)));
                assert_eq!(cache.heights(c), heights);
                assert_eq!(cache.outlines(c), placed.iter().flat_map(|p| p.outline.clone()).collect::<Vec<_>>());
            }
        }
    }

    #[test]
    fn a_components_solids_for_the_3d_view_are_its_placed_pieces_without_their_outline() {
        let mut s = Session::new();
        s.execute(
            "family",
            &json!({"name": "Probe", "types": {"A": {}},
            "forms": [{"profile": [[0,0],[800,0],[800,400],[0,400]], "to": 300}, {"circle": {"centre": [400, 200], "radius": 90}, "from": 300, "to": 900}]}),
        )
        .unwrap();
        for k in 0..4 {
            s.execute("component", &json!({"family": "Probe", "at": [k * 2000, 500], "angle": 30 * k, "mirrored": k == 3})).unwrap();
        }
        let mut model = (*s.model).clone();
        let ids: Vec<_> =
            model.elements.iter().filter(|(_, e)| matches!(e, crate::model::Element::Component(_))).map(|(id, _)| *id).collect();
        // Two lying, two upright.
        for (id, tilt) in ids.iter().zip([[[1.0, 0.0, 0.0], [0.0, -1.0, 0.0]], [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0]]]) {
            if let Some(crate::model::Element::Component(c)) = model.elements.get_mut(*id) {
                c.tilt = Some(tilt);
            }
        }
        model.check().unwrap();
        let mut cache = Components::new(&model);
        for id in &ids {
            let Some(crate::model::Element::Component(c)) = model.elements.get(*id) else { panic!("component") };
            let placed = model.component_pieces(c);
            let solids = cache.solids(c);
            assert_eq!(solids.len(), placed.len());
            for (a, b) in solids.iter().zip(&placed) {
                assert_eq!((&a.tris, &a.caps, a.round, &a.vis, &a.material), (&b.tris, &b.caps, b.round, &b.vis, &b.material));
            }
        }
    }
}

/// GEO-2c: what plans built finer than the default, kept across drawings so a zoomed plan drawn again (a pan, a selection, an
/// edit elsewhere) builds again only what changed. Each generation is of one allocation of a model's families, which any edit
/// of a family replaces: a component's local pieces are its family's at its key, so an edit of a component makes a new key and
/// an edit of a family a new generation. No more than [`GENERATIONS`] generations, nor [`BUDGET`] bytes, the least lately used
/// going first.
struct Kept {
    families: crate::shared::Identity<BTreeMap<String, crate::family::Family>>,
    shapes: HashMap<(Key, Fineness), Shape>,
    used: u64,
}

/// A shape kept: its pieces, about how many bytes they take, and when it was last used.
type Shape = (Arc<Vec<Piece>>, usize, u64);

const GENERATIONS: usize = 4;
const BUDGET: usize = 256 << 20;

/// The generations kept, a clock that marks each use, and the bytes they hold, counted as shapes come and go.
struct Store {
    generations: Vec<Kept>,
    clock: u64,
    bytes: usize,
}

static KEPT: Mutex<Store> = Mutex::new(Store { generations: Vec::new(), clock: 0, bytes: 0 });

#[cfg(test)]
thread_local! {
    static FINE_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
thread_local! {
    static COARSE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `f`, with the plans it draws on this thread built at the default however near they are: as before GEO-2c, to compare.
#[cfg(test)]
pub(crate) fn without_fineness<T>(f: impl FnOnce() -> T) -> T {
    struct Back(bool);
    impl Drop for Back {
        fn drop(&mut self) {
            COARSE.with(|c| c.set(self.0));
        }
    }
    let _back = Back(COARSE.with(|c| c.replace(true)));
    f()
}

/// How many components this thread has built finer than the default, not finding them kept.
#[cfg(test)]
pub(crate) fn fine_builds() -> usize {
    FINE_BUILDS.with(std::cell::Cell::get)
}

/// Whether a generation of `families` is kept.
#[cfg(test)]
pub(crate) fn keeps(families: &crate::shared::Identity<BTreeMap<String, crate::family::Family>>) -> bool {
    KEPT.lock().is_ok_and(|k| k.generations.iter().any(|g| g.families.is(families)))
}

/// The bytes kept, as counted and as summed afresh.
#[cfg(test)]
pub(crate) fn kept_bytes() -> (usize, usize) {
    KEPT.lock().map_or((0, 0), |k| (k.bytes, k.generations.iter().flat_map(|g| g.shapes.values()).map(|(_, b, _)| *b).sum()))
}

fn kept(model: &Model, key: &(Key, Fineness)) -> Option<Arc<Vec<Piece>>> {
    let mut guard = KEPT.lock().ok()?;
    let Store { generations, clock, .. } = &mut *guard;
    *clock += 1;
    let now = *clock;
    let g = generations.iter_mut().find(|g| g.families.is_of(&model.families))?;
    g.used = now;
    let (pieces, _, used) = g.shapes.get_mut(key)?;
    *used = now;
    Some(Arc::clone(pieces))
}

fn keep(model: &Model, key: &(Key, Fineness), pieces: &Arc<Vec<Piece>>) {
    let Ok(mut guard) = KEPT.lock() else { return };
    let Store { generations, clock, bytes: total } = &mut *guard;
    *clock += 1;
    let now = *clock;
    let held = |g: &Kept| g.shapes.values().map(|(_, b, _)| *b).sum::<usize>();
    // The generations of models gone (closed, or a family editor's drag a step behind): nothing can ask for them again.
    generations.retain(|g| {
        let alive = g.families.alive();
        if !alive {
            *total -= held(g);
        }
        alive
    });
    let bytes = pieces.iter().map(|p| 128 + p.tris.len() * 72 + (p.outline.len() + p.holes.iter().map(Vec::len).sum::<usize>()) * 16).sum();
    if !generations.iter().any(|g| g.families.is_of(&model.families)) {
        if generations.len() >= GENERATIONS
            && let Some(oldest) = (0..generations.len()).min_by_key(|i| generations[*i].used)
        {
            *total -= held(&generations.swap_remove(oldest));
        }
        generations.push(Kept { families: model.families.identity(), shapes: HashMap::new(), used: now });
    }
    if let Some(g) = generations.iter_mut().find(|g| g.families.is_of(&model.families)) {
        g.used = now;
        if let Some((_, was, _)) = g.shapes.insert(key.clone(), (Arc::clone(pieces), bytes, now)) {
            *total -= was;
        }
        *total += bytes;
    }
    // Over the budget: the least lately used shapes go, of every generation, until a quarter of it is free.
    if *total > BUDGET {
        let mut by_use: Vec<(u64, usize, (Key, Fineness))> =
            generations.iter().enumerate().flat_map(|(i, g)| g.shapes.iter().map(move |(k, (_, _, u))| (*u, i, k.clone()))).collect();
        by_use.sort_by_key(|(u, ..)| *u);
        for (_, i, k) in by_use {
            if *total <= BUDGET / 4 * 3 {
                break;
            }
            if let Some((_, b, _)) = generations.get_mut(i).and_then(|g| g.shapes.remove(&k)) {
                *total -= b;
            }
        }
    }
}
