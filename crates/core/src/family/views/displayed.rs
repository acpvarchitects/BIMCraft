//! GEO-2c-2e: what a family's plan or elevation drew, kept with its staged model so a click, an edge click and a box take what
//! the view shows: each piece of a form or a nested instance as the drawing built it (refined for the zoom, GEO-2c, or its
//! default build where that is too fine to cut), with, in plan, the strokes it was drawn with. The scratch model may hold a
//! second, halftone placement of what the type hides: its pieces name the same forms. (Ported from the upstream series'
//! picker, ec's 7f768c09e, e23c403b0, 7e3789a49, d19ae07d8 and 68be32306.)

use super::*;
use std::sync::OnceLock;

/// The pieces a view drew, each as it drew it.
#[derive(Default)]
pub(crate) struct DisplayedForms {
    pieces: Vec<DisplayedPiece>,
    /// In a door's or a window's elevation, the wall it stands in as the picture shows it: what hides a pick behind it.
    host: Option<super::displayed_host::HostCover>,
}

struct DisplayedPiece {
    /// The GUID of the form it is of (in the family whose placement drew it) and the nested instances it is part of.
    form: Guid,
    path: Vec<Guid>,
    /// Its triangles, placed: in plan, what the plan sees of it (below a hosted opening's cut, less what the wall covers,
    /// with the cut's caps); in an elevation, what the picture was drawn from.
    tris: Vec<[P3; 3]>,
    /// Several meshes of one selectable instance share their crease topology and clip each other's rims at the true depth.
    grouped: bool,
    /// Its edges as the view shows them, in the view's millimetres: in plan, the strokes it was drawn with; in an elevation,
    /// its seen edges less where another piece lies nearer the eye, worked out when first asked.
    edges: OnceLock<Vec<(P2, P2)>>,
    /// Its box in the view's millimetres and depth (larger nearer the eye), and each run of [`PATCH`] of its triangles' box:
    /// what quickly rules out a piece, or part of one, that can't hide an edge.
    bounds: OnceLock<[P3; 2]>,
    patch_bounds: OnceLock<Vec<[P3; 2]>>,
}

/// How many triangles of a piece one box rounds, to rule them out together.
const PATCH: usize = 32;

impl DisplayedForms {
    /// Wall `wall` as `picture`, drawn through `view`, shows it: what of the family lies behind it is no one's to pick.
    pub(crate) fn keep_host(&mut self, view: View, picture: &view3d::Image, wall: Id) {
        self.host = super::displayed_host::HostCover::new(view, picture, wall);
    }

    /// A piece an elevation drew. A nested instance is selected whole, so its pieces are kept as one (as far as
    /// [`MAX_OCCLUDERS`] lets them hide each other's edges): its nearer forms hide its farther ones' rims, as in the picture.
    pub(crate) fn push_mesh(&mut self, form: Guid, path: Vec<Guid>, tris: Vec<[P3; 3]>) {
        if let Some(instance) = path.first()
            && let Some(owner) = self.pieces.iter_mut().rev().find(|p| p.path.first() == Some(instance))
            && owner.tris.len() + tris.len() <= MAX_OCCLUDERS
        {
            owner.tris.extend(tris);
            owner.grouped = true;
            return;
        }
        self.pieces.push(DisplayedPiece {
            form,
            path,
            tris,
            grouped: false,
            edges: OnceLock::new(),
            bounds: OnceLock::new(),
            patch_bounds: OnceLock::new(),
        });
    }

    /// A piece a plan drew, with the strokes it drew it with.
    pub(crate) fn push_plan(&mut self, piece: crate::family::Piece, edges: Vec<(P2, P2)>) {
        self.pieces.push(DisplayedPiece {
            form: piece.form,
            path: piece.path,
            tris: piece.tris,
            grouped: false,
            edges: OnceLock::from(edges),
            bounds: OnceLock::new(),
            patch_bounds: OnceLock::new(),
        });
    }

    /// What of the family the piece on top at `at` is, among those `keep` keeps: in plan the highest, in an elevation the
    /// nearest the eye.
    pub(super) fn face(&self, st: &Staged, which: Which, at: P2, keep: impl Fn(Part) -> bool) -> Option<Part> {
        // Where the picture shows the wall it stands in, nothing of it is seen.
        if self.host.as_ref().is_some_and(|host| host.hides(at)) {
            return None;
        }
        let mut hit = None;
        for piece in &self.pieces {
            if let Some(part) = piece.part(st).filter(|part| keep(*part))
                && let Some(near) = piece.depth(st, which, at)
                && hit.is_none_or(|(best, _)| near > best)
            {
                hit = Some((near, part));
            }
        }
        hit.map(|(_, part)| part)
    }

