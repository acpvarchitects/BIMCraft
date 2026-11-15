//! FE-25: a selected form's shape handles in {3D}, as in a plan or an elevation (FE-5, F5). With one extrusion selected, an
//! arrow at the middle of each edge of its profile at mid-depth, pointing out of the solid, and one on its End and its Start
//! at the profile's middle, pointing out along the plane's normal; with a Blend, its base's edges at its base and its First and
//! Second End. A drag moves that side (`drag_edge_to`, the plans' own) or that end along the arrow, the pointer read along the
//! arrow as the view was when the button went down: live, one step of the history once the button is up; Escape, Undo, Redo
//! and another view shown give it up, the form put back. A greyed handle refuses the drag saying why, and the view doesn't turn.
//!
//! An arrow the view sees end-on (its way within about 75 degrees of the way the view looks) can't be dragged along: it isn't
//! drawn, as a plan draws no handle on a side it sees flat. Revolves, sweeps and swept blends have no handles in any view:
//! their shapes are their profiles' and paths', changed by sketching them again.
use super::select_box::Picked;
use super::shape_handles::ShapeHandle;
use super::sketch::reader;
use super::*;
use bimcraft_core::family::Extent;
use bimcraft_core::family::forms::FormKind;
use bimcraft_core::family::handles as hs;
use bimcraft_core::family::sketch::{FormCurve, FormLine};
use bimcraft_core::geom::P2;

/// How near a handle, in points, a press takes it: the plans' reach.
pub(crate) const REACH: f32 = 10.0;
/// How long an arrow is on screen, in points.
pub(crate) const ARROW: f32 = 9.0;
/// The least an arrow's way may show on screen, against a way across the view: less, and the view sees it end-on.
const SEEN: f32 = 0.25;

/// What a handle in {3D} drags: a side of the profile, or the End (true) or the Start.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Grab {
    Edge(ShapeHandle),
    End(bool),
}

/// A handle in {3D}: where it is in the family's axes, the way it points (and drags) there, what it drags, and why it can't be
/// dragged, if it can't (it is drawn greyed).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Handle3d {
    pub(crate) at: [f64; 3],
    pub(crate) way: [f64; 3],
    pub(crate) grab: Grab,
    pub(crate) blocked: Option<String>,
}

/// A handle held in {3D}: where the button went down, the points on screen one millimetre along the handle's way makes as the
/// view was then, and for an End or Start, its number before and the form's tag. A greyed handle is held too, doing nothing,
/// so the drag doesn't turn the view.
#[derive(Clone)]
pub(crate) struct Drag3d {
    press: egui::Pos2,
    per_mm: egui::Vec2,
    end: Option<(bool, String, u64)>,
    live: bool,
    last: f64,
    /// The last pass {3D} was drawn in: shown no longer, the drag is given up.
    pub(super) seen: u64,
}

