//! FE-16c-1: a form's faces painted ([`super::Form::paint`]), each by the GEO-2b key its triangles carry ([`FormFace`]). A
//! build gives each piece the material each of its keys is painted ([`super::Piece::paint`]); its triangles are never split, so
//! a piece stays the closed solid sections and plans cut. Whether a form still has a face is read from the family alone, with
//! no build ([`Family::has_face`]), and paint naming one it no longer has is dropped ([`Family::drop_dead_paint`]): after every
//! command, by the engine, and when a file is read, where it is listed as left out.

use std::collections::{BTreeMap, BTreeSet};

use super::{Family, Form, FormFace, Kind, MAX_PAINT, Val};
use crate::ids::Guid;

/// The entries of the paint of family JSON `f`'s forms, and its nested families', that this version can't read (a face key or
/// a field it doesn't know) made a paint of no material, which [`Family::drop_paint`] then leaves out as the file is read: so
/// every entry left out is said once, by its place in the list as the file had it.
pub fn unread_paint(f: &mut serde_json::Value) {
    if let Some(serde_json::Value::Array(forms)) = f.get_mut("forms") {
        for form in forms.iter_mut() {
            let Some(serde_json::Value::Array(paint)) = form.get_mut("paint") else { continue };
            for p in paint.iter_mut().filter(|p| serde_json::from_value::<super::FormPaint>((*p).clone()).is_err()) {
                *p = serde_json::json!({"face": "all", "material": ""});
            }
        }
    }
    if let Some(serde_json::Value::Array(nested)) = f.get_mut("nested") {
        for inner in nested.iter_mut().filter_map(|n| n.get_mut("family")) {
            unread_paint(inner);
        }
    }
}

impl Family {
    /// What form `form` paints each of `keys`, its material parameters read in `values`, as a built piece names them; empty
    /// where it paints none of them.
    pub(crate) fn paint_of(&self, form: usize, keys: &[FormFace], values: &BTreeMap<String, Val>) -> Vec<String> {
        let Some(f) = self.forms.get(form).filter(|f| !f.paint.is_empty()) else { return Vec::new() };
        let named = |m: &str| match values.get(m) {
            Some(Val::Text(t)) => t.clone(),
            _ => m.to_string(),
        };
        let out: Vec<String> =
            keys.iter().map(|k| f.paint.iter().find(|p| p.face == *k).map(|p| named(&p.material)).unwrap_or_default()).collect();
        if out.iter().all(String::is_empty) { Vec::new() } else { out }
    }

    /// Whether `form`'s paint is sound: at most [`MAX_PAINT`] faces, each once, each a material (`material_ok` asks of one of
    /// the library), on a solid form.
    pub(crate) fn check_paint(&self, form: &Form, who: &str, material_ok: &dyn Fn(&str, &str) -> Result<(), String>) -> Result<(), String> {
        if form.paint.is_empty() {
            return Ok(());
        }
        if form.void {
            return Err(format!("{who}: a void is never seen, so it takes no paint; take its `paint` away"));
        }
        if form.paint.len() > MAX_PAINT {
            return Err(format!("{who}: it paints {} faces, more than {MAX_PAINT}", form.paint.len()));
        }
        let mut seen = BTreeSet::new();
        for p in &form.paint {
            let face = serde_json::to_string(&p.face).unwrap_or_default();
            if p.material.trim().is_empty() {
                return Err(format!("{who}: its paint on {face} needs a material"));
            }
            if !seen.insert(&p.face) {
                return Err(format!("{who}: face {face} is painted twice; keep one"));
            }
            if !self.parameters.iter().any(|q| q.name == p.material && q.kind == Kind::Material) {
                material_ok(&format!("{who}, its paint on {face}"), &p.material)?;
            }
        }
        Ok(())
    }

    /// Whether form `i` still has face `key`, read from the family alone, with no build: a side's line (and the path line it
    /// goes along) is still a line of the form; an edge's place is still in its lines; a cut's form still cuts it (a void or
    /// a solid that cuts or joins it) and still has the face it names; a cap or the whole form always. What a build makes
    /// hangs on a placed instance's values too, so a face that only some instances have keeps its paint: paint on one that
    /// none has paints nothing.
    pub fn has_face(&self, i: usize, key: &FormFace) -> bool {
        self.has_face_in(i, key, 0)
    }

