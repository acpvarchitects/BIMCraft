//! The family document's {3D}, drawn by the project's 3D view (`orbit.rs`): the family staged in a scratch model, each of its
//! parts a family of its own, so the project's tints colour each and its pick finds each. Its picture is the card's or the
//! processor's, as the project's is, and the hand, the wheel, the 3D mouse, the triad and the ViewCube move its camera.
//!
//! What it highlights (T11), as the project's 3D view does: the form, void or nested instance selected, in the selection's
//! colour, and the form or nested instance under the pointer, which a click would select, in the hover colour, each tinted
//! halfway with its edges whole while what stands in front of it still hides it. A void, never seen, is drawn see-through
//! while it is selected. A reference plane or line selected in a plan or an elevation is laid over the picture in the
//! selection's colour, depth tested as the work plane is (`view3d::PlaneMark`).
use super::*;
use bimcraft_core::family::views::Part;
use bimcraft_core::ids::Guid;
use bimcraft_core::model::{Element, Id, View3D};
use bimcraft_core::render::Rgb;
use bimcraft_core::view3d::{Camera, PlaneMark};

/// What {3D} draws highlighted: a part of the family (`Part`), a reference plane or a reference line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Lit {
    Part(Part),
    Plane(Guid),
    Line(Guid),
}

/// The material a selected void is drawn in: see-through, the selection's tint over it. The scratch project's own.
const VOID: &str = "(selected void)";

/// The family's {3D} staged: the scratch model its scene is made of, each part of the family placed in it as a family of its
/// own, and which part each element is; built again only when what it shows changes (`key`).
pub(crate) struct FamilyScene {
    pub(crate) model: Arc<Model>,
    pub(crate) part: BTreeMap<Id, Part>,
    pub(crate) key: u64,
    /// Each element's GUIDs as [`rekey`] made them its own, to the family's: what a face clicked on it is named by in the
    /// family (FE-16c-1).
    pub(crate) guids: BTreeMap<Id, BTreeMap<Guid, Guid>>,
}

impl FamilyScene {
    /// The tints of what is `lit`: each part's elements in its colour.
    pub(crate) fn tints(&self, lit: &[(Lit, Rgb)]) -> Vec<(Id, Rgb)> {
        let mut out = Vec::new();
        for (l, c) in lit {
            if let Lit::Part(p) = l {
                out.extend(self.part.iter().filter(|(_, q)| *q == p).map(|(id, _)| (*id, *c)));
            }
        }
        out
    }
}

impl FamilyEditor {
    /// What is selected, as {3D} shows it: forms, nested instances, reference planes and reference lines, one or several
    /// (FE-1).
    pub(crate) fn selected_3d(&self) -> Vec<Lit> {
        use super::select_box::Picked;
        if self.adaptive.is_some() {
            return Vec::new();
        }
        self.selection
            .items()
            .iter()
            .filter(|p| self.holds(**p))
            .filter_map(|p| match p {
                Picked::Form(i) => Some(Lit::Part(Part::Form(*i))),
                Picked::Nested(i) => Some(Lit::Part(Part::Nested(*i))),
                Picked::Line(g) => Some(Lit::Line(*g)),
                Picked::Plane(g) => Some(Lit::Plane(*g)),
                _ => None,
            })
            .collect()
    }

    /// What {3D} highlights, and in which colour: what is selected, in the selection's, and the part under the pointer (when
    /// it isn't the one selected), in the hover colour, as the project's 3D view tints its elements. None while sketching.
    pub(crate) fn lit_3d(&self) -> Vec<(Lit, Rgb)> {
        if self.adaptive.is_some() || self.sketching() {
            return Vec::new();
        }
        let selected = self.selected_3d();
        let hovered = self.views.hover3d.and_then(|(_, _, p)| p).map(Lit::Part).filter(|h| !selected.contains(h));
        let colour = |c: Color32| Rgb(c.r(), c.g(), c.b());
        selected
            .into_iter()
            .map(|s| (s, colour(crate::style::canvas_pal().accent)))
            .chain(hovered.map(|h| (h, colour(crate::style::canvas_pal().hover))))
            .collect()
    }

