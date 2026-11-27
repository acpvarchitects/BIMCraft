//! Divide Parts and Edit Division (HP-7b), as Revit's: the parts selected are divided by lines sketched on a face of them in
//! the 3D view. Divide Parts asks for the face first, a click on it (`Scene::face_at`, the part's or, where the view shows
//! the original, its host's); the view then turns square to it and the sketch begins there (`sketch3d.rs`, on the face's
//! own axes), drawn with the Draw panel's shapes. Edit Division takes up a division's sketch on the face it was drawn on.
//! Finish chains the lines into cuts (`bimcraft_core::parts::drawn`) with the Options Bar's Gap and Depth, and runs
//! `divide_parts` on each part, one undo step that also shows parts in the 3D view; the pieces are then selected.

use bimcraft_i18n::msg;

use bimcraft_core::curves::Curve;
use bimcraft_core::divide::{Cut, Frame, Recipe};
use bimcraft_core::model::{Element, Id, Model};
use bimcraft_core::parts::drawn;
use bimcraft_core::sketch::SketchLine;
use bimcraft_core::view3d::{Camera, Face};
use egui::Ui;
use serde_json::{Value, json};

use crate::lang::t;
use crate::sketch::Of;
use crate::{BimApp, Sheet, Status, Tool};

/// A division under way: the parts it divides, the face it is sketched on once picked, the Options Bar's Gap and Depth as
/// typed, and the selection to go back to when it is cancelled.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Dividing {
    pub parts: Vec<Id>,
    pub face: Option<Frame>,
    pub gap: String,
    pub depth: String,
    pub was: Vec<Id>,
    /// Edit Division of a division the part has, not a new one: what the undo step is called.
    pub edit: bool,
}

const PICK_FACE: &str = msg!("Click the face of the selected parts to sketch the division on. Esc stops.");
const NOT_THEIRS: &str = msg!("That face is not of the parts being divided: click one of theirs.");

/// `part`'s division as the model has it now, in model coordinates.
fn division(m: &Model, part: &bimcraft_core::parts::BuildingPart) -> Option<Recipe> {
    let recipe = part.division.as_ref()?;
    Some(m.element_frame(part.host)?.model_recipe(recipe))
}

/// The face a division was sketched on: its first cut drawn on a face that divides every piece it meets.
fn sketched_on(r: &Recipe) -> Option<Frame> {
    r.cuts.iter().find_map(|d| match &d.cut {
        Cut::Sketch { frame, .. } if d.of.is_none() => Some(*frame),
        _ => None,
    })
}