fn add(a: [f64; 3], b: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

/// The middle of `lines`' points (their starts, a circle's centre), read by `num`.
fn middle(lines: &[FormLine], num: &mut dyn FnMut(&Expr) -> Result<f64, String>) -> Option<P2> {
    let pts: Vec<P2> = lines
        .iter()
        .filter(|l| !l.axis)
        .map(|l| match &l.curve {
            FormCurve::Line { start, .. } => Some(P2(num(&start[0]).ok()?, num(&start[1]).ok()?)),
            FormCurve::Circle { centre, .. } => Some(P2(num(&centre[0]).ok()?, num(&centre[1]).ok()?)),
        })
        .collect::<Option<_>>()?;
    (!pts.is_empty()).then(|| bimcraft_core::geom::centroid(&pts))
}

impl FamilyEditor {
    /// The selected extrusion's (or Blend's) handles in {3D}, as the type previewed flexes it; none while a tool is in hand.
    pub(crate) fn handles_3d(&self) -> Vec<Handle3d> {
        if !self.handles_shown() {
            return Vec::new();
        }
        let Some(i) = self.selected_form() else { return Vec::new() };
        let Ok(drawn) = self.family() else { return Vec::new() };
        let Some(form) = drawn.forms.get(i) else { return Vec::new() };
        let kind = form.kind();
        if !matches!(kind, FormKind::Extrusion | FormKind::Blend) || !form.mesh.is_empty() {
            return Vec::new();
        }
        let values = self.values();
        let shown = drawn.flexed(&values).map(std::borrow::Cow::into_owned).unwrap_or_else(|_| drawn.clone());
        let flexed = if form.guid.is_nil() { shown.forms.get(i) } else { shown.forms.iter().find(|f| f.guid == form.guid) };
        let Some(flexed) = flexed else { return Vec::new() };
        let Ok(frame) = shown.frame(&flexed.host) else { return Vec::new() };
        let mut num = reader(&values);
        let (Ok(from), Ok(to)) = (num(&flexed.from), num(&flexed.to)) else { return Vec::new() };
        let pinned = self.pinned_refusal(&[Picked::Form(i)]);
        let mut out = Vec::new();
        // The sides: an extrusion's at mid-depth, a Blend's base's at its base.
        let depth = if kind == FormKind::Extrusion { (from + to) / 2.0 } else { from };
        for h in hs::edge_handles(&flexed.sketch, &mut num) {
            let way: [f64; 3] = std::array::from_fn(|k| frame.x[k] * h.out.0 + frame.y[k] * h.out.1);
            let blocked = pinned.clone().or_else(|| match form.sketch.get(h.line).map(|l| drawn.edge_held(form.guid, l.guid)) {
                Some(Some(held)) => Some(super::shape_handles::held_said(&held, false)),
                Some(None) => super::shape_handles::greyed(&form.sketch, h.line, h.out, &mut num),
                None => Some(t("The profile can't take this: its lines would cross, overlap or no longer close.").to_string()),
            });
            let at = frame.point(h.at, depth);
            out.push(Handle3d {
                at,
                way,
                grab: Grab::Edge(ShapeHandle { at: h.at, way, line: h.line, out: h.out, blocked: blocked.clone() }),
                blocked,
            });
        }
        // The End and the Start, at the middle of the profile there (a Blend's top at its End).
        let base = middle(&flexed.sketch, &mut num);
        let top = if flexed.top.is_empty() { base } else { middle(&flexed.top, &mut num) };
        let Some(ft) = self.forms.get(i) else { return out };
        for (end, at, mid) in [(false, from, base), (true, to, top)] {
            let Some(mid) = mid else { continue };
            let (text, extent) = if end { (&ft.to, Extent::End) } else { (&ft.from, Extent::Start) };
            let blocked = pinned.clone().or_else(|| self.end_blocked(Some(&drawn), form.guid, text, extent));
            let k = if end { 1.0 } else { -1.0 };
            out.push(Handle3d { at: frame.point(mid, at), way: frame.z.map(|z| z * k), grab: Grab::End(end), blocked });
        }
        out
    }

    /// The handles in {3D} where `project` shows them, with the way each points on screen; those the view sees end-on left out.
    pub(crate) fn handles_3d_on(
        &self,
        project: &dyn Fn([f64; 3]) -> Option<egui::Pos2>,
        scale: f64,
    ) -> Vec<(egui::Pos2, egui::Vec2, Handle3d)> {
        // A millimetre, as the view is scaled: what a way across the view shows as.
        let k = 40.0 / scale.max(1e-9);
        self.handles_3d()
            .into_iter()
            .filter_map(|h| {
                let at = project(h.at)?;
                let along = project(add(h.at, h.way, k))? - at;
                // Across: the longer of two ways square to the arrow, as the view shows them.
                let across = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
                    .into_iter()
                    .map(|a: [f64; 3]| {
                        let d = a[0] * h.way[0] + a[1] * h.way[1] + a[2] * h.way[2];
                        add(a, h.way, -d)
                    })
                    .filter_map(|a| {
                        let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
                        (n > 1e-6).then(|| project(add(h.at, a, k / n)).map(|p| (p - at).length()))?
                    })
                    .fold(0.0_f32, f32::max);
                (along.length() > SEEN * across && along.length() > 1e-3).then(|| (at, along / k as f32, h))
            })
            .collect()
    }

    /// A press at `press` on handle `h`, which `per_mm` (points on screen a millimetre along it makes) says how to read the
    /// pointer along, in the pass `seen`: held. A greyed one is held doing nothing, saying why.
    pub(crate) fn grab_3d(&mut self, h: &Handle3d, press: egui::Pos2, per_mm: egui::Vec2, seen: u64) -> Result<(), String> {
        let mut drag = Drag3d { press, per_mm, end: None, live: false, last: 0.0, seen };
        let result = match (&h.blocked, &h.grab) {
            (Some(why), _) => Err(why.clone()),
            (None, Grab::Edge(sh)) => {
                let values = self.values();
                let shown = self.family().map(|f| f.flexed(&values).map(std::borrow::Cow::into_owned).unwrap_or(f));
                match shown.and_then(|f| self.edge_drag(sh, (None, None, seen), &f)) {
                    Ok(d) => {
                        self.views.edge_drag = Some(d);
                        drag.live = true;
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            (None, Grab::End(end)) => match self.selected_form().and_then(|i| self.forms.get(i)) {
                Some(ft) => {
                    drag.end = Some((*end, if *end { ft.to.clone() } else { ft.from.clone() }, ft.tag));
                    drag.live = true;
                    Ok(())
                }
                None => Err(String::new()),
            },
        };
        self.views.drag3d = Some(drag);
        result
    }

    /// The handle held in {3D}, the pointer at `now`: the side or the end that far along its arrow, live; a refusal leaves the
    /// form as it was, saying why.
    pub(crate) fn drag_3d_to(&mut self, now: egui::Pos2) -> Result<(), String> {
        let Some(drag) = self.views.drag3d.as_mut().filter(|d| d.live) else { return Ok(()) };
        let n = drag.per_mm.length_sq();
        if n <= 1e-12 {
            return Ok(());
        }
        let d = f64::from((now - drag.press).dot(drag.per_mm) / n);
        if !d.is_finite() || (d - drag.last).abs() < 1e-9 {
            return Ok(());
        }
        drag.last = d;
        match drag.end.clone() {
            None => {
                let Some(way) = self.views.edge_drag.as_ref().map(super::shape_handles::EdgeDrag::way) else { return Ok(()) };
                self.drag_edge_to(way.map(|w| w * d))
            }
            Some((end, before, tag)) => {
                let Some(ft) = self.forms.iter_mut().find(|f| f.tag == tag) else {
                    self.views.drag3d = None;
                    return Ok(());
                };
                let Some(x) = before.trim().parse::<f64>().ok().filter(|x| x.is_finite()) else {
                    return Err(t("This end is tied to a parameter or worked out by a formula: change it in Family Types.").to_string());
                };
                // The Start's arrow points back along the normal.
                let text = if end { &mut ft.to } else { &mut ft.from };
                *text = if d.abs() < 1e-9 { before } else { number_text(x + if end { d } else { -d }) };
                Ok(())
            }
        }
    }

    /// The handle held in {3D} let go: it stays where the drag left it.
    pub(crate) fn let_go_3d(&mut self) {
        if self.views.drag3d.take().is_some() {
            self.views.edge_drag = None;
        }
    }

    /// The End or Start being dragged in {3D} given up, its number put back. Whether there was a drag in {3D}.
    pub(super) fn cancel_3d(&mut self) -> bool {
        let Some(d) = self.views.drag3d.take() else { return false };
        if let Some((end, before, tag)) = d.end
            && let Some(f) = self.forms.iter_mut().find(|f| f.tag == tag)
        {
            *(if end { &mut f.to } else { &mut f.from }) = before;
        }
        true
    }

    /// Whether a handle is held in {3D}, and so the view doesn't turn.
    pub(crate) fn holding_3d(&self) -> bool {
        self.views.drag3d.is_some()
    }

    /// The pass a handle held in {3D} was last drawn in, kept up; a drag of another view's given up.
    pub(crate) fn keep_3d(&mut self, pass: u64) {
        if self.views.drag3d.as_ref().is_some_and(|d| d.seen + 1 < pass)
            || self.views.edge_drag.as_ref().is_some_and(|d| d.which.is_some())
            || self.views.end_drag.is_some()
            || self.views.moving.is_some()
        {
            self.cancel_drags();
        }
        if let Some(d) = self.views.drag3d.as_mut() {
            d.seen = pass;
        }
    }
}