    /// A click in {3D} on `part` (none: on nothing): the form or nested instance selected, as a click selects it in a plan; on
    /// nothing, the selection let go.
    pub(crate) fn click_3d(&mut self, part: Option<Part>) {
        match part {
            Some(Part::Form(i)) => {
                self.select_only_form(i);
            }
            Some(Part::Nested(i)) => self.select_nested(Some(i)),
            None => self.let_everything_go(),
        }
    }

    /// The family's {3D} staged anew where what it shows has changed: the draft, the type and its values, Preview Visibility,
    /// the Detail Level, Temporary Hide/Isolate, or a void selected. Why it can't be, in `problem`.
    pub(crate) fn stage_3d(&mut self, base: &Model) {
        let placed_only = self.document.preview_visibility;
        let void = self.selected_form().filter(|i| self.forms.get(*i).is_some_and(|f| f.void));
        let result = self.family().and_then(|f| {
            let key = {
                let mut h = DefaultHasher::new();
                (
                    serde_json::to_string(&f).unwrap_or_default(),
                    serde_json::to_string(&self.preview_params).unwrap_or_default(),
                    self.picked,
                    self.name.trim(),
                    placed_only,
                    self.temporary(),
                    self.views.detail,
                    void,
                    self.views.kept.key(None),
                )
                    .hash(&mut h);
                (base.materials.len(), base.images.len()).hash(&mut h);
                h.finish()
            };
            if self.scene3d.as_ref().is_some_and(|s| s.key == key) {
                return Ok(None);
            }
            self.staged_3d(base, &f, void, key).map(Some)
        });
        match result {
            Ok(Some(scene)) => (self.scene3d, self.problem) = (Some(scene), None),
            Ok(None) => self.problem = None,
            Err(e) => self.problem = Some(e),
        }
    }

