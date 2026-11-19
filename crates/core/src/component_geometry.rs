//! Per-operation reuse of family geometry, bound to an immutable model snapshot.
mod index;

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

pub(crate) fn local_envelope(m: &Model, c: &Component) -> Option<[[f64; 3]; 2]> {
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
    /// What its curves recorded as it was built: which finenesses would build it otherwise (cp's review of #88).
    pub(crate) needs: crate::family::fineness::Needs,
    /// FE-16c-2: its faces that face up, found from its triangles the first time a plan asks for a painted one's pattern, then
    /// kept with it, so a redraw or a pan finds them again in none of its triangles ([`Built::up_faces`]).
    up: std::sync::OnceLock<Arc<Vec<UpFace>>>,
    /// FE-16c-2: the lines of each model fill pattern a plan has drawn over its faces that face up, each face's in its family's
    /// axes, kept by the pattern ([`Built::pattern_lines`]).
    lines: Mutex<Vec<(crate::fills::FillPattern, Arc<PatternLines>)>>,
}

/// FE-16c-2: a fill pattern's lines over each of a build's faces that face up, in its family's axes.
pub(crate) type PatternLines = Vec<Vec<[P2; 2]>>;

/// FE-16c-2: one face of a built piece that faces up within a degree, as a plan sees it: the piece it is of (its place in
/// the build), its key, its family's paint of it (FE-16c-1; empty, none), its height, and what it covers in plan, as its
/// triangles (convex, for cutting what stands over it out) and as the rings round them (for its pattern's lines).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UpFace {
    pub(crate) piece: usize,
    pub(crate) face: crate::family::FormFace,
    pub(crate) paint: String,
    pub(crate) z: f64,
    pub(crate) tris: Vec<Vec<P2>>,
    pub(crate) rings: Vec<Vec<P2>>,
    /// What of it a plan sees where every piece of its build is drawn: less what a higher piece, or a higher face of its own
    /// piece, covers.
    pub(crate) shown: Vec<Vec<P2>>,
}

impl UpFace {
    /// What of it a plan sees under `pieces` (the ones it draws, each piece of its build's place among them in `at`): less the
    /// outlines of the other pieces higher than it, and its own piece's faces among `up` higher than it.
    pub(crate) fn seen(&self, pieces: &[&Piece], at: &[Option<usize>], up: &[UpFace]) -> Vec<Vec<P2>> {
        let mine = at.get(self.piece).copied().flatten();
        let meets = |ring: &[P2]| self.rings.iter().any(|r| boxes_meet(r, ring));
        let over: Vec<&[P2]> = pieces
            .iter()
            .enumerate()
            .filter(|(j, q)| Some(*j) != mine && q.top > self.z + 1e-6)
            .map(|(_, q)| q.outline.as_slice())
            .chain(up.iter().filter(|v| v.piece == self.piece && v.z > self.z + 1e-6).flat_map(|v| v.tris.iter().map(Vec::as_slice)))
            .filter(|ring| meets(ring))
            .collect();
        if over.is_empty() {
            return self.rings.clone();
        }
        over.iter().fold(self.tris.clone(), |left, ring| crate::walls::minus(&left, ring))
    }
}

/// Whether the boxes round two sets of points overlap.
fn boxes_meet(a: &[P2], b: &[P2]) -> bool {
    let bounds = |pts: &[P2]| {
        pts.iter().fold((P2(f64::INFINITY, f64::INFINITY), P2(f64::NEG_INFINITY, f64::NEG_INFINITY)), |(lo, hi), q| {
            (P2(lo.0.min(q.0), lo.1.min(q.1)), P2(hi.0.max(q.0), hi.1.max(q.1)))
        })
    };
    let ((alo, ahi), (blo, bhi)) = (bounds(a), bounds(b));
    alo.0 <= bhi.0 && blo.0 <= ahi.0 && alo.1 <= bhi.1 && blo.1 <= ahi.1
}