/// Whether triangle `t` lies in `f`'s plane, facing the same way.
fn in_plane(t: &[[f64; 3]; 3], f: &Face) -> bool {
    let (a, b) = (sub(t[1], t[0]), sub(t[2], t[0]));
    let n = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let len = dot(n, n).sqrt();
    len > 1e-9 && dot(n, f.normal) / len > 0.999 && t.iter().all(|q| dot(sub(*q, f.at), f.normal).abs() <= 1.0)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A length as the Options Bar shows it: blank for none, a whole number of mm, or a decimal.
fn mm(v: f64) -> String {
    if v == v.round() { format!("{v:.0}") } else { format!("{v}") }
}

impl BimApp {
    /// The selected parts Divide Parts divides: all of the selection, each a part neither divided already nor a piece of a
    /// division (Edit Division divides those again). Empty when it can't.
    pub(crate) fn dividable(&self) -> Vec<Id> {
        let m = &self.session.model;
        let ok = |id: &Id| matches!(m.elements.get(*id), Some(Element::Part(p)) if p.division.is_none() && p.piece.is_none());
        if self.selection.iter().all(ok) { self.selection.clone() } else { Vec::new() }
    }

    /// The divided part whose division Edit Division takes up: the one divided part selected, or the one part the pieces
    /// selected were divided from.
    pub(crate) fn division_to_edit(&self) -> Option<Id> {
        let m = &self.session.model;
        let mut of = None;
        for id in &self.selection {
            let Some(Element::Part(p)) = m.elements.get(*id) else { return None };
            let divided = match (&p.division, p.piece.as_ref().and(p.parent)) {
                (Some(_), _) => *id,
                (None, Some(parent)) => m.elements.id(parent)?,
                (None, None) => return None,
            };
            if of.is_some_and(|o| o != divided) {
                return None;
            }
            of = Some(divided);
        }
        of
    }

    /// Divide Parts on `parts`: the 3D view, waiting for a click on the face to sketch on.
    pub(crate) fn divide_parts(&mut self, parts: Vec<Id>) {
        let was = self.selection.clone();
        self.dividing = Some(Dividing { parts, face: None, gap: "0".into(), depth: String::new(), was, edit: false });
        self.tool = Tool::Modify;
        self.open_sheet(Sheet::View3D);
        self.status = Status { text: t(PICK_FACE).into(), error: false };
    }

    /// Edit Division on `part`: its sketch on the face it was drawn on, its Gap and Depth as they were; a division with no
    /// line drawn on a face asks for one, as Divide Parts does.
    pub(crate) fn edit_division(&mut self, part: Id) {
        let m = &self.session.model;
        let Some(Element::Part(p)) = m.elements.get(part) else { return };
        let recipe = division(m, p).unwrap_or_default();
        let face = sketched_on(&recipe);
        let drawn = face.map(|f| drawn::sketched(&recipe, &f)).unwrap_or_default();
        let was = self.selection.clone();
        let depth = drawn.depth.map(mm).unwrap_or_default();
        self.dividing = Some(Dividing { parts: vec![part], face: None, gap: mm(drawn.gap), depth, was, edit: true });
        self.tool = Tool::Modify;
        self.open_sheet(Sheet::View3D);
        match face {
            Some(f) => self.sketch_on(f, None, drawn.lines),
            None => self.status = Status { text: t(PICK_FACE).into(), error: false },
        }
    }

    /// Whether the sketch is a division's.
    pub(crate) fn sketching_division(&self) -> bool {
        self.sketch.as_ref().is_some_and(|sk| sk.of == Of::Divide)
    }

    /// Whether a click in the 3D view is the face Divide Parts waits for.
    pub(crate) fn picking_face(&self) -> bool {
        self.dividing.as_ref().is_some_and(|d| d.face.is_none()) && self.sketch.is_none()
    }

    /// A click on `face` (none where the model isn't under it) while Divide Parts waits for one: one of the parts', or of
    /// what they are parts of, begins the sketch there.
    pub(crate) fn divide_face_pick(&mut self, face: Option<Face>) {
        let Some(d) = &self.dividing else { return };
        let m = &self.session.model;
        // A part's own face; or, where the view shows what they are parts of, a face of that lying where one of theirs does.
        let theirs = |f: &Face| {
            d.parts.iter().any(|&id| match m.elements.get(id) {
                _ if id == f.owner => true,
                Some(Element::Part(p)) if m.elements.id(p.host) == Some(f.owner) => {
                    m.part_solid(p).is_ok_and(|tris| tris.iter().any(|t| in_plane(t, f)))
                }
                _ => false,
            })
        };
        let Some(face) = face.filter(theirs) else {
            self.status = Status { text: t(NOT_THEIRS).into(), error: true };
            return;
        };
        let level = bimcraft_core::workplane::Frame { origin: face.at.map(f64::round), normal: face.normal };
        self.sketch_on(level.into(), Some(face.at), Vec::new());
    }

    /// The sketch begun on `face`, the 3D view turned square to it (about `at`, else where it looks), with `lines` to edit.
    fn sketch_on(&mut self, face: Frame, at: Option<[f64; 3]>, lines: Vec<Curve>) {
        if let Some(d) = &mut self.dividing {
            d.face = Some(face);
        }
        if let Some(cam) = self.orbit.camera {
            let n = face.normal;
            let len = dot(n, n).sqrt().max(1e-12);
            let azimuth = n[1].atan2(n[0]).to_degrees();
            let elevation = (n[2] / len).asin().to_degrees().clamp(-89.0, 89.0);
            let azimuth = if n[0].abs() < 1e-9 && n[1].abs() < 1e-9 { cam.azimuth } else { azimuth };
            let mut turned = Camera { azimuth, elevation, target: at.unwrap_or(cam.target), lens: None, ..cam };
            // The parts filling most of the view, seen on the face.
            if let Some((target, span)) = self.parts_on(&face) {
                turned.target = target;
                let fits = (f64::from(cam.width) / span.0.max(1.0)).min(f64::from(cam.height) / span.1.max(1.0)) * 0.75;
                if fits.is_finite() && fits > 0.0 {
                    turned.scale = fits;
                }
            }
            self.orbit.camera = Some(turned);
        }
        self.start_sketch(Of::Divide, lines.into_iter().map(SketchLine::from).collect());
    }

    /// Where the parts being divided are on `face`: the model point at the middle of what they span on it, and how wide and
    /// high that is, mm. None when none has a solid.
    fn parts_on(&self, face: &Frame) -> Option<([f64; 3], (f64, f64))> {
        let m = &self.session.model;
        let d = self.dividing.as_ref()?;
        let seen: Vec<(f64, f64)> = d
            .parts
            .iter()
            .filter_map(|id| match m.elements.get(*id) {
                Some(Element::Part(p)) => m.part_solid(p).ok(),
                _ => None,
            })
            .flatten()
            .flatten()
            .filter_map(|q| face.local(q).map(|(p, _)| (p.0, p.1)))
            .collect();
        let lo = seen.iter().copied().reduce(|a, b| (a.0.min(b.0), a.1.min(b.1)))?;
        let hi = seen.iter().copied().reduce(|a, b| (a.0.max(b.0), a.1.max(b.1)))?;
        let middle = bimcraft_core::geom::P2((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0);
        Some((face.point(middle, 0.0)?, (hi.0 - lo.0, hi.1 - lo.1)))
    }

    /// Esc while Divide Parts waits for its face: given up, the selection as it was.
    pub(crate) fn stop_face_pick(&mut self) -> bool {
        if !self.picking_face() {
            return false;
        }
        if let Some(d) = self.dividing.take() {
            self.selection = d.was;
        }
        self.status = Status::default();
        true
    }

    /// Cancel Edit Mode of a division: nothing divided, the selection as it was.
    pub(crate) fn cancel_divide(&mut self) {
        if let Some(d) = self.dividing.take() {
            self.selection = d.was;
        }
    }

    /// Finish Edit Mode of a division: its lines made into cuts and each part divided by them, in one undo step; the pieces
    /// selected. What can't be divided is said, the sketch left as it was.
    pub(crate) fn finish_divide(&mut self) {
        let (Some(d), Some(sk)) = (&self.dividing, &self.sketch) else { return };
        let Some(face) = d.face else { return };
        let m = &self.session.model;
        let units = m.units;
        let gap = match d.gap.trim() {
            "" => Some(0.0),
            g => units.parse(g).filter(|v| *v >= 0.0),
        };
        let depth = match d.depth.trim() {
            "" => Some(None),
            g => units.parse(g).filter(|v| *v > 0.0).map(Some),
        };
        let (Some(gap), Some(depth)) = (gap, depth) else {
            let text = t("Gap is a width of 0 or more, and Depth a depth more than 0, or blank to cut through.");
            self.status = Status { text: text.into(), error: true };
            return;
        };
        let lines: Vec<Curve> = sk.lines.iter().map(|l| l.curve.clone()).collect();
        let mut commands: Vec<(&str, Value)> = Vec::new();
        for &id in &d.parts {
            let Some(Element::Part(p)) = m.elements.get(id) else { continue };
            let had = division(m, p).unwrap_or_default();
            match drawn::redivided(&had, &face, &lines, gap, depth) {
                Ok(r) => commands.push(("divide_parts", json!({"id": id, "cuts": r.cuts, "offsets": r.offsets}))),
                Err(e) => {
                    self.status = Status { text: e, error: true };
                    return;
                }
            }
        }
        if !self.graphics_shown().parts_visibility.unwrap_or_default().shows_parts()
            && self.graphics_now().parts_controlled_by(m).is_none()
            && let Some(view) = self.graphic_view()
        {
            commands.push(("graphics", json!({"view": view, "parts_visibility": "parts"})));
        }
        let label = if d.edit { msg!("Edit Division") } else { msg!("Divide Parts") };
        match self.exec_batch(label, &commands) {
            Ok(done) => {
                let pieces = done.iter().filter_map(|v| v["pieces"].as_array()).flatten().filter_map(|p| p["id"].as_u64());
                self.selection = pieces.collect();
                self.sketch = None;
                self.dividing = None;
                self.tool = Tool::Modify;
                self.forget_picks();
                self.status = Status::default();
            }
            Err(e) => self.status = Status { text: e, error: true },
        }
    }

    /// The Options Bar while sketching a division: the joint's width, and how deep an outline cuts (blank: through).
    pub(crate) fn divide_options(&mut self, ui: &mut Ui) {
        if !self.sketching_division() {
            return;
        }
        let Some(d) = &mut self.dividing else { return };
        let l = ui.label(t("Gap"));
        ui.add(egui::TextEdit::singleline(&mut d.gap).desired_width(60.0)).labelled_by(l.id);
        let l = ui.label(t("Depth"));
        ui.add(egui::TextEdit::singleline(&mut d.depth).desired_width(60.0).hint_text(t("through"))).labelled_by(l.id);
    }
}
