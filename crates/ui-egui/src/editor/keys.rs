//! The family document's two-letter keys, as Revit's family editor has them (FE-13). A family tab has a table of its own,
//! [`FAMILY_SHORTCUTS`], and never reads the project's `SHORTCUTS` or the commands' aliases (the project's keys are blocked on a
//! family tab: `BimApp::keys`), so no project action runs from one. A key Keyboard Shortcuts made is tried first, when it names a
//! command the family has; then the table. Each key does what its ribbon button does, where the button is enabled, and nothing
//! where it is greyed or not on the ribbon at all (a sketch's tools outside a sketch, the dimensions in {3D}, Component with no
//! family loaded).
use super::temp_hide::Asked;
use super::*;
use crate::adaptive_view::{self, Draw};
use bimcraft_core::draw::Shape;
use bimcraft_core::family::DimKind;

/// Revit's default keys for the family editor's commands, each to the name of its action in `shortcuts::ACTIONS`. A key the
/// project has too means the same command here (`the_family_table_gives_no_project_key_another_meaning`); the others (DD, AN,
/// WP, WW, FA, AA, PL, FG, CG, ZX, FE-3's ZR, ZZ, ZO, ZV, ZP, ZC, FE-8's RE, and FE-20's FF, PR, ID, SV, BF) are the family's
/// alone. Trim/Extend Single and Multiple Elements have no key, as in Revit: Keyboard Shortcuts gives them one; nor has
/// Drag Elements on Selection ([`FAMILY_UNKEYED`]).
pub(crate) const FAMILY_SHORTCUTS: &[(&str, &str)] = &[
    ("MD", "Modify"),
    ("PP", "Properties"),
    ("VP", "Properties"),
    ("CM", "Component"),
    ("RP", "Reference Plane"),
    ("WP", "Work Plane"),
    ("WW", "Work Plane"),
    ("FA", "Load Family"),
    ("DI", "Aligned Dimension"),
    ("DD", "Aligned Dimension"),
    ("AN", "Angular Dimension"),
    ("TX", "Text"),
    ("AL", "Align"),
    ("AA", "Align"),
    ("MV", "Move"),
    ("CO", "Copy"),
    ("RO", "Rotate"),
    ("TR", "Trim/Extend to Corner"),
    ("MM", "Mirror Pick Axis"),
    ("DM", "Mirror Draw Axis"),
    ("OF", "Offset"),
    ("SL", "Split"),
    ("RE", "Scale"),
    ("DE", "Delete"),
    ("PL", "Pick Lines"),
    ("FG", "Finish Edit Mode"),
    ("CG", "Cancel Edit Mode"),
    ("KS", "Keyboard Shortcuts"),
    ("3D", "3D View"),
    ("ZE", "Zoom to Fit"),
    ("ZF", "Zoom to Fit"),
    ("ZX", "Zoom to Fit"),
    ("ZA", "Zoom to Fit"),
    ("ZR", "Zoom In Region"),
    ("ZZ", "Zoom In Region"),
    ("ZO", "Zoom Out (2x)"),
    ("ZV", "Zoom Out (2x)"),
    ("ZP", "Previous Pan/Zoom"),
    ("ZC", "Previous Pan/Zoom"),
    ("HH", "Temporary Hide Element"),
    ("HC", "Temporary Hide Category"),
    ("HI", "Temporary Isolate Element"),
    ("IC", "Temporary Isolate Category"),
    ("HR", "Reset Temporary Hide/Isolate"),
    // FE-20, the selection's commands (`selection.rs`): SA as the project's, Visible in View; the others the family's alone.
    ("FF", "Filter Selection"),
    ("PR", "Select Previous"),
    ("ID", "Select by ID"),
    ("SA", "Select All Instances"),
    ("SV", "Select All Instances: In Entire Family"),
    ("BF", "Select Elements by Face"),
];

/// The family document's commands Revit gives no key, which Keyboard Shortcuts can: the status bar's Drag Elements on
/// Selection.
pub(crate) const FAMILY_UNKEYED: &[&str] = &["Drag Elements on Selection"];

/// Whether `action` is a command the family document has: one its table gives a key, or one of [`FAMILY_UNKEYED`].
pub(crate) fn in_family(action: &str) -> bool {
    FAMILY_SHORTCUTS.iter().any(|(_, a)| *a == action) || FAMILY_UNKEYED.contains(&action)
}