/// FE-16c-2: a component's faces that face up, as a plan draws its painted ones' patterns: its build's, in its family's axes,
/// and where it is placed (upright), or found from its pieces placed (tilted).
pub(crate) enum Tops {
    Local(Arc<Built>, crate::hosting::Placement),
    Placed(Vec<UpFace>),
}

/// How near straight up a face's normal is to be for a plan to draw its pattern: within a degree.
pub(crate) fn facing(n: [f64; 3], toward: [f64; 3]) -> bool {
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    len > 1e-12 && (n[0] * toward[0] + n[1] * toward[1] + n[2] * toward[2]) / len >= 1f64.to_radians().cos()
}

/// The faces of `pieces` that face up within a degree ([`UpFace`]), each piece's by its key; a piece with no keys, its
/// triangles one face, `all`.
pub(crate) fn up_faces(pieces: &[Piece]) -> Vec<UpFace> {
    use crate::view3d::{cross, sub};
    let mut out: Vec<UpFace> = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        let tagged = p.faces.len() == p.tris.len();
        let mut by: BTreeMap<u32, usize> = BTreeMap::new();
        for (j, t) in p.tris.iter().enumerate() {
            if !facing(cross(sub(t[1], t[0]), sub(t[2], t[0])), [0.0, 0.0, 1.0]) {
                continue;
            }
            let k = if tagged { p.faces[j] } else { u32::MAX };
            let at = *by.entry(k).or_insert_with(|| {
                let face = if tagged {
                    p.keys.get(k as usize).cloned().unwrap_or(crate::family::FormFace::All)
                } else {
                    crate::family::FormFace::All
                };
                let paint = if tagged { p.paint.get(k as usize).cloned().unwrap_or_default() } else { String::new() };
                out.push(UpFace { piece: i, face, paint, z: f64::NEG_INFINITY, tris: Vec::new(), rings: Vec::new(), shown: Vec::new() });
                out.len() - 1
            });
            let u = &mut out[at];
            u.z = u.z.max(t[0][2]).max(t[1][2]).max(t[2][2]);
            let mut ring: Vec<P2> = t.iter().map(|q| P2(q[0], q[1])).collect();
            if crate::geom::area(&ring) < 0.0 {
                ring.reverse();
            }
            u.tris.push(ring);
        }
    }
    for u in &mut out {
        u.rings = boundary(&u.tris);
    }
    // What a plan sees of each with every piece drawn.
    let all: Vec<&Piece> = pieces.iter().collect();
    let at: Vec<Option<usize>> = (0..pieces.len()).map(Some).collect();
    let shown: Vec<Vec<Vec<P2>>> = out.iter().map(|u| u.seen(&all, &at, &out)).collect();
    for (u, shown) in out.iter_mut().zip(shown) {
        u.shown = shown;
    }
    out
}

/// The rings round triangles `tris` (each counter-clockwise), which share their edges' ends exactly: each edge no other
/// triangle has the other way round, joined end to end. The triangles themselves where they don't join up into rings.
fn boundary(tris: &[Vec<P2>]) -> Vec<Vec<P2>> {
    type Corner = (u64, u64);
    let key = |p: P2| -> Corner { (p.0.to_bits(), p.1.to_bits()) };
    let mut edges: HashMap<(Corner, Corner), (P2, P2)> = HashMap::new();
    for t in tris {
        for e in 0..t.len() {
            let (a, b) = (t[e], t[(e + 1) % t.len()]);
            if edges.remove(&(key(b), key(a))).is_none() {
                edges.insert((key(a), key(b)), (a, b));
            }
        }
    }
    let mut next: HashMap<Corner, Vec<(P2, P2)>> = HashMap::new();
    for (a, b) in edges.into_values() {
        next.entry(key(a)).or_default().push((a, b));
    }
    let mut rings = Vec::new();
    while let Some(start) = next.keys().next().copied() {
        let mut ring = Vec::new();
        let mut at = start;
        loop {
            let Some((a, b)) = next.get_mut(&at).and_then(Vec::pop) else { return tris.to_vec() };
            if next.get(&at).is_some_and(Vec::is_empty) {
                next.remove(&at);
            }
            ring.push(a);
            at = key(b);
            if at == start {
                break;
            }
            if ring.len() > 4 * tris.len() + 4 {
                return tris.to_vec();
            }
        }
        rings.push(ring);
    }
    rings
}