    /// Family `f` staged for {3D}, `void` the void selected: a form the type hides (or its Visibility Settings leave out of {3D}
    /// at the document's Detail Level) drawn ghosted, as Revit's family editor draws it, unless Preview Visibility is on, and
    /// what Temporary Hide/Isolate hides left out. The family is placed once with its solids hidden (a door's or a window's
    /// opening still cuts its wall), and each solid form and nested instance beside it as a family of its own, where the family
    /// is (a door's or a window's at its opening's frame, a generic family with its wall's thickness, as the views' ghosts are).
    fn staged_3d(&self, base: &Model, f: &Family, void: Option<usize>, key: u64) -> Result<FamilyScene, String> {
        let values = self.values();
        let by_view = bimcraft_core::family::views::hidden_in(f, &values, None, self.views.detail, 0.0);
        let shown = if self.document.preview_visibility { f.clone() } else { ghosted(f, &values, &by_view).unwrap_or_else(|| f.clone()) };
        let shown = temp_hide::without(&shown, |i| self.temp_hidden_form(i)).unwrap_or(shown);
        let off = || Some(Expr::Number(0.0));
        let hides = |e: &Option<Expr>| e.as_ref().is_some_and(|v| crate::adaptive_editor::worth(v, &values) == Some(0.0));
        let mut host = shown.clone();
        for form in host.forms.iter_mut().filter(|form| !form.void) {
            form.visible = off();
        }
        for n in &mut host.nested_instances {
            n.visible = off();
        }
        let mut s = Session::new();
        s.model = Arc::new(self.scratch(base, &host)?);
        let name = self.name.trim();
        let type_name = self.types.get(self.picked).map(|(n, _)| n.clone()).unwrap_or_default();
        let mut place = json!({"type": type_name, "at": [0, 0], "params": self.preview_params});
        let hosted = shown.category.hosted();
        if hosted {
            let frame = s.model.elements.values().find_map(|e| match e {
                Element::Door(o) | Element::Window(o) => s.model.opening_frame(o),
                _ => None,
            });
            if let Some((p, _, z)) = frame {
                (place["at"], place["offset"], place["params"]) = (json!([p.0, p.1]), json!(z), json!({}));
            }
        }
        super::at_origin(&shown, &mut place);
        let parts = (0..shown.forms.len())
            .filter(|i| shown.forms.get(*i).is_some_and(|form| (!form.void || void == Some(*i)) && !hides(&form.visible)))
            .map(Part::Form)
            .chain((0..shown.nested_instances.len()).filter(|k| !hides(&shown.nested_instances[*k].visible)).map(Part::Nested));
        let (mut part, mut guids) = (BTreeMap::new(), BTreeMap::new());
        s.model_mut()
            .materials
            .insert(VOID.into(), bimcraft_core::model::Material { color: [40, 120, 230], transparency: 55, ..Default::default() });
        for (k, p) in parts.enumerate() {
            let mut g = alone(&shown, p);
            // Each part is a family of its own beside the family, and a project holds a GUID once: its forms' GUIDs, and the
            // lists that name them (FE-16), are made its own, or the project would give the copies new ones and break the lists.
            let back = rekey(&mut g, k as u64 + 1);
            if hosted {
                generic(&mut g, &values);
            }
            let own = format!("{name} (part {})", k + 1);
            s.model_mut().families.insert(own.clone(), g);
            let mut at = place.clone();
            at["family"] = json!(own);
            let made = s.execute("component", &at)?;
            if let Some(id) = made["id"].as_u64() {
                part.insert(id, p);
                guids.insert(id, back);
            }
        }
        // FE-18: a part {3D} overrides, or reveals in magenta, drawn so.
        let apart = self.apart_forms(None, f, &values, &by_view);
        let parted: Vec<(bimcraft_core::ids::Guid, bimcraft_core::graphics::Override)> = part
            .iter()
            .filter_map(|(id, p)| {
                let Part::Form(i) = p else { return None };
                let o = apart.iter().find(|(k, _)| k == i)?.1.clone();
                Some((s.model.elements.guid(*id)?, o))
            })
            .collect();
        let m = s.model_mut();
        // The view it is seen in: the document's Detail Level, the scratch project's levels hidden, and the door's or the window's
        // wall while Temporary Hide/Isolate hides it.
        let mut graphics = bimcraft_core::graphics::Graphics { detail: self.views.detail, ..Default::default() };
        let hidden = bimcraft_core::graphics::Override { hidden: true, ..Default::default() };
        graphics.categories.insert("level".into(), hidden.clone());
        if self.temp_hidden_host() {
            graphics.categories.insert("wall".into(), hidden);
        }
        // {3D}'s own Visibility/Graphics and visual style (FE-18). A door's or a window's parts stand as generic families of their
        // own, so its category's override is theirs.
        let mut own = self.views.kept.drawn_graphics(None);
        if hosted {
            let of = if shown.category == Category::Door { "door" } else { "window" };
            if let Some(o) = own.categories.get(of).cloned() {
                own.categories.insert("generic".into(), o);
            }
        }
        super::view_graphics::lay_over(&mut graphics, &own);
        graphics.elements.extend(parted);
        m.insert(Element::View3D(View3D {
            name: "{3D}".into(),
            azimuth: AZIMUTH,
            elevation: ELEVATION,
            graphics,
            // Selection Box (FE-23a): the box BX keeps, while it keeps one.
            section_box: self.views.kept.section_box.map(|(min, max)| bimcraft_core::model::SectionBox { min, max, on: true }),
            perspective: None,
            camera: None,
            display: self.views.kept.display,
            work_plane: None,
        }));
        Ok(FamilyScene { model: Arc::clone(&s.model), part, key, guids })
    }

