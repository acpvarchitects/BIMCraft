//! FE-16c-1: Paint (PT) and Remove Paint in a family document that isn't adaptive, as the project has them: a click on a form's
//! face in {3D} paints that face (by its GEO-2b key, `FormFace`) the material in the Options Bar, or takes its paint off. The
//! core does the work (`Family::paint_face`); the face clicked is found on the staged part (`Model::component_face`), whose
//! GUIDs `FamilyScene::guids` gives back as the family's. Each face painted is one step of the document's history.
use super::*;
use bimcraft_core::family::FormFace;
use bimcraft_core::ids::Guid;

/// `key` as family `back` names it: each GUID of a staged part given back as the family's ([`super::lit3d::FamilyScene`]).
pub(crate) fn key_back(key: FormFace, back: &BTreeMap<Guid, Guid>) -> FormFace {
    let g = |x: Guid| back.get(&x).copied().unwrap_or(x);
    match key {
        FormFace::Side { line, along } => FormFace::Side { line: g(line), along: along.map(g) },
        FormFace::TopSide(t) => FormFace::TopSide(g(t)),
        FormFace::Cut { by, face } => FormFace::Cut { by: g(by), face: Box::new(key_back(*face, back)) },
        k => k,
    }
}

impl FamilyEditor {
    /// Whether Paint can be used now: on the forms of a family that isn't adaptive, outside a sketch.
    pub(crate) fn paint_ready(&self) -> bool {
        self.adaptive.is_none() && self.sketch.is_none()
    }

    /// Paint, or Remove Paint (`remove`), in hand: the other tools put down, the selection let go, and {3D} shown, where it
    /// works. Nothing where it can't be used.
    pub(crate) fn paint_tool(&mut self, remove: bool) {
        if !self.paint_ready() {
            return;
        }
        self.drop_tools();
        self.views.painting = Some(remove);
        self.views.show(None);
        self.last_tool = Some(if remove { "Remove Paint" } else { "Paint" });
    }

    /// The material Paint lays: the one picked while `materials` has it, else their first.
    pub(crate) fn paint_material(&self, materials: &[String]) -> Option<String> {
        Some(self.views.paint_material.clone()).filter(|m| materials.contains(m)).or_else(|| materials.first().cloned())
    }

    /// A click with Paint or Remove Paint in hand on `face` of form `form` (none: on no form's face): the face painted
    /// `material`, or its paint taken off. What went wrong, if anything.
    pub(crate) fn paint_click(&mut self, face: Option<(usize, FormFace)>, material: Option<String>) -> Result<(), String> {
        let Some(remove) = self.views.painting else { return Ok(()) };
        let Some((form, key)) = face.filter(|(i, _)| *i < self.forms.len()) else {
            return Err(t("there's no face to paint there").to_string());
        };
        let material = if remove { None } else { Some(material.ok_or_else(|| t("there's no face to paint there").to_string())?) };
        let mut family = self.family()?;
        family.paint_face(form, Some(key), material)?;
        let made = family.forms.get(form).map(|f| f.paint.clone()).unwrap_or_default();
        self.forms[form].original.paint = made;
        Ok(())
    }
}