impl Built {
    pub(crate) fn new(pieces: Vec<Piece>, needs: crate::family::fineness::Needs) -> Self {
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for p in pieces.iter().flat_map(|pc| pc.tris.iter().flatten()) {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let heights = pieces.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.bottom), hi.max(p.top)));
        Self {
            bounds: (lo[0] <= hi[0]).then_some([lo, hi]),
            heights,
            pieces,
            needs,
            up: std::sync::OnceLock::new(),
            lines: Mutex::new(Vec::new()),
        }
    }

    /// The lines model fill pattern `p` draws over what a plan sees of each of its faces that face up ([`UpFace::shown`]), in
    /// its family's axes from their zero, found once a pattern and kept with it. None where the pattern has more lines than a
    /// view can hold.
    pub(crate) fn pattern_lines(&self, p: &crate::fills::FillPattern) -> Option<Arc<PatternLines>> {
        let mut kept = self.lines.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((_, lines)) = kept.iter().find(|(q, _)| q == p) {
            return Some(Arc::clone(lines));
        }
        let mut out = Vec::new();
        for u in self.up_faces().iter() {
            let [mut lo, mut hi] = [P2(f64::INFINITY, f64::INFINITY), P2(f64::NEG_INFINITY, f64::NEG_INFINITY)];
            for q in u.shown.iter().flatten() {
                (lo, hi) = (P2(lo.0.min(q.0), lo.1.min(q.1)), P2(hi.0.max(q.0), hi.1.max(q.1)));
            }
            out.push(crate::fills::strokes(p, &u.shown, 1.0, (lo, hi), 50_000)?);
        }
        let out = Arc::new(out);
        // A few patterns a build: the oldest goes past them.
        if kept.len() >= 4 {
            kept.remove(0);
        }
        kept.push((p.clone(), Arc::clone(&out)));
        Some(out)
    }

    /// Its faces that face up ([`up_faces`]), found once and kept with it.
    pub(crate) fn up_faces(&self) -> Arc<Vec<UpFace>> {
        Arc::clone(self.up.get_or_init(|| Arc::new(up_faces(&self.pieces))))
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
    /// Its envelope placed, once found (none inside: not known); from the index, where it was a candidate of it.
    placed: Option<Option<[[f64; 3]; 2]>>,
    /// Its model lines in its family's axes, once found.
    lines: Option<Arc<Vec<index::Line>>>,
}

