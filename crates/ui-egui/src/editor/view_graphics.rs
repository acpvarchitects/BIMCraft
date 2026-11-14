//! FE-18: view graphics in the family document, as Revit's family editor has them. Each of its views ({3D}, the two plans and
//! the four elevations) keeps its own Visibility/Graphics (VV, VG: the project's dialog, its rows the categories a family's
//! views draw), visual style (WF, HL, SD) and Graphic Display Options (GD), {3D}'s alone as in the project, and View Range (VR),
//! a plan's alone; Thin Lines (TL) is the window's, as Revit's is the application's. Each view's are laid over the scratch
//! project it is drawn of (`Views::staged`, `staged_3d`), so the project's drawing does what they say.
//!
//! What they set is the view's for the session, as Preview Visibility and Temporary Hide/Isolate are: no part of the draft, so
//! neither saved, nor a step of its history, nor an unsaved change; it goes when the family closes.
use super::tabs::DocId;
use super::*;
use bimcraft_core::family::views::Which;
use bimcraft_core::graphics::{Graphics, VisualStyle};
use bimcraft_core::model::ViewRange;
use egui::{Button, Grid, TextEdit};

/// What the family document's views keep of their graphics for the session.
#[derive(Default)]
pub(crate) struct Kept {
    /// Each view's Visibility/Graphics and visual style, by view ({3D}'s under none); none set, the project's defaults.
    graphics: BTreeMap<Option<Which>, Graphics>,
    /// Each plan's View Range; none set, a project plan's default.
    ranges: BTreeMap<Which, ViewRange>,
    /// {3D}'s Graphic Display Options.
    pub(crate) display: bimcraft_core::view3d::DisplayOptions,
    /// The View Range dialog, open on a plan: its four fields as typed, and why OK was refused.
    range_dialog: Option<RangeDialog>,
}

/// The View Range dialog, open.
struct RangeDialog {
    which: Which,
    fields: [String; 4],
    error: Option<String>,
}

/// The family document's view the Visibility/Graphics dialog is open on, and the categories it lists.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FamilyView {
    pub(crate) doc: DocId,
    pub(crate) which: Option<Which>,
    /// The Model Categories tab's rows, then the Annotation Categories tab's.
    pub(crate) model: Vec<&'static str>,
    pub(crate) annotation: Vec<&'static str>,
}

impl Kept {
    /// View `which`'s Visibility/Graphics and visual style.
    pub(crate) fn graphics_of(&self, which: Option<Which>) -> Graphics {
        self.graphics.get(&which).cloned().unwrap_or_default()
    }

    /// Plan `which`'s View Range.
    pub(crate) fn range_of(&self, which: Which) -> ViewRange {
        self.ranges.get(&which).copied().unwrap_or_default()
    }

    /// The heights plan `which` sees once its View Range is set, lowest and highest, mm over Ref. Level; none until then, every
    /// form whatever its height (`Staged::seen`).
    pub(crate) fn seen(&self, which: Which) -> Option<(f64, f64)> {
        let r = self.ranges.get(&which)?;
        Some((r.bottom.min(r.depth), r.top.unwrap_or(f64::INFINITY)))
    }

    /// View `which`'s graphics set to `g`.
    pub(crate) fn set_graphics(&mut self, which: Option<Which>, g: Graphics) {
        if g == Graphics::default() {
            self.graphics.remove(&which);
        } else {
            self.graphics.insert(which, g);
        }
    }

    /// Whether a dialog of the views' is open, which has the keys.
    pub(crate) fn dialog_open(&self) -> bool {
        self.range_dialog.is_some()
    }

    /// What each view keeps that changes how it is drawn, for the caches' keys.
    pub(crate) fn key(&self, which: Option<Which>) -> String {
        let range = which.and_then(|w| self.ranges.get(&w));
        format!("{}{:?}{:?}", serde_json::to_string(&self.graphics_of(which)).unwrap_or_default(), range, self.display)
    }
}

