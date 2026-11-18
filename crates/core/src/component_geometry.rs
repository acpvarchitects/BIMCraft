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

/// A component's local pieces as built, and what a plan reads of them without going through their triangles again: the box
/// round their triangles (none when they have none) and their lowest bottom and highest top (infinity and minus infinity).
pub(crate) struct Built {
    pub(crate) pieces: Vec<Piece>,
    pub(crate) bounds: Option<[[f64; 3]; 2]>,
    pub(crate) heights: (f64, f64),
}

impl Built {
    pub(crate) fn new(pieces: Vec<Piece>) -> Self {
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for p in pieces.iter().flat_map(|pc| pc.tris.iter().flatten()) {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let heights = pieces.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.bottom), hi.max(p.top)));
        Self { bounds: (lo[0] <= hi[0]).then_some([lo, hi]), heights, pieces }
    }
}

/// About how many bytes `pieces` take: each piece, the room its lists hold (used or not), and what is in them.
pub(crate) fn estimate(pieces: &[Piece]) -> usize {
    use std::mem::size_of;
    let piece = |p: &Piece| {
        p.tris.capacity() * size_of::<[[f64; 3]; 3]>()
            + p.outline.capacity() * size_of::<P2>()
            + p.holes.capacity() * size_of::<Vec<P2>>()
            + p.holes.iter().map(|h| h.capacity() * size_of::<P2>()).sum::<usize>()
            + p.material.capacity()
            + p.nested.capacity() * size_of::<crate::ids::Guid>()
    };
    size_of::<Built>() + std::mem::size_of_val(pieces) + pieces.iter().map(piece).sum::<usize>()
}

/// What a plan found of one of its components: its key, whether it meets the plan's window, and the build it draws.
struct Seen {
    key: Arc<Key>,
    inside: Option<bool>,
    shown: Option<Arc<Built>>,
}

pub(crate) struct Components<'a> {
    model: &'a Model,
    local: HashMap<(Arc<Key>, Fineness), Arc<Built>>,
    /// The keys and finenesses not kept across drawings, looked for once.
    missing: std::collections::HashSet<(Arc<Key>, Fineness)>,
    /// GEO-2c: the fineness a view asks for, and the part of the plan it shows. A component reaching into it is built at
    /// that fineness; the rest, which the view doesn't show, at the default.
    fine: Option<(Fineness, [P2; 2])>,
    /// GEO-2g: the part of the plan a component has to reach into to be built and drawn: the view's screen and one more
    /// screen each way ([`window_of`]). None, all of it.
    window: Option<[P2; 2]>,
    /// GEO-2g: whether its default builds are kept across drawings, as a plan's are.
    keeps: bool,
    /// What it found of each component of the model's, by where the component is: each looked up once a drawing.
    seen: HashMap<usize, Seen>,
    /// The heights of the model's components with no values of their own, over their base, by family, type and mirroring:
    /// most of a project's, found without a key made for each.
    plain: HashMap<(&'a str, &'a str, bool), (f64, f64)>,
    /// The points of each geometry's triangles that may be its farthest any way across or up ([`Components::extremes`]).
    extremes: BTreeMap<Key, Vec<[f64; 3]>>,
    envelopes: HashMap<Arc<Key>, Option<[[f64; 3]; 2]>>,
    /// Each piece's corners, each once, by geometry: what a tilted copy's heights and outline in plan are found from.
    corners: BTreeMap<Key, Vec<Vec<[f64; 3]>>>,
}

/// GEO-2g: the part of the plan `view` builds and draws components in: its screen and one more screen each way, however it is
/// turned, so a pan of up to a screen shows them before the next drawing, and so do a pick, a hover, Tab and a snap near its
/// edge. None where the view isn't a number.
pub(crate) fn window_of(view: &crate::render::View) -> Option<[P2; 2]> {
    let (w, h) = (1.5 * f64::from(view.width) / view.scale, 1.5 * f64::from(view.height) / view.scale);
    let (sin, cos) = view.angle.to_radians().sin_cos();
    let (ex, ey) = (w * cos.abs() + h * sin.abs(), w * sin.abs() + h * cos.abs());
    let window = [P2(view.cx - ex, view.cy - ey), P2(view.cx + ex, view.cy + ey)];
    window.iter().all(|p| p.0.is_finite() && p.1.is_finite()).then_some(window)
}

/// Whether box `[lo, hi]` in plan meets `window`. NaN compares false either way: such a box isn't known to be out of it.
fn meets([a, b]: [P2; 2], [lo, hi]: [P2; 2]) -> bool {
    !(a.0 > hi.0 || b.0 < lo.0 || a.1 > hi.1 || b.1 < lo.1)
}