impl Seen {
    fn new(key: Arc<Key>) -> Self {
        Seen { key, inside: None, shown: None, placed: None, lines: None }
    }
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
    /// Each key's model lines in its family's axes (ec's review of #88: found once a key, not once a question).
    lines: HashMap<Arc<Key>, Arc<Vec<index::Line>>>,
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
            lines: HashMap::new(),
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
        self.seen.entry(std::ptr::from_ref(c) as usize).or_insert_with(|| {
            #[cfg(test)]
            KEYS.with(|k| k.set(k.get() + 1));
            Seen::new(Arc::new(model.shape_key(c)))
        })
    }

    /// GEO-2g-2: the model's components that may show in a floor plan of `level` whose cut plane and view depth are `range`,
    /// by id, from the model's index ([`index::Index`]): those near its window, and what is found of each kept for the
    /// drawing, so none of them has its key made again. Every component where the index can't be had.
    pub(crate) fn candidates(&mut self, level: crate::ids::Guid, range: (f64, f64)) -> std::collections::BTreeSet<Id> {
        let model = self.model;
        #[cfg(test)]
        let model_index = if UNINDEXED.with(std::cell::Cell::get) { None } else { index_of(model) };
        #[cfg(not(test))]
        let model_index = index_of(model);
        let Some(index) = model_index else {
            return model.elements.iter().filter(|(_, e)| matches!(e, crate::model::Element::Component(_))).map(|(id, _)| *id).collect();
        };
        let found = index.query(index::Query { window: self.window, range: Some(range), level });
        for &id in &found {
            let (Some(crate::model::Element::Component(c)), Some(e)) = (model.elements.get(id), index.get(id)) else { continue };
            let seen = self.seen.entry(std::ptr::from_ref(c) as usize).or_insert_with(|| Seen::new(Arc::clone(&e.key)));
            seen.placed.get_or_insert(e.forms);
            seen.lines.get_or_insert_with(|| Arc::clone(&e.local.lines));
            self.envelopes.entry(Arc::clone(&e.key)).or_insert(e.local.envelope);
        }
        found
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
        // cp's review of #88: one whose curves are all within the fineness at the default is its default build, not built
        // again to find that out.
        if fineness != Fineness::DEFAULT {
            let default = self.local_at(key, c, Fineness::DEFAULT);
            if !fineness.changes(default.needs) {
                self.local.insert((Arc::clone(key), fineness), Arc::clone(&default));
                return default;
            }
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
        if let Some(placed) = self.seen(c).placed {
            return placed;
        }
        let placed = self.find_placed_envelope(c);
        self.seen(c).placed = Some(placed);
        placed
    }

    fn find_placed_envelope(&mut self, c: &'a Component) -> Option<[[f64; 3]; 2]> {
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

    /// GEO-2g: whether `c`'s forms meet the plan's window: by its default build's box where that is at hand; else not where
    /// its envelope is known to miss the window; else by the box of its default build, built. A component of no geometry
    /// meets it (there is nothing of it to build). Its model lines are asked apart ([`Components::lines_in_window`], ec's
    /// review of #88): one whose lines alone reach the window draws them, and isn't built for them.
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

    /// Whether any of `c`'s model lines (FE-15), at any detail, meets the plan's window: a model line may reach far past its
    /// family's solids, and a plan draws it wherever it reaches. False for a family of none; true for every component of a
    /// plan with no window. Found without building.
    pub(crate) fn lines_in_window(&mut self, c: &'a Component) -> bool {
        let Some(window) = self.window else { return true };
        self.placed_lines(c).iter().any(|[a, b]| meets([P2(a[0].min(b[0]), a[1].min(b[1])), P2(a[0].max(b[0]), a[1].max(b[1]))], window))
    }

    /// Whether `c` has a model line in the plan's window that may reach from a floor plan's view depth `depth` to its cut
    /// plane `cut` (each a hair past): one it may draw.
    pub(crate) fn lines_reach(&mut self, c: &'a Component, cut: f64, depth: f64) -> bool {
        let window = self.window;
        self.placed_lines(c).iter().any(|[a, b]| {
            let inside = window.is_none_or(|w| meets(w, [P2(a[0].min(b[0]), a[1].min(b[1])), P2(a[0].max(b[0]), a[1].max(b[1]))]));
            inside && a[2].min(b[2]) <= cut + crate::model::TOL && a[2].max(b[2]) >= depth - crate::model::TOL
        })
    }

    /// `c`'s model lines placed, as [`Model::component_model_line_parts`] places them: their ends in its family's axes found
    /// once a key (ec's review of #88), placed here.
    fn placed_lines(&mut self, c: &'a Component) -> Vec<index::Line> {
        if self.model.families.get(&c.family).is_none_or(|f| f.model_lines.is_empty()) {
            return Vec::new();
        }
        let local = match &self.seen(c).lines {
            Some(l) => Arc::clone(l),
            None => {
                let key = Arc::clone(&self.seen(c).key);
                let model = self.model;
                let l = Arc::clone(self.lines.entry(key).or_insert_with(|| Arc::new(index::local_lines(model, c))));
                self.seen(c).lines = Some(Arc::clone(&l));
                l
            }
        };
        let placed = self.model.component_placement(c);
        local.iter().map(|[a, b]| [placed.place(*a), placed.place(*b)]).collect()
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
            // Its curves within the fineness at the default: drawn with its default build, nothing built.
            if self.known(&key, Fineness::DEFAULT).is_some_and(|d| !fineness.changes(d.needs)) {
                continue;
            }
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

    /// What a plan draws of its pieces where it is placed: upright, each piece's outline, holes, heights and what says how it
    /// is drawn, as [`Components::pieces`] places them, but none of its triangles, which a plan doesn't draw (GEO-2g-2);
    /// tilted, its pieces, whose outlines are found from their triangles.
    pub(crate) fn plan_pieces(&mut self, c: &'a Component) -> Vec<Piece> {
        let placed = self.model.component_placement(c);
        if !placed.upright {
            return self.pieces(c);
        }
        let flat = |q: &P2| {
            let p = placed.place([q.0, q.1, 0.0]);
            P2(p[0], p[1])
        };
        let base = placed.origin[2];
        let pieces: Vec<Piece> = self
            .shown(c)
            .pieces
            .iter()
            .map(|pc| Piece {
                tris: Vec::new(),
                caps: pc.caps,
                outline: pc.outline.iter().map(flat).collect(),
                holes: pc.holes.iter().map(|h| h.iter().map(flat).collect()).collect(),
                bottom: base + pc.bottom,
                top: base + pc.top,
                material: pc.material.clone(),
                round: pc.round,
                vis: pc.vis,
                nested: pc.nested.clone(),
                faces: Vec::new(),
                keys: Vec::new(),
                paint: Vec::new(),
                form: pc.form,
                path: pc.path.clone(),
            })
            .collect();
        #[cfg(test)]
        for (a, b) in pieces.iter().zip(self.pieces(c)) {
            assert!(
                a.outline == b.outline
                    && a.holes == b.holes
                    && (a.bottom, a.top) == (b.bottom, b.top)
                    && a.vis == b.vis
                    && a.nested == b.nested
            );
        }
        pieces
    }

    /// FE-16c-2: its faces that face up ([`UpFace`]), each piece's as [`Components::plan_pieces`] numbers them: upright, its
    /// build's in its family's axes, found once a build and kept with it, with the placement that puts them in the model;
    /// tilted, found from its pieces placed.
    pub(crate) fn plan_up_faces(&mut self, c: &'a Component) -> Tops {
        let placed = self.model.component_placement(c);
        if !placed.upright {
            return Tops::Placed(up_faces(&self.pieces(c)));
        }
        Tops::Local(self.shown(c), placed)
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
        panic!("a probe's build panicked: W {}", c.params.get("W").map_or(String::new(), |w| w.to_string()));
    }
    let (pieces, needs) = crate::family::fineness::recording(|| fineness.scoped(|| model.component_local_pieces(c)));
    Built::new(pieces, needs)
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
            Some(Some(pool)) => in_order(pool.install(|| jobs.par_iter().map(|j| caught(&g, j)).collect())),
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
        Some(pool) if jobs.len() > 1 && pool.current_num_threads() > 1 => {
            in_order(pool.install(|| jobs.par_iter().map(|j| caught(&f, j)).collect()))
        }
        _ => jobs.iter().map(f).collect(),
    }
}

/// `f` of `j`, a panic in it caught.
fn caught<J, T>(f: &(impl Fn(&J) -> T + Sync), j: &J) -> std::thread::Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(j)))
}