    fn has_face_in(&self, i: usize, key: &FormFace, depth: usize) -> bool {
        let Some(form) = self.forms.get(i) else { return false };
        let line = |g: &Guid| !g.is_nil() && form.all_lines().any(|l| l.guid == *g);
        match key {
            FormFace::Start | FormFace::End | FormFace::All => true,
            FormFace::Side { line: l, along } => line(l) && along.as_ref().is_none_or(line),
            FormFace::TopSide(t) => line(t),
            FormFace::Edge(k) => *k < form.all_lines().count(),
            FormFace::Cut { by, face } => {
                // A key is as deep as the cuts that made it, never deeper than the family's forms.
                if depth > self.forms.len() || by.is_nil() {
                    return false;
                }
                let Some(m) = self.forms.iter().position(|f| f.guid == *by) else { return false };
                let cuts = self.cutters(i).contains(&m) || self.forms[m].joined_by.contains(&form.guid);
                cuts && self.has_face_in(m, face, depth + 1)
            }
        }
    }

    /// The lines of form `form` (by GUID) as an `{"edge": i}` counts them: each a line's GUID, or a line without one by
    /// where it lies. None where it hasn't the form.
    pub fn edge_print(&self, form: Guid) -> Option<Vec<String>> {
        let f = self.forms.iter().find(|f| f.guid == form && !form.is_nil())?;
        Some(
            f.all_lines()
                .map(|l| if l.guid.is_nil() { serde_json::to_string(l).unwrap_or_default() } else { l.guid.to_string() })
                .collect(),
        )
    }

    /// Its forms' paint, and its nested families', that names a GUID-less line by its place (`{"edge": i}`) in a form whose
    /// lines it has otherwise than `before` had them ([`crate::paint::edges_moved`]'s rule), taken away (ec's review of #91);
    /// what went, as JSON pointers from the family.
    pub fn drop_moved_edges(&mut self, before: &Family) -> Vec<String> {
        let moved: Vec<Vec<bool>> = self
            .forms
            .iter()
            .map(|f| {
                f.paint
                    .iter()
                    .map(|p| crate::paint::edge_owner(f.guid, &p.face).is_some_and(|g| self.edge_print(g) != before.edge_print(g)))
                    .collect()
            })
            .collect();
        let mut gone = Vec::new();
        for (i, (form, moved)) in self.forms.iter_mut().zip(moved).enumerate() {
            let mut k = 0;
            form.paint.retain(|_| {
                k += 1;
                let go = moved.get(k - 1).copied().unwrap_or(false);
                if go {
                    gone.push(format!("/forms/{i}/paint/{}", k - 1));
                }
                !go
            });
        }
        for (i, n) in self.nested.iter_mut().enumerate() {
            if let Some(was) = before.nested_family(&n.name).filter(|_| n.family.painted()) {
                let was = was.family.clone();
                gone.extend(n.family.drop_moved_edges(&was).into_iter().map(|p| format!("/nested/{i}/family{p}")));
            }
        }
        gone
    }

    /// Whether any of its forms, or of the families it nests, is painted.
    pub fn painted(&self) -> bool {
        self.forms.iter().any(|f| !f.paint.is_empty()) || self.nested.iter().any(|n| n.family.painted())
    }

    /// Its paint, and its nested families', that names a face its form no longer has ([`Family::has_face`]) taken away;
    /// what went, as JSON pointers from the family (`/forms/0/paint/1`, `/nested/0/family/forms/0/paint/0`).
    pub fn drop_dead_paint(&mut self) -> Vec<String> {
        self.drop_paint(&|_| true)
    }

    /// [`Family::drop_dead_paint`], and paint whose material is neither one `known` (the library's) nor a material parameter
    /// of its family taken away too, as a file is read: each said by its place in the list as the file had it.
    pub fn drop_paint(&mut self, known: &dyn Fn(&str) -> bool) -> Vec<String> {
        let params: BTreeSet<String> = self.parameters.iter().filter(|q| q.kind == Kind::Material).map(|q| q.name.clone()).collect();
        let keep: Vec<Vec<bool>> = (0..self.forms.len())
            .map(|i| {
                let ok = |m: &str| params.contains(m) || known(m);
                self.forms[i].paint.iter().map(|p| self.has_face(i, &p.face) && ok(&p.material)).collect()
            })
            .collect();
        let mut gone = Vec::new();
        for (i, (form, keep)) in self.forms.iter_mut().zip(keep).enumerate() {
            let mut k = 0;
            form.paint.retain(|_| {
                k += 1;
                let kept = keep.get(k - 1).copied().unwrap_or(true);
                if !kept {
                    gone.push(format!("/forms/{i}/paint/{}", k - 1));
                }
                kept
            });
        }
        for (i, n) in self.nested.iter_mut().enumerate() {
            if n.family.painted() {
                gone.extend(n.family.drop_paint(known).into_iter().map(|p| format!("/nested/{i}/family{p}")));
            }
        }
        gone
    }

