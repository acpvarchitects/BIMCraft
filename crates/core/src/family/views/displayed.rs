//! GEO-2c-2e: what a family's plan or elevation drew, kept with its staged model so a click, an edge click and a box take what
//! the view shows: each piece of a form or a nested instance as the drawing built it (refined for the zoom, GEO-2c, or its
//! default build where that is too fine to cut), with, in plan, the strokes it was drawn with. The scratch model may hold a
//! second, halftone placement of what the type hides: its pieces name the same forms. (Ported from the upstream series'
//! picker, ec's 7f768c09e and e23c403b0.)

use super::*;

/// The pieces a view drew, each as it drew it.
#[derive(Default)]
pub(crate) struct DisplayedForms {
    pieces: Vec<DisplayedPiece>,
}

struct DisplayedPiece {
    /// The GUID of the form it is of (in the family whose placement drew it) and the nested instances it is part of.
    form: Guid,
    path: Vec<Guid>,
    /// Its triangles, placed: in plan, what the plan sees of it (below a hosted opening's cut, less what the wall covers,
    /// with the cut's caps); in an elevation, what the picture was drawn from.
    tris: Vec<[P3; 3]>,
    /// In plan, the strokes it was drawn with, in the view's millimetres; an elevation finds a piece's edges in its own frame.
    edges: Option<Vec<(P2, P2)>>,
}

impl DisplayedForms {
    /// A piece an elevation drew. A nested instance is selected whole, so its pieces are kept as one (as far as
    /// [`MAX_OCCLUDERS`] lets them hide each other's edges): its nearer forms hide its farther ones' rims, as in the picture.
    pub(crate) fn push_mesh(&mut self, form: Guid, path: Vec<Guid>, tris: Vec<[P3; 3]>) {
        if let Some(instance) = path.first()
            && let Some(owner) = self.pieces.iter_mut().rev().find(|p| p.path.first() == Some(instance))
            && owner.tris.len() + tris.len() <= MAX_OCCLUDERS
        {
            owner.tris.extend(tris);
            return;
        }
        self.pieces.push(DisplayedPiece { form, path, tris, edges: None });
    }

    /// A piece a plan drew, with the strokes it drew it with.
    pub(crate) fn push_plan(&mut self, piece: crate::family::Piece, edges: Vec<(P2, P2)>) {
        self.pieces.push(DisplayedPiece { form: piece.form, path: piece.path, tris: piece.tris, edges: Some(edges) });
    }

    /// What of the family the piece on top at `at` is, among those `keep` keeps: in plan the highest, in an elevation the
    /// nearest the eye.
    pub(super) fn face(&self, st: &Staged, which: Which, at: P2, keep: impl Fn(Part) -> bool) -> Option<Part> {
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

    /// What of the family `keep` keeps has an edge the view drew within `reach` of `at`: the one whose edge is nearest. In an
    /// elevation only where no other piece lies nearer the eye there.
    pub(super) fn edge(&self, st: &Staged, which: Which, at: P2, reach: f64, keep: impl Fn(Part) -> bool) -> Option<Part> {
        self.pieces
            .iter()
            .filter_map(|piece| {
                let part = piece.part(st).filter(|part| keep(*part))?;
                let distance = match &piece.edges {
                    Some(edges) => {
                        edges.iter().map(|(a, b)| geom::dist_to_segment(at, *a, *b)).filter(|d| *d <= reach).min_by(f64::total_cmp)
                    }
                    None => edge_near(&piece.tris, st, which, at, reach, |q| {
                        if self.pieces.len() == 1 {
                            return true;
                        }
                        let depth = piece.depth(st, which, q).unwrap_or(f64::NEG_INFINITY);
                        !self.pieces.iter().any(|other| {
                            !std::ptr::eq(other, piece)
                                && other.part(st).is_some()
                                && other.depth(st, which, q).is_some_and(|near| near > depth + 1e-3)
                        })
                    }),
                }?;
                Some((part, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(part, _)| part)
    }

    /// The edges `part` was drawn with: in plan its strokes, in an elevation its pieces' seen edges.
    pub(super) fn outline(&self, st: &Staged, which: Which, part: Part) -> Vec<(P2, P2)> {
        self.pieces
            .iter()
            .filter(|p| p.part(st) == Some(part))
            .flat_map(|p| p.edges.clone().unwrap_or_else(|| seen_edges(&p.tris, st, which)))
            .collect()
    }

    /// Whether `boxed` takes `part` by what the view drew of it, as [`face_in_box`] says.
    pub(super) fn in_box(&self, st: &Staged, which: Which, part: Part, boxed: &geom::SelectBox) -> bool {
        let mut tris = self.pieces.iter().filter(|p| p.part(st) == Some(part)).flat_map(|p| &p.tris).peekable();
        if tris.peek().is_none() {
            return false;
        }
        if boxed.crossing {
            tris.any(|t| boxed.touches(&t.map(|p| st.to_view(which, p))))
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

    /// How near the eye it is at `at` (in plan, how high), where it covers it.
    fn depth(&self, st: &Staged, which: Which, at: P2) -> Option<f64> {
        self.tris
            .iter()
            .filter_map(|tri| {
                let projected = tri.map(|p| {
                    let P2(x, y) = st.to_view(which, p);
                    let z = match which {
                        Which::Plan => p[2],
                        Which::Ceiling => -p[2],
                        Which::Elevation(side) => -P2(p[0], p[1]).dot(side.looking()),
                    };
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
