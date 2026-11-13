//! FE-17: Pin and Unpin in a family document that isn't adaptive, as Revit's family editor has them (PN, UP; the Modify
//! panel's Pin and Unpin). What is selected is pinned where it is, or unpinned, in one step of the document's history; a datum
//! is pinned always. A part pinned keeps its place: a drag and the Modify tools refuse to move, turn, mirror or scale it,
//! Align to move it, Delete (and Cut) to take it away, each saying it is pinned; a copy of it (Ctrl+drag, Copy, Paste) is made
//! unpinned. Select Pinned Elements (PE, the status bar's toggle, a setting the window keeps) off, a click and a box pass over
//! what is pinned, as Revit's do. What a part's pin is, and what refuses it, is the core's (`family::select::pinned`,
//! `Stays::Pinned`).
use super::select_box::Picked;
use super::*;
use bimcraft_core::family::select;

/// What a refusal says of `what`, pinned.
pub(super) fn pinned_said(what: &str) -> String {
    tf("{} is pinned: unpin it (UP) to change where it is.", &[what])
}

impl FamilyEditor {
    /// Whether `p` is pinned in the draft: a datum always.
    pub(crate) fn is_pinned(&self, p: Picked) -> bool {
        match p {
            Picked::Form(i) => self.forms.get(i).is_some_and(|f| f.original.pinned),
            Picked::Plane(g) => self.planes.iter().any(|x| x.guid == g && (x.pinned || x.datum())),
            Picked::Line(g) => self.lines.iter().any(|x| x.guid == g && x.pinned),
            Picked::ModelLine(g) => self.model_lines.iter().any(|x| x.guid == g && x.pinned),
            Picked::Dimension(g) => self.dimensions.iter().any(|x| x.guid == g && x.pinned),
            Picked::Symbol(i) => self.symbols.get(i).is_some_and(|x| x.pinned),
            Picked::Connector(i) => self.connectors.get(i).is_some_and(|x| x.pinned()),
            Picked::Nested(i) => self.nested_instances.get(i).is_some_and(|x| x.pinned),
        }
    }

    /// The first of `items` pinned, named as a refusal says it; none where none is. A datum, pinned always, has refusals of
    /// its own ("is a datum"), and Delete leaves it where it is: it isn't one.
    pub(super) fn pinned_refusal(&self, items: &[Picked]) -> Option<String> {
        let p = items.iter().copied().find(|p| self.is_pinned(*p) && !self.is_datum(*p))?;
        let named = self.family().map(|f| select::said(&f, p)).unwrap_or_else(|_| t("an element").to_string());
        Some(pinned_said(&named))
    }

    /// Whether Pin (`on`) or Unpin has something selected to change: something not pinned, or pinned and not a datum.
    pub(crate) fn can_pin(&self, on: bool) -> bool {
        self.adaptive.is_none()
            && self.sketch.is_none()
            && self.selection.items().iter().any(|p| self.holds(*p) && self.is_pinned(*p) != on && !self.is_datum(*p))
    }

    fn is_datum(&self, p: Picked) -> bool {
        matches!(p, Picked::Plane(g) if self.planes.iter().any(|x| x.guid == g && x.datum()))
    }

    /// Pin (PN) or Unpin (UP, `on` false): what is selected pinned where it is, or not, a datum left pinned; one step of the
    /// history where anything changed, the selection kept. Nothing while a form is sketched or in an adaptive family.
    pub(crate) fn pin_selected(&mut self, on: bool) {
        if !self.can_pin(on) {
            return;
        }
        for p in self.selection.items().to_vec() {
            match p {
                Picked::Form(i) => {
                    if let Some(f) = self.forms.get_mut(i) {
                        f.original.pinned = on;
                    }
                }
                Picked::Plane(g) => {
                    if let Some(x) = self.planes.iter_mut().find(|x| x.guid == g && !x.datum()) {
                        x.pinned = on;
                    }
                }
                Picked::Line(g) => {
                    if let Some(x) = self.lines.iter_mut().find(|x| x.guid == g) {
                        x.pinned = on;
                    }
                }
                Picked::ModelLine(g) => {
                    if let Some(x) = self.model_lines.iter_mut().find(|x| x.guid == g) {
                        x.pinned = on;
                    }
                }
                Picked::Dimension(g) => {
                    if let Some(x) = self.dimensions.iter_mut().find(|x| x.guid == g) {
                        x.pinned = on;
                    }
                }
                Picked::Symbol(i) => {
                    if let Some(x) = self.symbols.get_mut(i) {
                        x.pinned = on;
                    }
                }
                Picked::Connector(i) => {
                    if let Some(x) = self.connectors.get_mut(i) {
                        x.set_pinned(on);
                    }
                }
                Picked::Nested(i) => {
                    if let Some(x) = self.nested_instances.get_mut(i) {
                        x.pinned = on;
                    }
                }
            }
        }
    }
}