impl<'a> Components<'a> {
    pub(crate) fn new(model: &'a Model) -> Self {
        Self {
            model,
            local: HashMap::new(),
            missing: Default::default(),
            fine: None,
            window: None,
            keeps: false,
            seen: HashMap::new(),
            plain: Default::default(),
            extremes: BTreeMap::new(),
            envelopes: HashMap::new(),
            corners: BTreeMap::new(),
        }
    }

    /// For a plan's extent: what it builds kept across drawings, for the plan drawn next.
    pub(crate) fn kept(model: &'a Model) -> Self {
        Self { keeps: true, ..Self::new(model) }
    }

    /// For a plan drawn through `view`: what it shows built at the [`Fineness`] its scale asks for (GEO-2c), only what meets
    /// its window built at all, and what it builds kept across drawings (GEO-2g).
    pub(crate) fn for_view(model: &'a Model, view: &crate::render::View) -> Self {
        let fineness = Fineness::for_view(view.scale);
        // Every point within half the screen's diagonal of its middle, however the view is turned.
        let half = f64::from(view.width).hypot(f64::from(view.height)) / 2.0 / view.scale;
        let shown_window = [P2(view.cx - half, view.cy - half), P2(view.cx + half, view.cy + half)];
        let shown = fineness != Fineness::DEFAULT && shown_window.iter().all(|p| p.0.is_finite() && p.1.is_finite());
        #[cfg(test)]
        let shown = shown && !COARSE.with(std::cell::Cell::get);
        let window = window_of(view);
        #[cfg(test)]
        let window = window.filter(|_| !UNCULLED.with(std::cell::Cell::get));
        Self { fine: shown.then_some((fineness, shown_window)), window, keeps: true, ..Self::new(model) }
    }

    fn key(&self, c: &Component) -> Arc<Key> {
        Arc::new(self.model.shape_key(c))
    }

    /// What it found of `c`, a component of the model's: its key made once.
    fn seen(&mut self, c: &'a Component) -> &mut Seen {
        let model = self.model;
        self.seen.entry(std::ptr::from_ref(c) as usize).or_insert_with(|| Seen {
            key: Arc::new(model.shape_key(c)),
            inside: None,
            shown: None,
        })
    }

    /// Its build at `fineness` where one is at hand, here or kept from an earlier drawing: none built.
    fn known(&mut self, key: &Arc<Key>, fineness: Fineness) -> Option<Arc<Built>> {
        let k = (Arc::clone(key), fineness);
        if let Some(b) = self.local.get(&k) {
            return Some(Arc::clone(b));
        }
        if !(self.keeps || fineness != Fineness::DEFAULT) || self.missing.contains(&k) {
            return None;
        }
        match kept(self.model, &k) {
            Some(b) => {
                self.local.insert(k, Arc::clone(&b));
                Some(b)
            }
            None => {
                self.missing.insert(k);
                None
            }
        }
    }

    /// `c`'s build at `fineness`, `key` its key; where it can't be built that finely (more than the cutting may take), as
    /// always, the same build shared.
    fn local_at(&mut self, key: &Arc<Key>, c: &Component, fineness: Fineness) -> Arc<Built> {
        if let Some(b) = self.known(key, fineness) {
            return b;
        }
        let model = self.model;
        count(fineness, 1);
        let built = build(model, c, fineness);
        let built = self.finer(key, c, fineness, built);
        self.store(key, fineness, &built);
        built
    }

    /// `built`, `c`'s at `fineness`, as it is kept: its default build, shared, where it is too fine to cut (as always, and
    /// remembered as such) or comes to just what the default builds (no curve of it large enough to take more chords).
    fn finer(&mut self, key: &Arc<Key>, c: &Component, fineness: Fineness, built: Built) -> Arc<Built> {
        if fineness == Fineness::DEFAULT {
            return Arc::new(built);
        }
        if built.pieces.is_empty() {
            return self.local_at(key, c, Fineness::DEFAULT);
        }
        let default = self.local_at(key, c, Fineness::DEFAULT);
        if default.pieces == built.pieces { default } else { Arc::new(built) }
    }

    /// `built` as `key`'s at `fineness`, for this drawing and, kept, the next.
    fn store(&mut self, key: &Arc<Key>, fineness: Fineness, built: &Arc<Built>) {
        let k = (Arc::clone(key), fineness);
        self.missing.remove(&k);
        if self.keeps || fineness != Fineness::DEFAULT {
            keep(self.model, std::slice::from_ref(&(k.clone(), Arc::clone(built))));
        }
        self.local.insert(k, Arc::clone(built));
    }

    fn default_of(&mut self, c: &'a Component) -> Arc<Built> {
        let key = Arc::clone(&self.seen(c).key);
        self.local_at(&key, c, Fineness::DEFAULT)
    }