/// The view a family document's view is drawn of has `own` laid over it: its categories' and elements' overrides and its
/// visual style, over what the document itself sets there (Temporary Hide's wall, the ghost's halftone), which win.
pub(crate) fn lay_over(graphics: &mut Graphics, own: &Graphics) {
    for (k, o) in &own.categories {
        graphics.categories.entry(k.clone()).or_insert_with(|| o.clone());
    }
    for (g, o) in &own.elements {
        graphics.elements.entry(*g).or_insert_with(|| o.clone());
    }
    graphics.style = own.style;
}

impl FamilyEditor {
    /// The categories the family's views draw, as Visibility/Graphics lists them: its own category (a door's or a window's as
    /// the project's Doors or Windows) and, for one that stands in a wall, Walls; and the levels its elevations draw Ref. Level
    /// as.
    pub(crate) fn view_categories(&self) -> (Vec<&'static str>, Vec<&'static str>) {
        let own = match self.category {
            Category::Door => "door",
            Category::Window => "window",
            c => bimcraft_core::graphics::component_category(c).unwrap_or(bimcraft_core::graphics::COMPONENT),
        };
        let mut model = vec![own];
        if self.category.hosted() {
            model.push("wall");
        }
        (model, vec!["level"])
    }
}

impl BimApp {
    /// Visibility/Graphics (VV, VG) for the family document's view shown.
    pub(crate) fn family_vg(&mut self) {
        let (Some(doc), Some(ed)) = (self.active_doc(), self.active_family()) else { return };
        self.vg_dialog = Some(vg_for(doc, ed));
    }

    /// The Visibility/Graphics dialog's OK or Apply on a family document's view: `g` its graphics now.
    pub(crate) fn family_vg_done(&mut self, view: &FamilyView, g: &Graphics) {
        if let Some(ed) = self.family_of_mut(view.doc) {
            let mut own = ed.views.kept.graphics_of(view.which);
            own.categories = g.categories.clone();
            own.elements = g.elements.clone();
            ed.views.kept.set_graphics(view.which, own);
        }
    }

    /// Wireframe, Hidden Line or Shaded (WF, HL, SD) for {3D}; in a plan or an elevation, says they are {3D}'s.
    pub(crate) fn family_style(&mut self, style: VisualStyle) {
        let Some(ed) = self.active_family_mut() else { return };
        if ed.views.shown.is_some() {
            self.status = Status { text: t("visual styles are for {3D}: open it from the Project Browser").into(), error: true };
            return;
        }
        let mut g = ed.views.kept.graphics_of(None);
        g.style = style;
        ed.views.kept.set_graphics(None, g);
    }

    /// Graphic Display Options (GD) for {3D}; in a plan or an elevation, says they are {3D}'s.
    pub(crate) fn family_display(&mut self) {
        let Some(ed) = self.active_family() else { return };
        if ed.views.shown.is_some() {
            let text = t("graphic display options are for {3D}: open it from the Project Browser");
            self.status = Status { text: text.into(), error: true };
            return;
        }
        let options = ed.views.kept.display;
        let Some(doc) = self.active_doc() else { return };
        self.display_dialog = Some(crate::display::DisplayDialog::for_family(options, doc));
    }

    /// The Graphic Display Options dialog's OK or Apply on a family document's {3D}.
    pub(crate) fn family_display_done(&mut self, doc: DocId, options: bimcraft_core::view3d::DisplayOptions) {
        if let Some(ed) = self.family_of_mut(doc) {
            ed.views.kept.display = options;
        }
    }

    /// View Range (VR) for the floor plan; in another view, says it is the floor plan's. A ceiling plan looks up from its own
    /// cut, as a project's does, and keeps none.
    pub(crate) fn family_view_range(&mut self) {
        let units = self.session.model.units;
        let Some(ed) = self.active_family_mut() else { return };
        let Some(which) = ed.views.shown.filter(|w| *w == Which::Plan) else {
            self.status = Status { text: t("a view range is the floor plan's: open Floor Plans > Ref. Level").into(), error: true };
            return;
        };
        let r = ed.views.kept.range_of(which);
        let fields =
            [r.top.map(|v| units.length(v)).unwrap_or_default(), units.length(r.cut), units.length(r.bottom), units.length(r.depth)];
        ed.views.kept.range_dialog = Some(RangeDialog { which, fields, error: None });
    }