/// What each job came to, in their order; where any panicked, the first of them in that order panics again, as the loop on
/// one thread would have (ec's review of #88).
fn in_order<T>(results: Vec<std::thread::Result<T>>) -> Vec<T> {
    let mut out = Vec::with_capacity(results.len());
    for r in results {
        match r {
            Ok(v) => out.push(v),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
    out
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
    /// The finenesses of each key's finer builds kept: what a default build gone may have shared.
    finer: HashMap<Arc<Key>, Vec<Fineness>>,
    envelopes: HashMap<Arc<Key>, Option<[[f64; 3]; 2]>>,
    /// GEO-2g-2: what a plan reads of each component of a model of these families before building any, and where each
    /// stands ([`index::Index`]): taken out while it is brought up to a model, put back after.
    index: Option<Arc<index::Index>>,
    used: u64,
}

impl Kept {
    fn new(families: crate::shared::Identity<BTreeMap<String, crate::family::Family>>, used: u64) -> Self {
        Kept { families, shapes: HashMap::new(), finer: HashMap::new(), envelopes: HashMap::new(), index: None, used }
    }

    /// ec's review of #88: the default builds `gone` no longer kept, the finer builds of their keys that shared them charged
    /// what they take, since they now keep them alive on their own. Once an eviction pass, each key's finer builds looked up
    /// by key, not found among all the generation's. The bytes that adds.
    fn unshare(&mut self, gone: &[Gone]) -> usize {
        count_unshare(0);
        let mut added = 0;
        for (key, built) in gone {
            for fineness in self.finer.get(key).into_iter().flatten() {
                count_unshare(1);
                let Some(shape) = self.shapes.get_mut(&(Arc::clone(key), *fineness)) else { continue };
                if Arc::ptr_eq(&shape.built, built) {
                    let full = estimate(&shape.built.pieces);
                    added += full.saturating_sub(shape.bytes);
                    shape.bytes = shape.bytes.max(full);
                }
            }
        }
        added
    }

    /// `key`'s finer build at `fineness` gone.
    fn forget_finer(&mut self, key: &Arc<Key>, fineness: Fineness) {
        if let Some(list) = self.finer.get_mut(key) {
            list.retain(|f| *f != fineness);
            if list.is_empty() {
                self.finer.remove(key);
            }
        }
    }
}

/// An unsharing pass (`looked` 0) or a finer build looked at in one, counted on this thread.
fn count_unshare(looked: usize) {
    #[cfg(test)]
    UNSHARES.with(|u| {
        let (passes, looks) = u.get();
        u.set(if looked == 0 { (passes + 1, looks) } else { (passes, looks + looked) })
    });
    #[cfg(not(test))]
    let _ = looked;
}

/// A shape kept: its build, about how many bytes it takes, and when it was last used.
struct Shape {
    built: Arc<Built>,
    bytes: usize,
    used: u64,
}

/// A default build gone from the store, with its key.
type Gone = (Arc<Key>, Arc<Built>);

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
        return b;
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
        self.generations.push(Kept::new(model.families.identity(), now));
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
            // The default builds gone, by generation: their finer builds unshared once the pass is over.
            let mut gone: BTreeMap<usize, Vec<Gone>> = BTreeMap::new();
            for (_, i, k) in by_use {
                if self.bytes[t] <= budgets[t] / 4 * 3 {
                    break;
                }
                if let Some(g) = self.generations.get_mut(i)
                    && let Some(s) = g.shapes.remove(&k)
                {
                    self.bytes[t] -= s.bytes;
                    if k.1 == Fineness::DEFAULT {
                        gone.entry(i).or_default().push((k.0, s.built));
                    } else {
                        g.forget_finer(&k.0, k.1);
                    }
                }
            }
            for (i, gone) in gone {
                if let Some(g) = self.generations.get_mut(i) {
                    self.bytes[1] += g.unshare(&gone);
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
            let g = &mut s.generations[i];
            match g.shapes.insert(key.clone(), Shape { built: Arc::clone(built), bytes, used: now }) {
                Some(was) => {
                    s.bytes[tier(key.1)] -= was.bytes;
                    if key.1 == Fineness::DEFAULT && !Arc::ptr_eq(&was.built, built) {
                        s.bytes[1] += g.unshare(&[(Arc::clone(&key.0), was.built)]);
                    }
                }
                None if key.1 != Fineness::DEFAULT => g.finer.entry(Arc::clone(&key.0)).or_default().push(key.1),
                None => {}
            }
            s.bytes[tier(key.1)] += bytes;
        }
        s.evict();
    });
}