    /// Its build's box placed, in plan; none for a build of no triangles.
    fn placed_box(&self, c: &Component, local: [[f64; 3]; 2]) -> [P2; 2] {
        let placed = self.model.component_placement(c);
        let (mut a, mut b) = (P2(f64::INFINITY, f64::INFINITY), P2(f64::NEG_INFINITY, f64::NEG_INFINITY));
        for k in 0..8 {
            let p = placed.place(std::array::from_fn(|axis| local[(k >> axis) & 1][axis]));
            (a, b) = (P2(a.0.min(p[0]), a.1.min(p[1])), P2(b.0.max(p[0]), b.1.max(p[1])));
        }
        [a, b]
    }

    /// The fineness `c` is built at: the view's where it may reach into the view, or where its extent isn't known.
    fn fineness(&mut self, c: &'a Component) -> Fineness {
        let Some((fineness, window)) = self.fine else { return Fineness::DEFAULT };
        // Its box as built at the default (which its heights are read from already), placed: a family's own box isn't known
        // for every kind of form, and one not known would be built finer wherever it stands.
        match self.default_of(c).bounds {
            Some(local) if meets(self.placed_box(c, local), window) => fineness,
            _ => Fineness::DEFAULT,
        }
    }

    /// Its supported local form envelope ([`crate::family::Family::conservative_bounds`]), placed: none where it isn't
    /// known. Found once for a key, and kept across drawings with its builds.
    fn placed_envelope(&mut self, c: &'a Component) -> Option<[[f64; 3]; 2]> {
        let key = Arc::clone(&self.seen(c).key);
        let local = match self.envelopes.get(&key) {
            Some(local) => *local,
            None => {
                let local = match self.keeps.then(|| kept_envelope(self.model, &key)).flatten() {
                    Some(local) => local,
                    None => {
                        let local = local_envelope(self.model, c);
                        if self.keeps {
                            keep_envelopes(self.model, std::slice::from_ref(&(Arc::clone(&key), local)));
                        }
                        local
                    }
                };
                self.envelopes.insert(key, local);
                local
            }
        };
        match crate::bounds::place_component(self.model, c, local?) {
            crate::bounds::Bounds::Box(bounds) => Some(bounds),
            _ => None,
        }
    }

    /// GEO-2g: whether `c` meets the plan's window: by its default build's box where that is at hand; else not where its
    /// envelope is known to miss the window; else by the box of its default build, built. A component of no geometry
    /// meets it (there is nothing of it to build, and its lines and symbols are drawn as they always were).
    pub(crate) fn in_window(&mut self, c: &'a Component) -> bool {
        let Some(window) = self.window else { return true };
        if let Some(inside) = self.seen(c).inside {
            return inside;
        }
        let key = Arc::clone(&self.seen(c).key);
        let built = match self.known(&key, Fineness::DEFAULT) {
            Some(built) => Some(built),
            None => match self.placed_envelope(c) {
                Some([lo, hi]) if !meets([P2(lo[0], lo[1]), P2(hi[0], hi[1])], window) => None,
                _ => Some(self.local_at(&key, c, Fineness::DEFAULT)),
            },
        };
        let inside = built.is_some_and(|b| b.bounds.is_none_or(|local| meets(self.placed_box(c, local), window)));
        self.seen(c).inside = Some(inside);
        inside
    }

    /// [`Components::in_window`] where that is known without building `c`; else, that it may.
    pub(crate) fn near_window(&mut self, c: &'a Component) -> bool {
        let Some(window) = self.window else { return true };
        if let Some(inside) = self.seen(c).inside {
            return inside;
        }
        let key = Arc::clone(&self.seen(c).key);
        match self.known(&key, Fineness::DEFAULT) {
            Some(_) => self.in_window(c),
            None => self.placed_envelope(c).is_none_or(|[lo, hi]| meets([P2(lo[0], lo[1]), P2(hi[0], hi[1])], window)),
        }
    }

    /// GEO-2g: whether `c` may reach from a floor plan's view depth `depth` (exclusive) to under its cut plane `cut`: false
    /// only where its envelope, known without building it, lies wholly at or over the cut or wholly at or under the view
    /// depth. A component the plan sees only beyond its cut, under it and over the view depth, may.
    pub(crate) fn may_reach(&mut self, c: &'a Component, cut: f64, depth: f64) -> bool {
        let key = Arc::clone(&self.seen(c).key);
        if self.known(&key, Fineness::DEFAULT).is_some() {
            return true;
        }
        self.placed_envelope(c).is_none_or(|[lo, hi]| !(lo[2] >= cut || hi[2] <= depth))
    }