    /// FE-16c-1's `paint` edit: form `form`'s `face` painted `material`, or its paint there taken off (`material` none), or all
    /// of its paint taken off (`face` none too). A face is refused unless the form has it ([`Family::has_face`]); a material is
    /// checked with the family.
    pub fn paint_face(&mut self, form: usize, face: Option<FormFace>, material: Option<String>) -> Result<(), String> {
        let known = face.as_ref().is_none_or(|k| self.has_face(form, k));
        let Some(f) = self.forms.get_mut(form) else { return Err(format!("the family has no form {}", form + 1)) };
        if f.void && material.is_some() {
            return Err(format!("form {} is a void, never seen: it takes no paint", form + 1));
        }
        match (face, material) {
            (None, None) => f.paint.clear(),
            (None, Some(_)) => {
                return Err("`face` is required: the face to paint, or null with `material`: null to take all its paint off".into());
            }
            (Some(face), None) => f.paint.retain(|p| p.face != face),
            (Some(face), Some(material)) => {
                if !known {
                    let said = serde_json::to_string(&face).unwrap_or_default();
                    return Err(format!(
                        "form {} has no face {said}: a side's line, or a cut's form, is no longer the form's (keys: `Form::paint`)",
                        form + 1
                    ));
                }
                match f.paint.iter_mut().find(|p| p.face == face) {
                    Some(p) => p.material = material,
                    None => f.paint.push(super::FormPaint { face, material }),
                }
            }
        }
        Ok(())
    }
}

impl crate::model::Model {
    /// The face of component `id`'s form a point `at` on its surface lies on, seen along `normal` (either way): the key of its
    /// nearest triangle that faces that way ([`FormFace`]), with its form's GUID and the nested instances down to it
    /// (FE-16c-2), what Paint clicks name. None where `id` is no component or no triangle of it is within a millimetre.
    pub fn component_face(&self, id: crate::model::Id, at: [f64; 3], normal: [f64; 3]) -> Option<crate::paint::FaceAt> {
        let Some(crate::model::Element::Component(c)) = self.elements.get(id) else { return None };
        face_among(at, normal, || self.component_pieces(c))
    }
}

/// [`crate::model::Model::component_face`] among the pieces `built` gives, a component's placed: the build it is looked for in,
/// built only once `at` and `normal` are found to be numbers.
pub(crate) fn face_among(
    at: [f64; 3],
    normal: [f64; 3],
    built: impl FnOnce() -> Vec<crate::family::Piece>,
) -> Option<crate::paint::FaceAt> {
    use crate::view3d::{cross, dot, sub};
    if !at.iter().chain(&normal).all(|v| v.is_finite()) {
        return None;
    }
    let mut best: Option<(f64, crate::paint::FaceAt)> = None;
    for piece in built() {
        for (t, k) in piece.tris.iter().zip(&piece.faces) {
            let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            let len = dot(n, n).sqrt();
            if len < 1e-12 {
                continue;
            }
            let n = n.map(|v| v / len);
            let way = dot(n, normal) / dot(normal, normal).sqrt().max(1e-12);
            if way.abs() < 0.9 {
                continue;
            }
            let off = dot(sub(at, t[0]), n).abs();
            // Inside the triangle, by the side of each edge, a millimetre's slack.
            let inside = (0..3).all(|e| {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                let edge = sub(b, a);
                let elen = dot(edge, edge).sqrt().max(1e-12);
                dot(cross(edge, sub(at, a)), n) / elen >= -1.0
            });
            if off > 1.0 || !inside {
                continue;
            }
            let Some(key) = piece.keys.get(*k as usize) else { continue };
            if best.as_ref().is_none_or(|(d, _)| off < *d) {
                best = Some((off, crate::paint::FaceAt { form: piece.form, nested: piece.path.clone(), face: key.clone() }));
            }
        }
    }
    best.map(|(_, k)| k)
}

impl crate::model::Model {
    /// The element whose faces a click names, for component `id`'s face reached through the nested instances `nested` (FE-16c-2):
    /// the deepest shared nested instance on the path ([`crate::nested_elements`]), an element of its own, with the rest of the
    /// path below it; else the component, with the whole path. None where `id` is no component.
    pub fn face_owner(&self, id: crate::model::Id, nested: &[Guid]) -> Option<(crate::model::Id, Vec<Guid>)> {
        let crate::model::Element::Component(c) = self.elements.get(id)? else { return None };
        let f = self.families.get(&c.family)?;
        let k = f.shared_steps(nested);
        if k == 0 {
            return Some((id, nested.to_vec()));
        }
        let host = self.elements.guid(id)?;
        let element = self.elements.id(crate::nested_elements::guid_of(host, &nested[..k]))?;
        Some((element, nested[k..].to_vec()))
    }
}
