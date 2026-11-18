//! FE-16c-1: a form's faces painted ([`super::Form::paint`]), each by the GEO-2b key its triangles carry ([`FormFace`]). A
//! build gives each piece the material each of its keys is painted ([`super::Piece::paint`]); its triangles are never split, so
//! a piece stays the closed solid sections and plans cut. Which faces a form has is worked out from what it builds
//! ([`Family::faces_built`]), and paint naming one it no longer has is dropped ([`Family::drop_dead_paint`]): after every
//! command, by the engine, and when a file is read, where it is listed as left out.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::{Family, Form, FormFace, Kind, MAX_PAINT, Val, csg};

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
                material_ok(who, &p.material)?;
            }
        }
        Ok(())
    }

    /// The faces each form builds, by its place in the list: the keys of its piece under any of the family's types, every
    /// form shown (a form its `visible` hides under every type keeps its faces). None where no type builds, when nothing can
    /// be said of them.
    pub fn faces_built(&self) -> Option<Vec<BTreeSet<FormFace>>> {
        let mut shown = self.clone();
        shown.forms.iter_mut().for_each(|f| f.visible = None);
        // What a form builds never hangs on what the family nests.
        shown.nested_instances.clear();
        let mut out = vec![BTreeSet::new(); self.forms.len()];
        let mut any = false;
        for t in shown.types.keys() {
            let Ok(values) = shown.values(t, &BTreeMap::new()) else { continue };
            let Ok(flexed) = shown.flexed(&values) else { continue };
            let Ok(built) = flexed.build_of(&values, &mut HashMap::new(), &mut csg::Meter::default()) else { continue };
            any = true;
            for (i, p) in built {
                if let Some(keys) = out.get_mut(i) {
                    keys.extend(p.keys);
                }
            }
        }
        any.then_some(out)
    }

    /// Whether any of its forms, or of the families it nests, is painted.
    pub fn painted(&self) -> bool {
        self.forms.iter().any(|f| !f.paint.is_empty()) || self.nested.iter().any(|n| n.family.painted())
    }

    /// Its paint, and its nested families', that names a face its form no longer builds ([`Family::faces_built`]) taken away;
    /// what went, as JSON pointers from the family (`/forms/0/paint/1`, `/nested/0/family/forms/0/paint/0`). Nothing goes where
    /// no type builds.
    pub fn drop_dead_paint(&mut self) -> Vec<String> {
        let mut gone = Vec::new();
        if self.forms.iter().any(|f| !f.paint.is_empty())
            && let Some(built) = self.faces_built()
        {
            for (i, (form, faces)) in self.forms.iter_mut().zip(&built).enumerate() {
                let mut k = 0;
                form.paint.retain(|p| {
                    k += 1;
                    let alive = faces.contains(&p.face);
                    if !alive {
                        gone.push(format!("/forms/{i}/paint/{}", k - 1));
                    }
                    alive
                });
            }
        }
        for (i, n) in self.nested.iter_mut().enumerate() {
            if n.family.painted() {
                gone.extend(n.family.drop_dead_paint().into_iter().map(|p| format!("/nested/{i}/family{p}")));
            }
        }
        gone
    }

    /// FE-16c-1's `paint` edit: form `form`'s `face` painted `material`, or its paint there taken off (`material` none), or all
    /// of its paint taken off (`face` none too). A face is refused unless the form builds it; a material is checked with the
    /// family.
    pub fn paint_face(&mut self, form: usize, face: Option<FormFace>, material: Option<String>) -> Result<(), String> {
        let built = match &face {
            Some(_) if material.is_some() => Some(self.faces_built()),
            _ => None,
        };
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
                let faces = built.flatten().ok_or("the family builds under none of its types, so its faces aren't known")?;
                let has = faces.get(form).cloned().unwrap_or_default();
                if !has.contains(&face) {
                    let mut named: Vec<String> = has.iter().take(12).map(|k| serde_json::to_string(k).unwrap_or_default()).collect();
                    if has.len() > 12 {
                        named.push("…".into());
                    }
                    let said = serde_json::to_string(&face).unwrap_or_default();
                    return Err(format!("form {} has no face {said}; its faces are {}", form + 1, named.join(", ")));
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
    /// nearest triangle that faces that way ([`FormFace`]), what Paint clicks name. None where `id` is no component or no
    /// triangle of it is within a millimetre.
    pub fn component_face(&self, id: crate::model::Id, at: [f64; 3], normal: [f64; 3]) -> Option<FormFace> {
        use crate::view3d::{cross, dot, sub};
        let Some(crate::model::Element::Component(c)) = self.elements.get(id) else { return None };
        if !at.iter().chain(&normal).all(|v| v.is_finite()) {
            return None;
        }
        let mut best: Option<(f64, FormFace)> = None;
        for piece in self.component_pieces(c) {
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
                    best = Some((off, key.clone()));
                }
            }
        }
        best.map(|(_, k)| k)
    }
}