    /// What {3D} lays over its picture through `cam`: Work Plane > Show's plane, and the reference plane or line selected.
    pub(crate) fn marks_3d(&self, cam: &Camera) -> Vec<PlaneMark> {
        let Ok(f) = self.family() else { return Vec::new() };
        let values = self.values();
        let extents = bimcraft_core::family::views::extents(&f, &values);
        let mut marks: Vec<PlaneMark> =
            self.shown_work_plane_3d().and_then(|shown| super::views::work_plane_mark(&shown, extents)).into_iter().collect();
        // Where a Modify tool in hand would put the forms it takes (FE-24).
        if self.views.shown.is_none() && self.views.modify.is_some() {
            marks.extend(self.views.ghost_3d.iter().copied());
        }
        let datums: Vec<(Lit, Rgb)> = self.lit_3d().into_iter().filter(|(l, _)| !matches!(l, Lit::Part(_))).collect();
        if !datums.is_empty() {
            let flexed = f.flexed(&values).map(std::borrow::Cow::into_owned).unwrap_or_else(|_| f.clone());
            for (datum, c) in datums {
                marks.extend(datum_mark(cam, &flexed, datum, Color32::from_rgb(c.0, c.1, c.2), extents));
            }
        }
        marks
    }
}

/// Family `f` with only part `p` of it seen: a form alone with the voids that cut it (a void, made a see-through solid), a
/// nested instance alone.
fn alone(f: &Family, p: Part) -> Family {
    let off = || Some(Expr::Number(0.0));
    let mut g = f.clone();
    for (k, form) in g.forms.iter_mut().enumerate() {
        if p == Part::Form(k) {
            if form.void {
                form.void = false;
                form.material = VOID.into();
            }
        } else if !form.void {
            form.visible = off();
        }
    }
    for (k, n) in g.nested_instances.iter_mut().enumerate() {
        if p != Part::Nested(k) {
            n.visible = off();
        }
    }
    if let Part::Form(k) = p
        && f.forms.get(k).is_some_and(|x| !x.void)
    {
        solid_cutters_as_voids(f, &mut g, k);
    }
    // Its symbols are drawn in plans only.
    g.symbols.clear();
    g
}

/// Part `k` (a solid form) of `f` is `g` with the other solids hidden, and a hidden form cuts nothing: so the solids that cut
/// it (`cut_by`, `joined_by`, FE-16) are made voids in `g`, which are never drawn but cut as they cut it in the family. A void
/// listed after it cuts it by the order, one listed before it by `cut_by`. The other forms' lists go, as they are hidden.
fn solid_cutters_as_voids(f: &Family, g: &mut Family, k: usize) {
    let cutters: Vec<usize> = f.cutters(k).into_iter().filter(|m| !f.forms[*m].void).collect();
    for (i, form) in g.forms.iter_mut().enumerate().filter(|(i, _)| *i != k) {
        (form.cut_by, form.uncut_by, form.joined_by) = Default::default();
        if cutters.contains(&i) {
            form.void = true;
            form.visible = f.forms[i].visible.clone();
        }
    }
    let named: Vec<bimcraft_core::ids::Guid> = cutters.iter().map(|m| f.forms[*m].guid).collect();
    let target = &mut g.forms[k];
    target.joined_by.clear();
    target.cut_by.retain(|x| !named.contains(x));
    for m in cutters.into_iter().filter(|m| *m < k) {
        target.cut_by.push(f.forms[m].guid);
    }
}