/// GEO-2g-2: the index of `model`'s components, brought up to it: kept with its families' generation, so a drawing after
/// an edit finds again only what changed. None where the store can't be had.
fn index_of(model: &Model) -> Option<Arc<index::Index>> {
    let had = with_store(|s| {
        let now = s.tick();
        s.prune();
        let i = s.generation(model, now);
        s.generations[i].index.take()
    })?;
    let mut index = had.unwrap_or_else(|| Arc::new(index::Index::of(model)));
    Arc::make_mut(&mut index).bring_up(model);
    with_store(|s| {
        let now = s.tick();
        let i = s.generation(model, now);
        s.generations[i].index = Some(Arc::clone(&index));
    });
    Some(index)
}

/// The index kept for `model`'s families, brought up to it: what a test checks against one made afresh.
#[cfg(test)]
pub(crate) fn index_is_made_afresh(model: &Model) -> bool {
    index_of(model).is_some_and(|i| i.same_as_made(model))
}

/// How many entries of the index's cells this thread has looked at to take components out of them.
#[cfg(test)]
pub(crate) fn index_scanned() -> usize {
    index::scanned()
}

/// How many entries of the index this thread has found, cells it has looked in and candidates it has looked at.
#[cfg(test)]
pub(crate) fn index_counts() -> (usize, usize, usize) {
    index::counts()
}