    /// The family document's View Range dialog, while it is open: the plan's range typed in the project's units; OK sets it.
    pub(crate) fn family_range_window(&mut self, ctx: &Context) {
        let units = self.session.model.units;
        let Some(ed) = self.active_family_mut() else { return };
        let Some(mut d) = ed.views.kept.range_dialog.take() else { return };
        let (mut ok, mut cancel) = (false, false);
        let r = crate::popup::Modal::new(UiId::new("family-view-range"), t("View Range")).width(340.0).show(ctx, |ui| {
            crate::popup::body(ui, |ui| {
                Grid::new("family-view-range-fields").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                    for (i, (label, hint)) in
                        [(t("Top"), t("Unlimited")), (t("Cut Plane"), ""), (t("Bottom"), ""), (t("View Depth"), "")].into_iter().enumerate()
                    {
                        let l = ui.label(label);
                        ui.add_sized([160.0, ui.spacing().interact_size.y], TextEdit::singleline(&mut d.fields[i]).hint_text(hint))
                            .labelled_by(l.id);
                        ui.end_row();
                    }
                });
                ui.label(RichText::new(t("Offsets from Ref. Level.")).color(pal().muted));
                if let Some(e) = &d.error {
                    ui.label(RichText::new(e).color(pal().error));
                }
            });
            crate::popup::footer(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let size = vec2(76.0, 0.0);
                    cancel = ui.add(Button::new(t("Cancel")).min_size(size)).clicked();
                    ok = ui.add(Button::new(t("OK")).min_size(size)).clicked();
                });
            });
        });
        let mut keep = !(ok || cancel || r.should_close());
        if ok {
            let parse = |s: &str| units.parse(s);
            let top = if d.fields[0].trim().is_empty() { Some(None) } else { parse(&d.fields[0]).map(Some) };
            match (top, parse(&d.fields[1]), parse(&d.fields[2]), parse(&d.fields[3])) {
                (Some(top), Some(cut), Some(bottom), Some(depth)) => {
                    // As the project checks a plan's: within reach, and top ≥ cut plane ≥ bottom ≥ view depth.
                    let range = ViewRange { top, cut, bottom, depth };
                    match bimcraft_core::model::range_ok(t("Ref. Level"), &range) {
                        // OK on what the dialog showed of a plan that had none sets none: it still sees every form.
                        Ok(()) if range == ViewRange::default() && !ed.views.kept.ranges.contains_key(&d.which) => {}
                        Ok(()) => {
                            ed.views.kept.ranges.insert(d.which, range);
                        }
                        Err(e) => {
                            d.error = Some(e);
                            keep = true;
                        }
                    }
                }
                _ => {
                    d.error = Some(t("Each is a length, and Top may be left empty for the level above.").to_string());
                    keep = true;
                }
            }
        }
        if keep {
            ed.views.kept.range_dialog = Some(d);
        }
    }
}

/// Visibility/Graphics on the view document `doc` (`ed`) shows.
pub(crate) fn vg_for(doc: DocId, ed: &FamilyEditor) -> crate::graphics::VgDialog {
    let which = ed.views.shown;
    let (model, annotation) = ed.view_categories();
    let title = format!("{} - {}", ed.name.trim(), view_name(which));
    let graphics = ed.views.kept.graphics_of(which);
    crate::graphics::VgDialog::for_family(FamilyView { doc, which, model, annotation }, title, graphics)
}

/// View `which`'s name, as the Project Browser and its tab name it.
fn view_name(which: Option<Which>) -> String {
    match which {
        None => "{3D}".into(),
        Some(Which::Plan) => format!("{} {}", t("Floor Plan:"), t("Ref. Level")),
        Some(Which::Ceiling) => format!("{} {}", t("Ceiling Plan:"), t("Ref. Level")),
        Some(Which::Elevation(s)) => format!("{} {}", t("Elevation:"), t(s.name())),
    }
}