impl BimApp {
    /// The family document's shortcuts (`family_keys` calls it unless a field, a dialog or a menu has the keys): Ctrl+1, the
    /// Properties palette as on a project's tab, and the two-letter keys: what Keyboard Shortcuts made a key, if the family has
    /// that command, else what [`FAMILY_SHORTCUTS`] gives it. A key that is neither stays in the buffer, as the project's does.
    /// Nothing while Keyboard Shortcuts is open over the document.
    pub(super) fn family_typed(&mut self, ctx: &Context) {
        if self.show_shortcuts {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num1)) {
            self.toggle_properties();
        }
        let typed: String = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Text(t) => Some(t.as_str()),
                    _ => None,
                })
                .collect()
        });
        for ch in typed.chars().filter(char::is_ascii_alphanumeric) {
            self.typed.push(ch.to_ascii_uppercase());
            if self.typed.len() > 2 {
                self.typed.remove(0);
            }
            if self.typed.len() < 2 {
                continue;
            }
            let custom = self.custom_shortcuts.get(&self.typed).map(String::as_str).filter(|a| in_family(a));
            let Some(action) = custom.or_else(|| FAMILY_SHORTCUTS.iter().find(|(k, _)| *k == self.typed).map(|(_, a)| *a)) else {
                continue;
            };
            let action = action.to_string();
            self.typed.clear();
            self.family_action(&action);
        }
    }

    /// `action` (a name of `shortcuts::ACTIONS`) in the active family document: what its ribbon button does there, where the
    /// button is enabled, else nothing. Whether the family has the command at all.
    pub(crate) fn family_action(&mut self, action: &str) -> bool {
        // What the window does for the document: its palettes, dialogs and the view's Temporary Hide/Isolate.
        let asked = match action {
            "Properties" => {
                self.toggle_properties();
                return true;
            }
            "Keyboard Shortcuts" => {
                self.show_shortcuts = true;
                return true;
            }
            // The status bar's toggle (FE-20), the window's own setting: where it changes how the view picks.
            "Select Elements by Face" => {
                if self.active_family().is_some_and(FamilyEditor::picks_by_face) {
                    self.select_by_face = !self.select_by_face;
                }
                return true;
            }
            // Its neighbour, Drag Elements on Selection: the same, and with no key of its own unless one is made for it.
            "Drag Elements on Selection" => {
                if self.active_family().is_some_and(FamilyEditor::picks_by_face) {
                    self.drag_on_selection = !self.drag_on_selection;
                }
                return true;
            }
            "Temporary Hide Element" => Some(Asked::Hide { category: false }),
            "Temporary Hide Category" => Some(Asked::Hide { category: true }),
            "Temporary Isolate Element" => Some(Asked::Isolate { category: false }),
            "Temporary Isolate Category" => Some(Asked::Isolate { category: true }),
            "Reset Temporary Hide/Isolate" => Some(Asked::Reset),
            _ => None,
        };
        if let Some(asked) = asked {
            self.family_temp(asked);
            return true;
        }
        let units = self.session.model.units;
        let Some(ed) = super::tabs::active_in(&mut self.families, self.sheet) else { return false };
        let sketching = ed.sketching();
        let ordinary = ed.adaptive.is_none();
        // What a command said: done (the status bar cleared, as Finish and Cancel clear it), or refused and why.
        let mut said: Option<Result<(), String>> = None;
        // FE-8: Modify's tools in a family that isn't adaptive, where their buttons are enabled; else why not, in the status bar.
        let modify = if ordinary { super::modify::Tool::of(action) } else { None };
        if let Some(tool) = modify {
            said = Some(ed.modify_tool_on(tool));
        }
        match action {
            _ if modify.is_some() => {}
            // While a form is sketched, the sketch's Modify: its Draw panel's shape put down, and Align.
            "Modify" => match ed.sketch.as_mut() {
                Some(s) => {
                    s.set_tool(None, false);
                    ed.views.aligning = None;
                    ed.views.dimensioning = None;
                }
                None => ed.modify_tool(),
            },
            // Create > Model's Component: enabled once a family is loaded into one that isn't adaptive.
            "Component" if !sketching && ordinary && !ed.nested.is_empty() => ed.component_tool(),
            "Reference Plane" if ed.editing_2d() => ed.reference_plane_tool(),
            // Work Plane's Set: the Work Plane dialog, for the sketch's plane while a form is sketched.
            "Work Plane" if ordinary => {
                let target = if sketching { super::work_plane::Target::Sketch } else { super::work_plane::Target::View };
                ed.open_work_plane_dialog(target);
            }
            "Load Family" if !sketching => ed.ask.load = true,
            // The tools check where they work: in a plan or an elevation; while sketching, Align, and the dimensions where the
            // view sees the sketch's plane flat (FE-12).
            "Aligned Dimension" => ed.dimension_tool(),
            "Angular Dimension" => ed.radial_tool(DimKind::Angular),
            "Align" => ed.align_tool(),
            // Annotate > Detail's Text, a label the family's plans draw.
            "Text" if !sketching && ordinary => ed.add_symbol(Part::Text),
            // Modify's Move, Copy and Rotate, enabled while something is selected in an adaptive family's view.
            "Move" | "Copy" if !ed.modeller.selection.is_empty() => {
                ed.modeller.tool = Draw::Move { from: None, copy: action == "Copy" };
            }
            "Rotate" => {
                let values = ed.values();
                let centre = ed.adaptive.as_ref().and_then(|d| adaptive_view::selection_centre(d, &ed.modeller, &values));
                if let Some(centre) = centre.filter(|_| !ed.modeller.selection.is_empty()) {
                    ed.modeller.tool = Draw::Rotate { centre, start: None };
                }
            }
            // Delete, as the Delete key: the lines picked while a form is sketched, else what is selected.
            "Delete" => {
                if let Some(s) = ed.sketch.as_mut() {
                    // The dimension selected while sketching, else the lines picked.
                    if let Some(g) = s.selected_dim {
                        ed.delete_dimension(g);
                    } else if !s.sketch.picked.is_empty() {
                        s.delete_picked();
                    }
                } else if ed.deletable() || !ed.modeller.selection.is_empty() {
                    said = ed.delete_selected().err().map(Err);
                }
            }
            // The sketch's own: Trim/Extend to Corner, the Draw gallery's Pick Lines, and Mode's Finish and Cancel.
            "Trim/Extend to Corner" => match ed.sketch.as_mut() {
                Some(s) => {
                    s.trim_tool();
                    ed.views.aligning = None;
                    ed.views.dimensioning = None;
                }
                None if ordinary => {
                    let why = t("Trim/Extend to Corner works on a sketch's lines: edit a form's sketch (double-click the form) first.");
                    said = Some(Err(why.to_string()));
                }
                None => {}
            },
            "Pick Lines" => {
                if let Some(s) = ed.sketch.as_mut() {
                    let axis = s.axis_tool;
                    s.set_tool(Some(Shape::PickLines), axis);
                }
            }
            "Finish Edit Mode" if sketching => said = Some(ed.finish_form_sketch(units)),
            "Cancel Edit Mode" if sketching => {
                ed.cancel_form_sketch();
                said = Some(Ok(()));
            }
            // The family's own {3D}, and the view control bar's zoom commands, in the view shown (`zoom.rs`).
            "3D View" => ed.views.show(None),
            "Zoom to Fit" => ed.zoom_to_fit(),
            "Zoom In Region" => ed.zoom_region_tool(),
            "Zoom Out (2x)" => ed.zoom_out(),
            "Previous Pan/Zoom" => ed.previous_zoom(),
            // The selection's commands (FE-20), each where it has something to work on: Filter with something selected,
            // Select Previous with a selection before this one, Select All Instances with a nested instance selected.
            "Filter Selection" => ed.open_filter(),
            "Select Previous" => ed.select_previous(),
            "Select by ID" => ed.open_select_by_id(),
            "Select All Instances" => ed.select_instances(true),
            "Select All Instances: In Entire Family" => ed.select_instances(false),
            _ => return in_family(action),
        }
        match said {
            Some(Ok(())) => self.status = Status::default(),
            Some(Err(text)) => self.status = Status { text, error: true },
            None => {}
        }
        true
    }
}