    /// What of the family `keep` keeps has an edge the view shows within `reach` of `at` ([`DisplayedForms::edges`]): the one
    /// whose edge is nearest.
    pub(super) fn edge(&self, st: &Staged, which: Which, at: P2, reach: f64, keep: impl Fn(Part) -> bool) -> Option<Part> {
        self.pieces
            .iter()
            .filter_map(|piece| {
                let part = piece.part(st).filter(|part| keep(*part))?;
                if let Which::Elevation(_) = which {
                    let [lo, hi] = *piece.bounds(st, which);
                    if at.0 < lo[0] - reach || at.0 > hi[0] + reach || at.1 < lo[1] - reach || at.1 > hi[1] + reach {
                        return None;
                    }
                }
                let distance = self
                    .edges(st, which, piece)
                    .iter()
                    .map(|(a, b)| geom::dist_to_segment(at, *a, *b))
                    .filter(|d| d.is_finite() && *d <= reach)
                    .min_by(f64::total_cmp)?;
                Some((part, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(part, _)| part)
    }

    /// `piece`'s edges as the view shows them: in plan its strokes; in an elevation its seen edges ([`seen_edges`]), each less
    /// the spans another piece lies nearer the eye over, so a click, a box and the outline drawn selected agree with the
    /// picture on what is seen (ec's 68be32306).
    fn edges<'a>(&self, st: &Staged, which: Which, piece: &'a DisplayedPiece) -> &'a [(P2, P2)] {
        piece.edges.get_or_init(|| {
            // A combined nested mesh keeps its common edges for crease classification. Its faces take part in the same
            // exact depth clipping as other pieces, so a narrow visible stretch cannot fall between two self-occlusion samples.
            let occluders: Vec<_> =
                self.pieces.iter().filter(|p| (piece.grouped || !std::ptr::eq(*p, piece)) && p.part(st).is_some()).collect();
            let mut out = Vec::new();
            seen_edges_with(&piece.tris, st, which, !piece.grouped, |a, b| {
                let (a, b) = (projected(st, which, a), projected(st, which, b));
                let mut hidden = Vec::new();
                if let Some(host) = &self.host {
                    host.hide_edge(P2(a[0], a[1]), P2(b[0], b[1]), &mut hidden);
                }
                for other in &occluders {
                    let may_cover = |[lo, hi]: &[P3; 2]| {
                        hi[2] > a[2].min(b[2]) + DEPTH_EPS && (0..2).all(|i| hi[i] >= a[i].min(b[i]) && lo[i] <= a[i].max(b[i]))
                    };
                    if !may_cover(other.bounds(st, which)) {
                        continue;
                    }
                    let bounds =
                        other.patch_bounds.get_or_init(|| other.tris.chunks(PATCH).map(|patch| mesh_bounds(patch, st, which)).collect());
                    for (patch, bounds) in other.tris.chunks(PATCH).zip(bounds) {
                        if may_cover(bounds) {
                            hidden.extend(patch.iter().filter_map(|tri| covered_span(a, b, tri.map(|p| projected(st, which, p)))));
                        }
                    }
                }
                hidden.sort_unstable_by(|x, y| x.0.total_cmp(&y.0));
                let xy = |s: f64| P2(a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s);
                let mut start = 0.0;
                for (lo, hi) in hidden {
                    if lo > start + 1e-10 {
                        out.push((xy(start), xy(lo)));
                    }
                    start = f64::max(start, hi);
                }
                if start < 1.0 - 1e-10 {
                    out.push((xy(start), xy(1.0)));
                }
            });
            out
        })
    }

    /// The edges `part` is shown with ([`DisplayedForms::edges`]): what it is drawn selected with and boxed by.
    pub(super) fn outline(&self, st: &Staged, which: Which, part: Part) -> Vec<(P2, P2)> {
        self.pieces.iter().filter(|p| p.part(st) == Some(part)).flat_map(|p| self.edges(st, which, p).iter().copied()).collect()
    }

    /// Whether `boxed` takes `part` by what the view drew of it, as [`face_in_box`] says.
    pub(super) fn in_box(&self, st: &Staged, which: Which, part: Part, boxed: &geom::SelectBox) -> bool {
        let mut tris = self.pieces.iter().filter(|p| p.part(st) == Some(part)).flat_map(|p| &p.tris).peekable();
        if tris.peek().is_none() {
            return false;
        }
        if boxed.crossing {
            tris.any(|t| {
                let tri = t.map(|p| st.to_view(which, p));
                boxed.touches(&tri) && self.host.as_ref().is_none_or(|host| host.crosses(boxed, &tri))
            })
        } else {
            tris.all(|t| t.iter().all(|p| boxed.holds(st.to_view(which, *p))))
        }
    }
}

impl DisplayedPiece {
    /// The family's form or nested instance it is of: a nested instance's by the first of its path.
    fn part(&self, st: &Staged) -> Option<Part> {
        match self.path.first() {
            Some(first) => st.instance_ids.get(first).copied().map(Part::Nested),
            None => st.form_ids.get(&self.form).copied().map(Part::Form),
        }
    }