    /// Its geometry's heights over its base, lowest and highest, where they are known without building it: its default
    /// build's, or else its envelope's. None where neither is.
    pub(crate) fn known_local_heights(&mut self, c: &'a Component) -> Option<(f64, f64)> {
        let key = Arc::clone(&self.seen(c).key);
        if let Some(built) = self.known(&key, Fineness::DEFAULT) {
            return Some(built.heights);
        }
        if !self.envelopes.contains_key(&key) {
            self.placed_envelope(c);
        }
        self.envelopes.get(&key).copied().flatten().map(|[lo, hi]| (lo[2], hi[2]))
    }

    /// GEO-2g: builds at once, on several threads ([`in_parallel`]), what a plan of `cs` will ask of them: the default build
    /// of each that meets the window (by its envelope, where known) and, with `range` (a floor plan's cut plane and view
    /// depth), that may reach into it; then the finer build of each of those the view shows. What is kept from an earlier
    /// drawing isn't built again. They are put in the model's order, so the drawing doesn't depend on how many threads built
    /// them.
    pub(crate) fn prepare(&mut self, cs: &[&'a Component], range: Option<(f64, f64)>) {
        let mut jobs: Vec<(Arc<Key>, &'a Component)> = Vec::new();
        let mut queued = std::collections::HashSet::new();
        for &c in cs {
            let key = Arc::clone(&self.seen(c).key);
            if queued.contains(&key) || self.known(&key, Fineness::DEFAULT).is_some() {
                continue;
            }
            let reaches = range.is_none_or(|(cut, depth)| self.may_reach(c, cut, depth));
            let inside = self
                .window
                .is_none_or(|window| self.placed_envelope(c).is_none_or(|[lo, hi]| meets([P2(lo[0], lo[1]), P2(hi[0], hi[1])], window)));
            if reaches && inside {
                queued.insert(Arc::clone(&key));
                jobs.push((key, c));
            }
        }
        self.build_all(&jobs, Fineness::DEFAULT);
        let Some((fineness, _)) = self.fine else { return };
        let mut jobs: Vec<(Arc<Key>, &'a Component)> = Vec::new();
        queued.clear();
        for &c in cs {
            if !self.in_window(c) || self.fineness(c) == Fineness::DEFAULT {
                continue;
            }
            if let Some((cut, depth)) = range {
                let (bottom, top) = self.heights(c);
                if !(bottom < cut && top > depth) {
                    continue;
                }
            }
            let key = Arc::clone(&self.seen(c).key);
            if !queued.contains(&key) && self.known(&key, fineness).is_none() {
                queued.insert(Arc::clone(&key));
                jobs.push((key, c));
            }
        }
        self.build_all(&jobs, fineness);
    }

    /// Each of `jobs` built at `fineness`, on several threads; a finer build too fine to cut as its default one.
    fn build_all(&mut self, jobs: &[(Arc<Key>, &'a Component)], fineness: Fineness) {
        if jobs.is_empty() {
            return;
        }
        let model = self.model;
        let built = in_parallel(jobs, |(_, c)| build(model, c, fineness));
        count(fineness, jobs.len());
        let mut kept = Vec::with_capacity(jobs.len());
        for ((key, c), built) in jobs.iter().zip(built) {
            let built = self.finer(key, c, fineness, built);
            let k = (Arc::clone(key), fineness);
            self.missing.remove(&k);
            self.local.insert(k.clone(), Arc::clone(&built));
            kept.push((k, built));
        }
        if self.keeps || fineness != Fineness::DEFAULT {
            keep(model, &kept);
        }
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
                let span = self.default_of(c).heights;
                self.plain.insert(key, span);
                span
            }
        };
        (base + lo, base + hi)
    }

    /// `c`'s build at the default, for what doesn't come from the model's components.
    fn default_any(&mut self, c: &Component) -> Arc<Built> {
        let key = self.key(c);
        self.local_at(&key, c, Fineness::DEFAULT)
    }

    /// The points of its pieces' triangles, where it is placed, that may be the farthest any way: across (along any line
    /// in plan) or up. Those within a micron of the hull of all of them in plan, and the lowest and highest: the same
    /// points [`Components::pieces`] places, by the same sums, so the farthest of them any way is the farthest of every
    /// point of its pieces, to the last bit.
    pub(crate) fn extremes(&mut self, c: &Component) -> Vec<[f64; 3]> {
        let key = self.model.shape_key(c);
        let built = self.default_any(c);
        if !self.extremes.contains_key(&key) {
            let points: Vec<[f64; 3]> = built.pieces.iter().flat_map(|pc| pc.tris.iter().flatten().copied()).collect();
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
            return built.pieces.iter().flat_map(|pc| pc.tris.iter().flatten().map(|p| placed.place(*p))).collect();
        }
        self.extremes.get(&key).map(|v| v.iter().map(|p| placed.place(*p)).collect()).unwrap_or_default()
    }

    /// Its pieces' corners, each once ([`Components::corners`]).
    fn corners(&mut self, c: &Component) -> &[Vec<[f64; 3]>] {
        let key = self.model.shape_key(c);
        if !self.corners.contains_key(&key) {
            // Its heights and a tilted copy's outline stay the default build's, as every height does (GEO-2c).
            let corners = self
                .default_any(c)
                .pieces
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

    pub(crate) fn heights(&mut self, c: &'a Component) -> (f64, f64) {
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
        let (lo, hi) = self.default_of(c).heights;
        (base + lo, base + hi)
    }

    /// The build `c` is drawn with: at the fineness the view asks of it. Found once a drawing, for its outlines and its
    /// pieces alike.
    fn shown(&mut self, c: &'a Component) -> Arc<Built> {
        if let Some(b) = &self.seen(c).shown {
            return Arc::clone(b);
        }
        let fineness = self.fineness(c);
        let key = Arc::clone(&self.seen(c).key);
        let built = self.local_at(&key, c, fineness);
        self.seen(c).shown = Some(Arc::clone(&built));
        built
    }

    /// Its pieces' outlines in plan, as [`Components::pieces`] places them, without their triangles (upright).
    pub(crate) fn outlines(&mut self, c: &'a Component) -> Vec<P2> {
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
        let outlines: Vec<P2> = self
            .shown(c)
            .pieces
            .iter()
            .flat_map(|pc| pc.outline.iter().map(|q| placed.place([q.0, q.1, 0.0])).map(|p| P2(p[0], p[1])))
            .collect();
        #[cfg(test)]
        if self.fine.is_none() {
            assert_eq!(outlines, self.model.component_pieces(c).into_iter().flat_map(|p| p.outline).collect::<Vec<_>>());
        }
        outlines
    }

    /// Its pieces where it is placed, as the 3D view draws them ([`crate::hosting::Placement::solid`]): no outline in plan.
    pub(crate) fn solids(&mut self, c: &Component) -> Vec<Piece> {
        let placed = self.model.component_placement(c);
        self.default_any(c).pieces.iter().map(|pc| placed.solid(pc)).collect()
    }

    pub(crate) fn pieces(&mut self, c: &'a Component) -> Vec<Piece> {
        let placed = self.model.component_placement(c);
        self.shown(c).pieces.iter().map(|pc| placed.piece(pc)).collect()
    }
}

/// `c`'s local pieces at `fineness`, whatever the fineness of the thread it is built on.
fn build(model: &Model, c: &Component, fineness: Fineness) -> Built {
    #[cfg(test)]
    if c.params.values().any(|v| v.as_f64() == Some(PANIC_PROBE)) {
        panic!("a probe's build panicked");
    }
    Built::new(fineness.scoped(|| model.component_local_pieces(c)))
}

/// GEO-2g: the plan's own workers, as many as the 3D view's and apart from them, so a plan built while a 3D view renders
/// neither waits for it nor nests in it.
fn plan_pool() -> Option<&'static rayon::ThreadPool> {
    static POOL: std::sync::OnceLock<Option<rayon::ThreadPool>> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let n = std::thread::available_parallelism().map_or(1, usize::from).saturating_sub(1).clamp(1, 8);
        rayon::ThreadPoolBuilder::new().num_threads(n).thread_name(|n| format!("bimcraft-plan-{n}")).build().ok()
    })
    .as_ref()
}

/// `f` of each of `jobs`, in their order, on the plan's workers when there are several of each. A panic in one comes back
/// out of this as it would have from the loop.
fn in_parallel<J: Sync, T: Send>(jobs: &[J], f: impl Fn(&J) -> T + Sync + Send) -> Vec<T> {
    #[cfg(test)]
    {
        use rayon::prelude::*;
        let threads = Mutex::new(std::collections::HashSet::new());
        let g = |j: &J| {
            if let Ok(mut t) = threads.lock() {
                t.insert(std::thread::current().id());
            }
            f(j)
        };
        let out = match TEST_POOL.with(|p| p.borrow().clone()) {
            Some(Some(pool)) => pool.install(|| jobs.par_iter().map(&g).collect()),
            Some(None) => jobs.iter().map(&g).collect(),
            None => on_workers(jobs, g),
        };
        let n = threads.lock().map_or(0, |t| t.len());
        BUILD_THREADS.with(|b| b.set(b.get().max(n)));
        out
    }
    #[cfg(not(test))]
    on_workers(jobs, f)
}

fn on_workers<J: Sync, T: Send>(jobs: &[J], f: impl Fn(&J) -> T + Sync + Send) -> Vec<T> {
    use rayon::prelude::*;
    match plan_pool() {
        Some(pool) if jobs.len() > 1 && pool.current_num_threads() > 1 => pool.install(|| jobs.par_iter().map(f).collect()),
        _ => jobs.iter().map(f).collect(),
    }
}

/// `n` builds at `fineness`, counted on this thread.
fn count(fineness: Fineness, n: usize) {
    #[cfg(test)]
    if fineness == Fineness::DEFAULT {
        DEFAULT_BUILDS.with(|b| b.set(b.get() + n));
    } else {
        FINE_BUILDS.with(|b| b.set(b.get() + n));
    }
    #[cfg(not(test))]
    let _ = (fineness, n);
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

    /// ec's review of #84: a build too fine to cut is its default build, shared, not a copy of it.
    #[test]
    fn a_build_too_fine_to_cut_shares_its_default_build() {
        let mut s = Session::new();
        let mut forms = vec![json!({"sketch": [{"centre": [0, 0], "radius": 20000}], "from": 0, "to": 3000})];
        for x in [-10000, 10000] {
            forms.push(json!({"sketch": [{"centre": [x, 0], "radius": 6000}], "from": 1000, "to": 4000, "void": true}));
        }
        s.execute("family", &json!({"name": "Round", "category": "generic", "types": {"T": {}}, "forms": forms})).unwrap();
        s.execute("component", &json!({"family": "Round", "at": [0, 0]})).unwrap();
        let Some(crate::model::Element::Component(c)) =
            s.model.elements.values().find(|e| matches!(e, crate::model::Element::Component(_)))
        else {
            panic!("a component")
        };
        let view = crate::render::View { angle: 0.0, scale: 2.0, cx: 20_000.0, cy: 0.0, width: 1000, height: 800 };
        let mut cache = Components::for_view(&s.model, &view);
        cache.pieces(c);
        let at = |f: bool| cache.local.iter().find(|((_, k), _)| (*k != Fineness::DEFAULT) == f).map(|(_, b)| Arc::clone(b)).unwrap();
        assert!(Arc::ptr_eq(&at(true), &at(false)));
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

/// GEO-2c and GEO-2g: what plans built, kept across drawings so a plan drawn again (a pan, a selection, an edit elsewhere)
/// builds again only what changed. Each generation is of one allocation of a model's families, which any edit of a family
/// replaces: a component's local pieces are its family's at its key, so an edit of a component makes a new key and an edit of
/// a family a new generation. No more than [`GENERATIONS`] generations, those of models gone going at the next keep. Two
/// tiers, each with a budget of its own, the least lately used of a tier going first: the default builds, which a plan reads
/// every component's heights and box from ([`DEFAULT_BUDGET`]), and the finer ones it draws what it shows with ([`BUDGET`]),
/// so a zoom in never pushes out what every drawing needs. And each key's envelope, which a plan leaves out what is far from
/// its window by without building it.
struct Kept {
    families: crate::shared::Identity<BTreeMap<String, crate::family::Family>>,
    shapes: HashMap<(Arc<Key>, Fineness), Shape>,
    envelopes: HashMap<Arc<Key>, Option<[[f64; 3]; 2]>>,
    used: u64,
}

/// A shape kept: its build, about how many bytes it takes, and when it was last used.
struct Shape {
    built: Arc<Built>,
    bytes: usize,
    used: u64,
}

/// A build to keep: its key and fineness, and itself.
type Keeping = ((Arc<Key>, Fineness), Arc<Built>);

/// A key's local envelope; none where it isn't known.
type Envelope = Option<[[f64; 3]; 2]>;

const GENERATIONS: usize = 4;
/// The finer builds' budget (GEO-2c).
const BUDGET: usize = 256 << 20;
/// The default builds' budget (GEO-2g): 10,000 of GEO-2f's facade panels take some 240 MB (about 24 KB each).
const DEFAULT_BUDGET: usize = 384 << 20;
/// The most envelopes a generation keeps, some 40 MB; past them it starts again.
const ENVELOPES: usize = 1 << 18;

/// The generations kept, a clock that marks each use, and the bytes each tier holds, counted as shapes come and go.
struct Store {
    generations: Vec<Kept>,
    clock: u64,
    bytes: [usize; 2],
}

/// The tier a build at `fineness` is kept in: 0 the default's, 1 the finer.
fn tier(fineness: Fineness) -> usize {
    usize::from(fineness != Fineness::DEFAULT)
}

fn budgets() -> [usize; 2] {
    #[cfg(test)]
    if let Some(b) = BUDGETS.with(std::cell::Cell::get) {
        return [b, b];
    }
    [DEFAULT_BUDGET, BUDGET]
}

impl Store {
    const fn new() -> Self {
        Store { generations: Vec::new(), clock: 0, bytes: [0, 0] }
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// The generations of models gone (closed, or a family editor's drag a step behind): nothing can ask for them again.
    fn prune(&mut self) {
        let bytes = &mut self.bytes;
        self.generations.retain(|g| {
            let alive = g.families.alive();
            if !alive {
                for shape in g.shapes.iter() {
                    bytes[tier(shape.0.1)] -= shape.1.bytes;
                }
            }
            alive
        });
    }

    /// Where `model`'s generation is, made if it isn't, the least lately used going if there are too many.
    fn generation(&mut self, model: &Model, now: u64) -> usize {
        if let Some(i) = self.generations.iter().position(|g| g.families.is_of(&model.families)) {
            self.generations[i].used = now;
            return i;
        }
        if self.generations.len() >= GENERATIONS
            && let Some(oldest) = (0..self.generations.len()).min_by_key(|i| self.generations[*i].used)
        {
            let gone = self.generations.swap_remove(oldest);
            for ((_, fineness), shape) in gone.shapes {
                self.bytes[tier(fineness)] -= shape.bytes;
            }
        }
        self.generations.push(Kept { families: model.families.identity(), shapes: HashMap::new(), envelopes: HashMap::new(), used: now });
        self.generations.len() - 1
    }

    /// Over a tier's budget: its least lately used shapes go, of every generation, until a quarter of it is free.
    fn evict(&mut self) {
        let budgets = budgets();
        for t in 0..2 {
            if self.bytes[t] <= budgets[t] {
                continue;
            }
            let mut by_use: Vec<(u64, usize, (Arc<Key>, Fineness))> = self
                .generations
                .iter()
                .enumerate()
                .flat_map(|(i, g)| g.shapes.iter().filter(|(k, _)| tier(k.1) == t).map(move |(k, s)| (s.used, i, k.clone())))
                .collect();
            by_use.sort_by_key(|(u, ..)| *u);
            for (_, i, k) in by_use {
                if self.bytes[t] <= budgets[t] / 4 * 3 {
                    break;
                }
                if let Some(s) = self.generations.get_mut(i).and_then(|g| g.shapes.remove(&k)) {
                    self.bytes[t] -= s.bytes;
                }
            }
        }
    }
}

#[cfg(not(test))]
static KEPT: Mutex<Store> = Mutex::new(Store::new());

// Tests keep their own, each on its thread, so one test's drawings never push out another's.
#[cfg(test)]
thread_local! {
    static KEPT: std::cell::RefCell<Store> = const { std::cell::RefCell::new(Store::new()) };
}

/// `f` of the store; none where it can't be had (a thread that panicked holding it).
fn with_store<R>(f: impl FnOnce(&mut Store) -> R) -> Option<R> {
    #[cfg(not(test))]
    {
        KEPT.lock().ok().map(|mut s| f(&mut s))
    }
    #[cfg(test)]
    {
        KEPT.with(|k| k.try_borrow_mut().ok().map(|mut s| f(&mut s)))
    }
}

fn kept(model: &Model, key: &(Arc<Key>, Fineness)) -> Option<Arc<Built>> {
    with_store(|s| {
        let now = s.tick();
        let g = s.generations.iter_mut().find(|g| g.families.is_of(&model.families))?;
        g.used = now;
        let shape = g.shapes.get_mut(key)?;
        shape.used = now;
        Some(Arc::clone(&shape.built))
    })
    .flatten()
}

fn keep(model: &Model, shapes: &[Keeping]) {
    with_store(|s| {
        let now = s.tick();
        s.prune();
        let i = s.generation(model, now);
        for (key, built) in shapes {
            // A finer build that is its default one, shared, takes no more than its place.
            let shared = key.1 != Fineness::DEFAULT
                && s.generations[i].shapes.get(&(Arc::clone(&key.0), Fineness::DEFAULT)).is_some_and(|d| Arc::ptr_eq(&d.built, built));
            let bytes = if shared { std::mem::size_of::<Shape>() } else { estimate(&built.pieces) };
            if let Some(was) = s.generations[i].shapes.insert(key.clone(), Shape { built: Arc::clone(built), bytes, used: now }) {
                s.bytes[tier(key.1)] -= was.bytes;
            }
            s.bytes[tier(key.1)] += bytes;
        }
        s.evict();
    });
}

/// `key`'s envelope, where it is kept: `Some(None)` for one known to be unknown.
fn kept_envelope(model: &Model, key: &Arc<Key>) -> Option<Option<[[f64; 3]; 2]>> {
    with_store(|s| s.generations.iter().find(|g| g.families.is_of(&model.families))?.envelopes.get(key).copied()).flatten()
}

fn keep_envelopes(model: &Model, envelopes: &[(Arc<Key>, Envelope)]) {
    with_store(|s| {
        let now = s.tick();
        let i = s.generation(model, now);
        let g = &mut s.generations[i];
        if g.envelopes.len() + envelopes.len() > ENVELOPES {
            g.envelopes.clear();
        }
        g.envelopes.extend(envelopes.iter().cloned());
    });
}

#[cfg(test)]
thread_local! {
    static FINE_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DEFAULT_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static COARSE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static UNCULLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static BUDGETS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static BUILD_THREADS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// The workers a test asks for: none asked, the plan's; `Some(None)`, this thread alone.
    static TEST_POOL: std::cell::RefCell<Option<Option<Arc<rayon::ThreadPool>>>> = const { std::cell::RefCell::new(None) };
}

/// `f` with `cell` set to `value`, and back to what it was however `f` ends.
#[cfg(test)]
fn with_cell<V: Copy + 'static, T>(cell: &'static std::thread::LocalKey<std::cell::Cell<V>>, value: V, f: impl FnOnce() -> T) -> T {
    struct Back<V: Copy + 'static>(&'static std::thread::LocalKey<std::cell::Cell<V>>, V);
    impl<V: Copy + 'static> Drop for Back<V> {
        fn drop(&mut self) {
            self.0.with(|c| c.set(self.1));
        }
    }
    let _back = Back(cell, cell.with(|c| c.replace(value)));
    f()
}

/// `f`, with the plans it draws on this thread built at the default however near they are: as before GEO-2c, to compare.
#[cfg(test)]
pub(crate) fn without_fineness<T>(f: impl FnOnce() -> T) -> T {
    with_cell(&COARSE, true, f)
}

/// `f`, with the plans it draws on this thread building and drawing every component wherever it is: as before GEO-2g, to
/// compare.
#[cfg(test)]
pub(crate) fn without_culling<T>(f: impl FnOnce() -> T) -> T {
    with_cell(&UNCULLED, true, f)
}

/// `f`, with what it keeps on this thread kept within `bytes` a tier.
#[cfg(test)]
pub(crate) fn with_budget<T>(bytes: usize, f: impl FnOnce() -> T) -> T {
    with_cell(&BUDGETS, Some(bytes), f)
}

/// `f`, with the plans it draws on this thread built on `threads` workers of their own (one: on this thread).
#[cfg(test)]
pub(crate) fn with_threads<T>(threads: usize, f: impl FnOnce() -> T) -> T {
    type Pool = Option<Option<Arc<rayon::ThreadPool>>>;
    struct Back(Pool);
    impl Drop for Back {
        fn drop(&mut self) {
            let was = self.0.take();
            TEST_POOL.with(|p| *p.borrow_mut() = was);
        }
    }
    let pool = (threads > 1).then(|| rayon::ThreadPoolBuilder::new().num_threads(threads).build().ok().map(Arc::new)).flatten();
    BUILD_THREADS.with(|b| b.set(0));
    let _back = Back(TEST_POOL.with(|p| p.replace(Some(pool))));
    f()
}

/// The most threads one batch of builds was spread over on this thread, since [`with_threads`] last began.
#[cfg(test)]
pub(crate) fn build_threads() -> usize {
    BUILD_THREADS.with(std::cell::Cell::get)
}

/// A value of a component's that makes its build panic, in tests.
#[cfg(test)]
pub(crate) const PANIC_PROBE: f64 = -4242.0;

/// How many components this thread has built finer than the default, not finding them kept.
#[cfg(test)]
pub(crate) fn fine_builds() -> usize {
    FINE_BUILDS.with(std::cell::Cell::get)
}

/// How many components this thread has built, at the default or finer, not finding them kept.
#[cfg(test)]
pub(crate) fn builds() -> usize {
    fine_builds() + DEFAULT_BUILDS.with(std::cell::Cell::get)
}

/// Whether a generation of `families` is kept.
#[cfg(test)]
pub(crate) fn keeps(families: &crate::shared::Identity<BTreeMap<String, crate::family::Family>>) -> bool {
    with_store(|s| s.generations.iter().any(|g| g.families.is(families))).unwrap_or(false)
}

/// How many finer builds are kept as their default ones, shared, and how many of their own.
#[cfg(test)]
pub(crate) fn kept_finer() -> (usize, usize) {
    with_store(|s| {
        let mut out = (0, 0);
        for g in &s.generations {
            for ((key, fineness), shape) in g.shapes.iter().filter(|(k, _)| k.1 != Fineness::DEFAULT) {
                let _ = fineness;
                match g.shapes.get(&(Arc::clone(key), Fineness::DEFAULT)) {
                    Some(d) if Arc::ptr_eq(&d.built, &shape.built) => out.0 += 1,
                    _ => out.1 += 1,
                }
            }
        }
        out
    })
    .unwrap_or_default()
}

/// The bytes kept, each tier's, as counted and as summed afresh.
#[cfg(test)]
pub(crate) fn kept_bytes() -> ([usize; 2], [usize; 2]) {
    with_store(|s| {
        let mut summed = [0, 0];
        for (k, shape) in s.generations.iter().flat_map(|g| g.shapes.iter()) {
            summed[tier(k.1)] += shape.bytes;
        }
        (s.bytes, summed)
    })
    .unwrap_or_default()
}