/// How many components' keys this thread's drawings have made, not finding them in the index.
#[cfg(test)]
pub(crate) fn keys_made() -> usize {
    KEYS.with(std::cell::Cell::get)
}

/// How many unsharing passes this thread has made, and finer builds it has looked at in them.
#[cfg(test)]
pub(crate) fn unshares() -> (usize, usize) {
    UNSHARES.with(std::cell::Cell::get)
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
    static UNINDEXED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static BUDGETS: std::cell::Cell<Option<[usize; 2]>> = const { std::cell::Cell::new(None) };
    static BUILD_THREADS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static UNSHARES: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    static KEYS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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

/// `f`, with the plans it draws on this thread finding their components among all of them, not through the index: as
/// before GEO-2g-2, to compare.
#[cfg(test)]
pub(crate) fn without_index<T>(f: impl FnOnce() -> T) -> T {
    with_cell(&UNINDEXED, true, f)
}

/// `f`, with what it keeps on this thread kept within `bytes` a tier.
#[cfg(test)]
pub(crate) fn with_budget<T>(bytes: usize, f: impl FnOnce() -> T) -> T {
    with_budgets([bytes, bytes], f)
}

/// `f`, with what it keeps on this thread kept within `bytes`: the default builds' tier and the finer ones'.
#[cfg(test)]
pub(crate) fn with_budgets<T>(bytes: [usize; 2], f: impl FnOnce() -> T) -> T {
    with_cell(&BUDGETS, Some(bytes), f)
}

/// Whether each finer build kept is charged what it takes, but where its default one, kept, shares it.
#[cfg(test)]
pub(crate) fn kept_finer_honest() -> bool {
    with_store(|s| {
        s.generations.iter().all(|g| {
            g.shapes.iter().filter(|(k, _)| k.1 != Fineness::DEFAULT).all(|((key, _), shape)| {
                let shared = g.shapes.get(&(Arc::clone(key), Fineness::DEFAULT)).is_some_and(|d| Arc::ptr_eq(&d.built, &shape.built));
                shared || shape.bytes == estimate(&shape.built.pieces)
            })
        })
    })
    .unwrap_or(false)
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