/// Family `g` with the GUID of each of its parts made one of its own, from `salt`, wherever the family names it: a form's, a
/// sketch line's, a plane's, a line's, a model line's, a dimension's, a symbol's, a connector's, a nested family's or instance's.
/// A project holds a GUID once, and each part of {3D} is a family of its own beside the family, so the project would give the
/// copies new GUIDs and everything that names them by GUID (the cutting lists of FE-16, a dimension's references, a form's host
/// plane, a lock) would name what is gone. Datums, which every family has, keep theirs. Done on the family as text, so a place
/// that names a part by GUID is never missed.
fn rekey(g: &mut Family, salt: u64) -> BTreeMap<Guid, Guid> {
    let mut was: Vec<Guid> = Vec::new();
    was.extend(g.forms.iter().map(|x| x.guid));
    was.extend(g.forms.iter().flat_map(|x| x.all_lines().map(|l| l.guid)));
    was.extend(g.symbols.iter().map(|x| x.guid));
    was.extend(g.model_lines.iter().map(|x| x.guid));
    was.extend(g.connectors.iter().map(|x| x.guid));
    was.extend(g.planes.iter().filter(|x| !x.datum()).map(|x| x.guid));
    was.extend(g.lines.iter().map(|x| x.guid));
    was.extend(g.dimensions.iter().map(|x| x.guid));
    was.extend(g.nested.iter().map(|x| x.guid));
    was.extend(g.nested_instances.iter().map(|x| x.guid));
    was.retain(|x| !x.is_nil());
    was.sort();
    was.dedup();
    let seed = bimcraft_core::model::mix(0x4645_3136_6270_6172 ^ salt);
    let mut back = BTreeMap::new();
    let Ok(mut text) = serde_json::to_string(&*g) else { return back };
    let said = |x: &Guid| serde_json::to_value(x).ok().and_then(|v| v.as_str().map(str::to_string));
    for (i, old) in was.iter().enumerate() {
        let new = Guid::seeded(seed, i as u64);
        if let (Some(from), Some(to)) = (said(old), said(&new)) {
            text = text.replace(&from, &to);
            back.insert(new, *old);
        }
    }
    match serde_json::from_str::<Family>(&text) {
        Ok(made) => *g = made,
        Err(_) => back.clear(),
    }
    back
}

/// A door's or a window's part made a generic family, its wall's thickness a parameter, so it stands as a component at the
/// opening's frame beside the opening rather than a second opening in the wall.
fn generic(g: &mut Family, values: &BTreeMap<String, bimcraft_core::family::Val>) {
    let wall = match values.get(bimcraft_core::family::WALL) {
        Some(bimcraft_core::family::Val::Number(t)) => *t,
        _ => bimcraft_core::family::DEFAULT_WALL,
    };
    if !g.parameters.iter().any(|p| p.name == bimcraft_core::family::WALL)
        && let Ok(p) = serde_json::from_value(json!({"name": bimcraft_core::family::WALL, "value": wall}))
    {
        g.parameters.push(p);
    }
    g.category = Category::Generic;
    g.operation = None;
}

/// The reference plane or line `lit` names as a mark over {3D}'s picture, seen through `cam`, in colour `c`: the plane as much
/// of it as the family takes up (`extents`) and a margin round, as Work Plane > Show lays one; the line a strip a few pixels
/// wide, turned to the eye. `f` is the family as flexed, with its planes and lines where the type puts them.
fn datum_mark(cam: &Camera, f: &Family, lit: Lit, c: Color32, extents: ([f64; 3], [f64; 3])) -> Option<PlaneMark> {
    match lit {
        Lit::Plane(g) => {
            let frame = f.frame(&PlaneRef::Plane(g)).ok()?;
            let plane = bimcraft_core::workplane::Frame { origin: frame.origin, normal: frame.z };
            let p = crate::workplane::patch(plane, extents, super::views::WORK_PLANE_MARGIN, [0.0; 3])?;
            Some(PlaneMark {
                origin: p.origin,
                axes: p.axes,
                lo: p.lo,
                hi: p.hi,
                step: 0.0,
                fill: c.gamma_multiply(0.25).to_array(),
                grid: [0; 4],
                edge: c.to_array(),
                edge_width: 2.0,
            })
        }
        Lit::Line(g) => {
            let l = f.lines.iter().find(|l| l.guid == g)?;
            let d: [f64; 3] = std::array::from_fn(|i| l.end[i] - l.start[i]);
            let long = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if !long.is_finite() || long < 1e-9 {
                return None;
            }
            let u = d.map(|v| v / long);
            // Across the line and square to the eye, as wide as 3 pixels.
            let (a, e) = (cam.azimuth.to_radians(), cam.elevation.to_radians());
            let back = [e.cos() * a.cos(), e.cos() * a.sin(), e.sin()];
            let across = [u[1] * back[2] - u[2] * back[1], u[2] * back[0] - u[0] * back[2], u[0] * back[1] - u[1] * back[0]];
            let n = (across[0] * across[0] + across[1] * across[1] + across[2] * across[2]).sqrt();
            if !n.is_finite() || n < 1e-9 {
                return None;
            }
            let half = 1.5 / cam.scale.max(1e-9);
            Some(PlaneMark {
                origin: l.start,
                axes: (u, across.map(|v| v / n)),
                lo: (0.0, -half),
                hi: (long, half),
                step: 0.0,
                fill: c.to_array(),
                grid: [0; 4],
                edge: c.to_array(),
                edge_width: 0.0,
            })
        }
        Lit::Part(_) => None,
    }
}