    /// Its box in the view ([`mesh_bounds`]).
    fn bounds(&self, st: &Staged, which: Which) -> &[P3; 2] {
        self.bounds.get_or_init(|| mesh_bounds(&self.tris, st, which))
    }

    /// How near the eye it is at `at` (in plan, how high), where it covers it.
    fn depth(&self, st: &Staged, which: Which, at: P2) -> Option<f64> {
        self.tris
            .iter()
            .filter_map(|tri| {
                let projected = tri.map(|p| {
                    let [x, y, z] = projected(st, which, p);
                    (x, y, z)
                });
                depth_at(projected, (at.0, at.1))
            })
            .max_by(f64::total_cmp)
    }
}

impl Staged {
    /// Component `placement` of the scratch model is a copy of the family placed apart from it (the halftone ghost of what
    /// its type hides, a form drawn with graphics of its own, FE-18), its forms and nested instances in the family's order: its
    /// pieces are of the family's forms and nested instances, whatever GUIDs the copy's were given when it was placed.
    pub fn identify_copy(&mut self, placement: Id) {
        let Some(Element::Component(c)) = self.model.elements.get(placement) else { return };
        let Some(copy) = self.model.families.get(&c.family) else { return };
        self.form_ids.extend(copy.forms.iter().enumerate().filter(|(_, f)| !f.guid.is_nil()).map(|(i, f)| (f.guid, i)));
        self.instance_ids.extend(copy.nested_instances.iter().enumerate().filter(|(_, n)| !n.guid.is_nil()).map(|(i, n)| (n.guid, i)));
    }
}

/// The triangles `tris` (below a plan's cut) less what the wall `wall` (rings in plan) covers: each triangle's plan clipped
/// and lifted back onto it, so a sloping piece keeps its heights for what lies over what.
pub(crate) fn exposed_below(tris: Vec<[P3; 3]>, wall: &[Vec<P2>]) -> Vec<[P3; 3]> {
    let mut out = Vec::new();
    for tri in tris {
        let xy = tri.map(|p| P2(p[0], p[1]));
        if geom::area(&xy).abs() < 1e-9 {
            continue;
        }
        let remaining =
            wall.iter().fold(vec![xy.to_vec()], |pieces, r| pieces.into_iter().flat_map(|p| geom::subtract_convex(&p, r)).collect());
        let height = tri.map(|p| (p[0], p[1], p[2]));
        for r in remaining {
            for t in geom::triangulate(&r) {
                let [Some(a), Some(b), Some(c)] = t.map(|i| depth_at(height, (r[i].0, r[i].1)).map(|z| [r[i].0, r[i].1, z])) else {
                    continue;
                };
                out.push([a, b, c]);
            }
        }
    }
    out
}

/// Each of `family`'s forms and nested instances by GUID, its place in the family: where the family (as read from an older
/// file) has none, a GUID made for it now in the scratch model's copies, which its pieces then name.
pub(super) fn identities(model: &mut Model, family: &Family) -> (BTreeMap<Guid, usize>, BTreeMap<Guid, usize>) {
    let mut forms: BTreeMap<_, _> = family.forms.iter().enumerate().filter(|(_, f)| !f.guid.is_nil()).map(|(i, f)| (f.guid, i)).collect();
    let mut instances: BTreeMap<_, _> =
        family.nested_instances.iter().enumerate().filter(|(_, n)| !n.guid.is_nil()).map(|(i, n)| (n.guid, i)).collect();
    if forms.len() != family.forms.len() || instances.len() != family.nested_instances.len() {
        let names: std::collections::BTreeSet<_> = model
            .elements
            .values()
            .filter_map(|e| match e {
                Element::Component(c) => Some(c.family.clone()),
                Element::Door(o) | Element::Window(o) => o.family.clone(),
                _ => None,
            })
            .collect();
        for name in names {
            if let Some(f) = model.families.get_mut(&name) {
                for (i, form) in f.forms.iter_mut().enumerate().take(family.forms.len()) {
                    if family.forms[i].guid.is_nil() {
                        if form.guid.is_nil() {
                            form.guid = Guid::new();
                        }
                        forms.insert(form.guid, i);
                    }
                }
                for (i, instance) in f.nested_instances.iter_mut().enumerate().take(family.nested_instances.len()) {
                    if family.nested_instances[i].guid.is_nil() {
                        if instance.guid.is_nil() {
                            instance.guid = Guid::new();
                        }
                        instances.insert(instance.guid, i);
                    }
                }
            }
        }
    }
    (forms, instances)
}

/// The box round `tris` in view `which`: its millimetres, and depth, larger nearer the eye ([`projected`]).
fn mesh_bounds(tris: &[[P3; 3]], st: &Staged, which: Which) -> [P3; 2] {
    let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for p in tris.iter().flatten() {
        for (i, x) in projected(st, which, *p).into_iter().enumerate() {
            bounds[0][i] = bounds[0][i].min(x);
            bounds[1][i] = bounds[1][i].max(x);
        }
    }
    bounds
}

/// Point `p` in view `which`: its millimetres there, and how near the eye it is (in plan, how high).
fn projected(st: &Staged, which: Which, p: P3) -> P3 {
    let P2(x, y) = st.to_view(which, p);
    let depth = match which {
        Which::Plan => p[2],
        Which::Ceiling => -p[2],
        Which::Elevation(side) => -P2(p[0], p[1]).dot(side.looking()),
    };
    [x, y, depth]
}

/// How much nearer the eye a face must be than an edge to hide it.
const DEPTH_EPS: f64 = 1e-3;

/// The span (as fractions from `a` to `b`) of the segment from `a` to `b` (view millimetres and depth) that triangle `t` lies in
/// front of: the segment clipped to the triangle's three sides in the view and to the half space behind its face. Barycentric
/// coordinates and the depth between them both run straight along the segment. None where it hides none of it.
fn covered_span(a: P3, b: P3, t: [P3; 3]) -> Option<(f64, f64)> {
    let xy = |p: P3| P2(p[0], p[1]);
    let u = xy(t[1]) - xy(t[0]);
    let v = xy(t[2]) - xy(t[0]);
    let area = u.cross(v);
    if area.abs() < 1e-9 {
        return None;
    }
    let distances = |p: P3| {
        let q = xy(p) - xy(t[0]);
        let w1 = q.cross(v) / area;
        let w2 = u.cross(q) / area;
        let depth = t[0][2] + w1 * (t[1][2] - t[0][2]) + w2 * (t[2][2] - t[0][2]);
        [1.0 - w1 - w2, w1, w2, depth - p[2] - DEPTH_EPS]
    };
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for (x, y) in distances(a).into_iter().zip(distances(b)) {
        if x < 0.0 && y < 0.0 {
            return None;
        }
        if x < 0.0 {
            lo = lo.max(x / (x - y));
        } else if y < 0.0 {
            hi = hi.min(x / (x - y));
        }
    }
    (lo < hi).then_some((lo, hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An edge passing behind a sloping triangle and out in front of it again is hidden only where the triangle lies nearer.
    #[test]
    fn an_edge_crossing_the_depth_of_a_sloped_occluder_is_only_partly_hidden() {
        let triangle = [[0.0, 0.0, 0.0], [10.0, 0.0, 10.0], [0.0, 10.0, 0.0]];
        // At y = 2 the triangle runs from x 0 to 8, x deep there. The segment runs from x -2 to 10, 3 deep.
        let (a, b) = ([-2.0, 2.0, 3.0], [10.0, 2.0, 3.0]);
        let (lo, hi) = ((5.0 + DEPTH_EPS) / 12.0, 10.0 / 12.0);
        for (start, end, expected) in [(a, b, (lo, hi)), (b, a, (1.0 - hi, 1.0 - lo))] {
            let actual = covered_span(start, end, triangle).unwrap();
            assert!((actual.0 - expected.0).abs() < 1e-9 && (actual.1 - expected.1).abs() < 1e-9, "{actual:?}");
        }
        let coplanar = [[0.0, 0.0, 3.0], [10.0, 0.0, 3.0], [0.0, 10.0, 3.0]];
        assert_eq!(covered_span(a, b, coplanar), None);
    }
}