/// How much bigger than what it boxes a Selection Box is, each way: a share of its longest side, and at least so many mm.
const BOX_GROWS: (f64, f64) = (0.05, 50.0);

impl BimApp {
    /// Selection Box (BX, FE-23a), as the project's: {3D} cut to a box round the forms and nested instances selected, a little
    /// bigger, and opened. With nothing selected, a box it keeps is taken away; with none, a note says what to select. The
    /// box is the view's for the session, as its other graphics are (`view_graphics.rs`): not saved, no step of the history.
    pub(crate) fn family_selection_box(&mut self) {
        let base = Arc::clone(&self.session.model);
        let Some(doc) = self.active_doc() else { return };
        let Some(ed) = self.active_family_mut() else { return };
        let chosen = !ed.selection.is_empty();
        ed.stage_3d(&base);
        let parts: Vec<Part> = ed.selected_3d().into_iter().filter_map(|l| if let Lit::Part(p) = l { Some(p) } else { None }).collect();
        // Nothing selected: the box goes. Something selected that isn't a form or a nested instance (a reference plane, a
        // dimension): the box, if there is one, stays as it was.
        let said = |text: &str| Status { text: format!("{text} {}", t("The box is unchanged.")), error: true };
        if parts.is_empty() {
            if chosen {
                self.status = said(t("Selection Box boxes forms and nested instances:"));
            } else if ed.views.kept.section_box.take().is_none() {
                let text = t("Selection Box: select the forms or nested instances to box first").to_string();
                self.status = Status { text, error: false };
            }
            return;
        }
        let Some(scene) = ed.scene3d.as_ref() else {
            let problem = ed.problem.clone().unwrap_or_default();
            self.status = said(&format!("{} {problem}", t("Selection Box: {3D} can't be drawn.")));
            return;
        };
        // From what the staged scene draws of each part selected: a void's cut, a nested instance, a revolve as drawn.
        let ids: std::collections::HashSet<Id> = scene.part.iter().filter(|(_, p)| parts.contains(p)).map(|(id, _)| *id).collect();
        let boxes = bimcraft_core::view3d::Scene::new(&scene.model).boxes(&ids);
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for (a, b) in boxes.values() {
            for k in 0..3 {
                (lo[k], hi[k]) = (lo[k].min(a[k]), hi[k].max(b[k]));
            }
        }
        if !(lo.iter().chain(&hi).all(|v| v.is_finite())) {
            self.status = said(t("Selection Box: what is selected draws nothing in {3D} to box."));
            return;
        }
        let side = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max);
        let grow = (side * BOX_GROWS.0).max(BOX_GROWS.1);
        ed.views.kept.section_box = Some((lo.map(|v| v - grow), hi.map(|v| v + grow)));
        self.open_sheet(crate::Sheet::Family(doc, None));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "fe16b000-0000-4000-8000-00000000000a";
    const B: &str = "fe16b000-0000-4000-8000-00000000000b";

    fn volume(tris: &[[[f64; 3]; 3]]) -> f64 {
        tris.iter()
            .map(|[a, b, c]| {
                let cross = [b[1] * c[2] - b[2] * c[1], b[2] * c[0] - b[0] * c[2], b[0] * c[1] - b[1] * c[0]];
                (a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2]) / 6.0
            })
            .sum()
    }

    /// Solid A (100 to 500 square, 300 high) and solid B (`x0` to `x0 + 400` east-west, 300 high) with `lists` on A.
    fn pair(x0: i32, lists: &str) -> Family {
        let sq = |x0: i32| format!("[[{x0}, 100], [{}, 100], [{}, 500], [{x0}, 500]]", x0 + 400, x0 + 400);
        serde_json::from_value(
            serde_json::from_str::<Value>(&format!(
                r#"{{"types": {{"T": {{}}}}, "forms": [{{"guid": "{A}", {lists} "profile": {}, "from": 0, "to": 300}}, {{"guid": "{B}", "profile": {}, "from": 0, "to": 300}}]}}"#,
                sq(100),
                sq(x0)
            ))
            .unwrap(),
        )
        .unwrap()
    }

    /// What {3D} builds for form 0: its part's volume, and the family's own piece for it.
    fn volumes(f: &Family) -> (f64, f64) {
        let values = BTreeMap::new();
        let mut part = alone(f, Part::Form(0));
        rekey(&mut part, 1);
        let whole = f.pieces(&values).unwrap();
        let own = part.pieces(&values).unwrap();
        assert_eq!(own.len(), 1, "the part is the form alone");
        (volume(&own[0].tris), volume(&whole[0].tris))
    }

    #[test]
    fn a_part_cut_or_joined_by_a_solid_that_misses_it_is_the_whole_form_and_one_it_overlaps_loses_the_overlap() {
        let whole = 400.0 * 400.0 * 300.0;
        for lists in [format!(r#""cut_by": ["{B}"],"#), format!(r#""joined_by": ["{B}"],"#)] {
            // Far apart: unchanged, in the part as in the family.
            let (part, family) = volumes(&pair(-1000, &lists));
            assert!((part - whole).abs() < 1.0 && (family - whole).abs() < 1.0, "{lists}: {part} {family}");
            // Overlapping by 200 mm: exactly the overlap (200 x 400 x 300) comes out.
            let (part, family) = volumes(&pair(300, &lists));
            let left = whole - 200.0 * 400.0 * 300.0;
            assert!((part - left).abs() < 1.0 && (family - left).abs() < 1.0, "{lists}: {part} {family}");
        }
        // Nothing listed: nothing cuts, though B overlaps it.
        let (part, family) = volumes(&pair(300, ""));
        assert!((part - whole).abs() < 1.0 && (family - whole).abs() < 1.0);
    }

    /// The volume of the piece form `k` makes: in its part ({3D}), and in the family itself, which must agree.
    fn agree(f: &Family, k: usize) -> (f64, f64) {
        let values = BTreeMap::new();
        let mut part = alone(f, Part::Form(k));
        rekey(&mut part, 1);
        let own = part.pieces(&values).unwrap();
        assert_eq!(own.len(), 1, "the part is the form alone");
        let before = f.forms[..k].iter().filter(|x| !x.void).count();
        (volume(&own[0].tris), volume(&f.pieces(&values).unwrap()[before].tris))
    }

    fn family_of(forms: &str) -> Family {
        serde_json::from_str(&format!(r#"{{"types": {{"T": {{}}}}, "forms": [{forms}]}}"#)).unwrap()
    }

    const V: &str = "fe16b000-0000-4000-8000-0000000000c0";

    fn solid(guid: &str, x0: i32, extra: &str) -> String {
        format!(
            r#"{{"guid": "{guid}", {extra} "profile": [[{x0}, 100], [{}, 100], [{}, 500], [{x0}, 500]], "from": 0, "to": 300}}"#,
            x0 + 400,
            x0 + 400
        )
    }

    fn void(x0: i32, x1: i32) -> String {
        format!(r#"{{"guid": "{V}", "void": true, "profile": [[{x0}, 100], [{x1}, 100], [{x1}, 500], [{x0}, 500]], "from": 0, "to": 200}}"#)
    }

    #[test]
    fn a_solid_cutter_listed_before_its_target_a_void_with_it_and_an_uncut_by_all_agree_with_the_family() {
        // The cutter before the target (cut_by naming a solid listed earlier).
        let f = family_of(&format!("{}, {}", solid(B, 300, ""), solid(A, 100, &format!(r#""cut_by": ["{B}"],"#))));
        let (part, family) = agree(&f, 1);
        assert!((part - family).abs() < 1.0 && (part - (160000.0 * 300.0 - 200.0 * 400.0 * 300.0)).abs() < 1.0, "{part} {family}");
        // A void and a solid together: the void listed after, the solid by cut_by.
        let f = family_of(&format!("{}, {}, {}", solid(A, 100, &format!(r#""cut_by": ["{B}"],"#)), solid(B, 300, ""), void(150, 250)));
        let (part, family) = agree(&f, 0);
        assert!((part - family).abs() < 1.0 && part < 160000.0 * 300.0 - 200.0 * 400.0 * 300.0 - 1.0, "{part} {family}");
        // uncut_by the void, still cut by the solid.
        let f = family_of(&format!(
            "{}, {}, {}",
            solid(A, 100, &format!(r#""cut_by": ["{B}"], "uncut_by": ["{V}"],"#)),
            solid(B, 300, ""),
            void(150, 250)
        ));
        let (part, family) = agree(&f, 0);
        assert!((part - family).abs() < 1.0 && (part - (160000.0 * 300.0 - 200.0 * 400.0 * 300.0)).abs() < 1.0, "{part} {family}");
    }

    #[test]
    fn a_dimension_naming_a_form_and_a_plane_survives_the_part_with_the_same_volume() {
        let f: Family = serde_json::from_str(
            r#"{"types": {"T": {}}, "planes": [{"guid": "fe16b111-0000-4000-8000-000000000001", "name": "Top", "start": [-2000, 0, 300], "end": [2000, 0, 300], "up": [0, 1, 0]}], "forms": [{"guid": "fe16b111-0000-4000-8000-00000000000a", "profile": [[0, 0], [400, 0], [400, 400], [0, 400]], "from": 0, "to": 250}], "dimensions": [{"refs": [{"plane": "fe16b111-0000-4000-8000-000000000001"}, {"form": "fe16b111-0000-4000-8000-00000000000a", "end": "end"}], "view": "front", "at": [600, 0, 300], "locked": [{"segment": 0, "length": 0}], "alignment": true}]}"#,
        )
        .unwrap();
        let mut part = alone(&f, Part::Form(0));
        rekey(&mut part, 3);
        // Sound in its own right, as a project's check holds every family to.
        assert_eq!(part.check(&|_| true), Ok(()));
        assert_eq!(f.check(&|_| true), Ok(()));
        let values = BTreeMap::new();
        let (a, b) = (f.flexed(&values).unwrap().pieces(&values).unwrap(), part.flexed(&values).unwrap().pieces(&values).unwrap());
        assert!((volume(&a[0].tris) - volume(&b[0].tris)).abs() < 1.0, "{} {}", volume(&a[0].tris), volume(&b[0].tris));
        // The dimension names the part's own form and plane, as it did the family's.
        assert_ne!(part.forms[0].guid, f.forms[0].guid);
        assert_eq!(part.dimensions.len(), 1);
    }
}
