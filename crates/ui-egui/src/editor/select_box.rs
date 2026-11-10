//! FE-1: what is selected in the document of a family that isn't adaptive, kept as one set, and the selection box its plans
//! and elevations draw, as Revit's family editor has them.
//!
//! The set (`Selection`) is the only place the selection is kept: forms, reference planes (a datum too, as a click takes
//! one), reference lines, dimensions, plan symbols, connectors and nested instances, each once, in the order picked. The
//! accessors the rest of the editor reads, one for each kind (`selected_form`, `selected_plane`, `selected_line`,
//! `selected_dimension`, `selected_symbol`, `selected_connector`, `selected_nested`), say what the set holds while it holds
//! exactly one thing, of whatever kind, and nothing while it holds several: so Properties, the contextual tab, Align, Pivot,
//! the Start and End handles and the grips act on one element as they always have, never on one element out of several.
//! With several selected, Properties say how many (Multi-Select (N)), the contextual tab is Modify | Multi-Select, Delete
//! takes them all away in one step of the document's history (none of them, saying why, when one can't go), and Escape and
//! Modify let them all go; the Delete key, the ribbon's buttons and the document's two-letter keys (DE and MD, FE-13's
//! `BimApp::family_action` in `keys.rs`) all come to `delete_selected` and `modify_tool`. Each is drawn in the selection's
//! colour, in the views and in {3D}, as one selected alone is.
//!
//! The setters the rest of the editor calls (`select_form_at`, `select_symbol`, `select_connector`, `select_nested`, …)
//! make the set what they select, as a click does. Something taken away by its place in a list (a form, a symbol, a
//! connector, a nested instance) goes from the set and the places after it move up one (`Selection::taken_out`); what is
//! gone from the draft otherwise (undone, redone, or changed while another tab was in front) is dropped from the set
//! (`FamilyEditor::prune_selection`, after each undo and redo, on a tab coming to the front and at the end of every frame).
//!
//! In a plan or an elevation, a drag of the primary button that starts on nothing a click would select (nor a handle, a
//! grip or a padlock), with no tool in hand, draws a box: left to right a window, solid, taking what lies wholly inside
//! it; right to left a crossing, dashed, taking what it touches too, by the one rule the adaptive family's modelling view
//! boxes by (`geom::SelectBox`), drawn as the project's plans draw theirs (`canvas::box_shapes`). Ctrl adds what it takes,
//! Shift takes that away, else it is the selection; a click on an element with Ctrl or Shift does the same with it. Each
//! element is boxed by what it is drawn with on screen: a form's or a nested instance's outline, a plane's trace, a
//! reference line, a dimension's line, a symbol's strokes (in the plans, where symbols are drawn), a connector's place. So
//! a crossing box wholly inside a form's face, meeting none of its edges, doesn't take it, as Revit's doesn't. A drag from
//! an element that isn't selected moves nothing: dragging an element to move it is FE-7's. {3D} has no box: a drag there
//! turns the view.
use super::*;
use bimcraft_core::family::constraints::DimMark;
use bimcraft_core::family::views::{self as fv, Staged, Which};
use bimcraft_core::family::{FamilyDimension, Val};
use bimcraft_core::geom::{P2, SelectBox};
use bimcraft_core::ids::Guid;
use bimcraft_core::render::{Drawing, View};
use egui::{Modifiers, Painter, Pos2, Response, Shape};

/// One thing selected in a family document that isn't adaptive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Picked {
    /// A form, by its place in the family's forms.
    Form(usize),
    /// A reference plane, a datum or the family's own.
    Plane(Guid),
    /// A reference line.
    Line(Guid),
    Dimension(Guid),
    /// A plan symbol, by its place in the family's symbols.
    Symbol(usize),
    /// A connector, by its place in the family's connectors.
    Connector(usize),
    /// A nested instance, by its place in the family's nested instances.
    Nested(usize),
}

/// What a click or a box does with what it takes, by the keys held, as Revit's: Ctrl adds it, Shift takes it away, else it
/// is the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum How {
    Replace,
    Add,
    Remove,
}

impl How {
    pub(crate) fn of(m: Modifiers) -> How {
        if m.ctrl || m.command {
            How::Add
        } else if m.shift {
            How::Remove
        } else {
            How::Replace
        }
    }
}

/// What is selected in a family document that isn't adaptive, and the box being dragged to select more.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Selection {
    items: Vec<Picked>,
    /// A box being dragged in a plan or an elevation: where it began, on screen.
    boxing: Option<Pos2>,
}

impl Selection {
    /// What is selected, in the order it was picked.
    pub(crate) fn items(&self) -> &[Picked] {
        &self.items
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub(crate) fn contains(&self, p: Picked) -> bool {
        self.items.contains(&p)
    }

    /// The one thing selected, while there is exactly one.
    pub(crate) fn only(&self) -> Option<Picked> {
        match self.items.as_slice() {
            [p] => Some(*p),
            _ => None,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.items.clear();
    }

    /// `p` let go, if it was selected.
    pub(crate) fn forget(&mut self, p: Picked) {
        self.items.retain(|q| *q != p);
    }

    pub(crate) fn retain(&mut self, keep: impl FnMut(&Picked) -> bool) {
        self.items.retain(keep);
    }

    /// What a click or a box took, `taken`, made the selection, added to it or taken away from it, as `how` says.
    pub(crate) fn apply(&mut self, taken: &[Picked], how: How) {
        if how == How::Replace {
            self.items.clear();
        }
        for p in taken {
            match how {
                How::Remove => self.forget(*p),
                How::Replace | How::Add => {
                    if !self.items.contains(p) {
                        self.items.push(*p);
                    }
                }
            }
        }
    }

    /// `gone`, held by its place in a list (a form, a symbol, a connector, a nested instance), taken out of that list: it
    /// goes from the selection, and what is selected of the same list after it moves up one place.
    pub(crate) fn taken_out(&mut self, gone: Picked) {
        self.forget(gone);
        for p in &mut self.items {
            *p = match (*p, gone) {
                (Picked::Form(j), Picked::Form(i)) if j > i => Picked::Form(j - 1),
                (Picked::Symbol(j), Picked::Symbol(i)) if j > i => Picked::Symbol(j - 1),
                (Picked::Connector(j), Picked::Connector(i)) if j > i => Picked::Connector(j - 1),
                (Picked::Nested(j), Picked::Nested(i)) if j > i => Picked::Nested(j - 1),
                (same, _) => same,
            };
        }
    }

    /// Forms `i` and `j` swapped in their list (Move Up, Move Down): each selected stays so where it went.
    pub(crate) fn forms_swapped(&mut self, i: usize, j: usize) {
        for p in &mut self.items {
            *p = match *p {
                Picked::Form(k) if k == i => Picked::Form(j),
                Picked::Form(k) if k == j => Picked::Form(i),
                same => same,
            };
        }
    }
}

impl FamilyEditor {
    /// `p` alone selected, whatever was selected let go: as a click or the Project Browser selects one.
    pub(crate) fn select_only(&mut self, p: Picked) {
        self.selection.apply(&[p], How::Replace);
    }

    /// The one thing selected, while exactly one is and the draft still has it: what the accessors of each kind read.
    pub(crate) fn only_selected(&self) -> Option<Picked> {
        self.selection.only().filter(|p| self.holds(*p))
    }

    /// Whether the draft has `p`.
    pub(crate) fn holds(&self, p: Picked) -> bool {
        match p {
            Picked::Form(i) => i < self.forms.len(),
            Picked::Plane(g) => self.planes.iter().any(|q| q.guid == g),
            Picked::Line(g) => self.lines.iter().any(|l| l.guid == g),
            Picked::Dimension(g) => self.dimensions.iter().any(|d| d.guid == g),
            Picked::Symbol(i) => i < self.symbols.len(),
            Picked::Connector(i) => i < self.connectors.len(),
            Picked::Nested(i) => i < self.nested_instances.len(),
        }
    }

    /// The forms selected, by place, while the draft has them.
    pub(crate) fn picked_forms(&self) -> impl Iterator<Item = usize> + '_ {
        self.selection.items().iter().filter_map(|p| match p {
            Picked::Form(i) if *i < self.forms.len() => Some(*i),
            _ => None,
        })
    }

    /// The planes selected, while the draft has them.
    pub(crate) fn picked_planes(&self) -> impl Iterator<Item = Guid> + '_ {
        self.selection.items().iter().filter_map(|p| match p {
            Picked::Plane(g) if self.planes.iter().any(|q| q.guid == *g) => Some(*g),
            _ => None,
        })
    }

    /// What is gone from the draft let go: after an undo or a redo, on its tab coming to the front, after each frame.
    pub(crate) fn prune_selection(&mut self) {
        let mut items = std::mem::take(&mut self.selection.items);
        items.retain(|p| self.holds(*p));
        self.selection.items = items;
    }

    /// A click on `hit` in a plan or an elevation (none: on nothing), with the keys `mods` held: Ctrl adds it, Shift takes
    /// it away, else it alone is selected; a click on nothing lets the selection go, unless Ctrl or Shift is held.
    pub(crate) fn click_select(&mut self, hit: Option<Picked>, mods: Modifiers) {
        match (hit, How::of(mods)) {
            (Some(p), how) => self.selection.apply(&[p], how),
            (None, How::Replace) => self.let_everything_go(),
            (None, _) => {}
        }
    }

    /// Whether Delete takes something of the selection away: anything but a datum plane.
    pub(super) fn deletable_picked(&self) -> bool {
        self.selection.items().iter().any(|p| match p {
            Picked::Plane(g) => self.planes.iter().any(|q| q.guid == *g && !q.datum()),
            other => self.holds(*other),
        })
    }

    /// Delete (DE, the Delete key, the ribbon's Delete) on the selection: everything in it taken away at once, so one step of
    /// the document's history, each with what depends on it, a datum left as it is. What depends on another goes first (a
    /// dimension, a nested instance, a form, a symbol, a connector, then a reference line, then a plane), and each list from
    /// its end, so the places of the rest hold. Where one can't go (a plane something not selected is drawn on), nothing does,
    /// and the error says why.
    pub(super) fn delete_picked(&mut self) -> Result<(), String> {
        let kept = (
            self.forms.clone(),
            self.planes.clone(),
            self.lines.clone(),
            self.dimensions.clone(),
            self.symbols.clone(),
            self.connectors.clone(),
            self.nested_instances.clone(),
            self.selection.clone(),
        );
        let items: Vec<Picked> = self.selection.items().iter().copied().filter(|p| self.holds(*p)).collect();
        let places = |of: fn(Picked) -> Option<usize>| {
            let mut v: Vec<usize> = items.iter().filter_map(|p| of(*p)).collect();
            v.sort_unstable_by(|a, b| b.cmp(a));
            v
        };
        let guids = |of: fn(Picked) -> Option<Guid>| items.iter().filter_map(|p| of(*p)).collect::<Vec<Guid>>();
        let done = (|| -> Result<(), String> {
            for g in guids(|p| if let Picked::Dimension(g) = p { Some(g) } else { None }) {
                self.delete_dimension(g);
            }
            for i in places(|p| if let Picked::Nested(i) = p { Some(i) } else { None }) {
                self.remove_nested(i);
            }
            for i in places(|p| if let Picked::Form(i) = p { Some(i) } else { None }) {
                self.remove_form(i);
            }
            for i in places(|p| if let Picked::Symbol(i) = p { Some(i) } else { None }) {
                self.remove_symbol(i);
            }
            for i in places(|p| if let Picked::Connector(i) = p { Some(i) } else { None }) {
                self.remove_connector(i);
            }
            for g in guids(|p| if let Picked::Line(g) = p { Some(g) } else { None }) {
                self.delete_line(g)?;
            }
            for g in guids(|p| if let Picked::Plane(g) = p { Some(g) } else { None }) {
                if self.planes.iter().any(|q| q.guid == g && !q.datum()) {
                    self.delete_plane(g)?;
                }
            }
            Ok(())
        })();
        if let Err(e) = done {
            (self.forms, self.planes, self.lines, self.dimensions, self.symbols, self.connectors, self.nested_instances, self.selection) =
                kept;
            return Err(e);
        }
        self.prune_selection();
        Ok(())
    }

    /// Nested instance `i` taken away.
    pub(crate) fn remove_nested(&mut self, i: usize) {
        if i < self.nested_instances.len() {
            self.nested_instances.remove(i);
            self.selection.taken_out(Picked::Nested(i));
        }
    }
}

/// What a plan or an elevation shows of the family, for the box and the selection's outlines: the family as the type previewed
/// flexes it and its values, the scratch model and drawing it is drawn from, where the drawing is on screen, and the
/// dimensions' and connectors' marks the view draws over it.
pub(super) struct Scene<'a> {
    pub(super) family: &'a Family,
    pub(super) values: &'a BTreeMap<String, Val>,
    pub(super) staged: &'a Staged,
    pub(super) drawing: &'a Drawing,
    pub(super) which: Which,
    pub(super) view: View,
    /// The drawing's top left corner on screen.
    pub(super) min: Pos2,
    pub(super) dims: &'a [DimMark],
    pub(super) dimensions: &'a [FamilyDimension],
    pub(super) connectors: &'a [super::connector_marks::Mark],
    /// The document's Detail Level, which says which symbols its plans draw.
    pub(super) detail: bimcraft_core::graphics::Detail,
    /// The forms Temporary Hide/Isolate leaves out of the view, by place.
    pub(super) hidden: &'a [bool],
}

impl Scene<'_> {
    fn screen(&self, q: P2) -> Pos2 {
        let (x, y) = self.view.px(q);
        self.min + vec2(x, y)
    }

    /// Point `p` of the family's axes on screen.
    fn at(&self, p: [f64; 3]) -> Pos2 {
        self.screen(self.staged.to_view(self.which, p))
    }

    /// The outline form or nested instance `p` is seen with, on screen; none for anything else.
    pub(super) fn outline(&self, p: Picked) -> Vec<Pos2> {
        let ring = match p {
            Picked::Form(i) => fv::outline(self.family, self.values, self.staged, self.which, i),
            Picked::Nested(i) => fv::nested_outline(self.family, self.values, self.staged, self.which, i),
            _ => Vec::new(),
        };
        ring.into_iter().map(|q| self.screen(q)).collect()
    }

    /// What `p` is drawn with on screen, as segments (a point, one of no length): what a box takes it by, and what lights it.
    pub(super) fn pieces(&self, p: Picked) -> Vec<[Pos2; 2]> {
        let closed = |ring: Vec<Pos2>| -> Vec<[Pos2; 2]> {
            if ring.len() < 2 {
                return ring.iter().map(|q| [*q, *q]).collect();
            }
            (0..ring.len()).map(|k| [ring[k], ring[(k + 1) % ring.len()]]).collect()
        };
        let open = |pts: &[P2]| -> Vec<[Pos2; 2]> {
            let on: Vec<Pos2> = pts.iter().map(|q| self.at([q.0, q.1, 0.0])).collect();
            match on.as_slice() {
                [one] => vec![[*one, *one]],
                _ => on.windows(2).map(|w| [w[0], w[1]]).collect(),
            }
        };
        match p {
            Picked::Form(_) | Picked::Nested(_) => closed(self.outline(p)),
            Picked::Plane(g) => {
                let strokes = super::views::plane_strokes(self.drawing, self.staged, g, self.min);
                if !strokes.is_empty() {
                    return strokes;
                }
                let trace = self
                    .family
                    .planes
                    .iter()
                    .find(|q| q.guid == g)
                    .and_then(|q| super::views::plane_trace(self.staged, self.which, &q.frame()));
                trace.map(|(a, b)| vec![[self.screen(a), self.screen(b)]]).unwrap_or_default()
            }
            Picked::Line(g) => {
                let Some(l) = self.family.lines.iter().find(|l| l.guid == g) else { return Vec::new() };
                vec![[self.at(l.start), self.at(l.end)]]
            }
            Picked::Dimension(g) => {
                let Some(m) = self.dims.iter().find(|m| !m.alignment && self.dimensions.get(m.index).is_some_and(|d| d.guid == g)) else {
                    return Vec::new();
                };
                let pts: Vec<Pos2> = m.points.iter().map(|q| self.screen(*q)).collect();
                match (m.kind, pts.split_first()) {
                    // An angle's: from where its lines cross out along each.
                    (bimcraft_core::family::DimKind::Angular, Some((v, rays))) => rays.iter().map(|r| [*v, *r]).collect(),
                    _ => pts.windows(2).map(|w| [w[0], w[1]]).collect(),
                }
            }
            Picked::Symbol(i) => {
                // Symbols are annotation of the plans: the elevations draw none.
                if !matches!(self.which, Which::Plan | Which::Ceiling) {
                    return Vec::new();
                }
                use bimcraft_core::family::Mark;
                match self.family.symbol_mark(i, self.values, Some(self.detail)) {
                    Ok(Some(Mark::Line(pts))) => open(&pts),
                    Ok(Some(Mark::Dot { centre, radius })) => {
                        let ring: Vec<P2> = (0..=12)
                            .map(|k| {
                                let (s, c) = (std::f64::consts::TAU * f64::from(k) / 12.0).sin_cos();
                                P2(centre.0 + radius * c, centre.1 + radius * s)
                            })
                            .collect();
                        open(&ring)
                    }
                    Ok(Some(Mark::Label { at, height, text, align })) => {
                        bimcraft_core::family::lettering(&text, at, height, align).iter().flat_map(|stroke| open(stroke)).collect()
                    }
                    _ => Vec::new(),
                }
            }
            Picked::Connector(i) => {
                let Some(m) = self.connectors.iter().find(|m| m.index == i) else { return Vec::new() };
                let c = self.at(m.at);
                vec![[c, c]]
            }
        }
    }

    /// Everything the view shows that a box may take: what a click there could select, and the symbols the plans draw.
    fn candidates(&self) -> Vec<Picked> {
        let mut out: Vec<Picked> = (0..self.family.forms.len())
            .filter(|i| !self.family.forms[*i].void && !self.hidden.get(*i).copied().unwrap_or(false))
            .map(Picked::Form)
            .collect();
        out.extend((0..self.family.nested_instances.len()).map(Picked::Nested));
        let mut planes: Vec<Guid> = Vec::new();
        for g in self.drawing.owners.iter().filter_map(|o| o.and_then(|o| self.staged.plane_of(o))) {
            if !planes.contains(&g) {
                planes.push(g);
            }
        }
        out.extend(planes.into_iter().map(Picked::Plane));
        out.extend(self.family.lines.iter().map(|l| Picked::Line(l.guid)));
        out.extend(
            self.dims.iter().filter(|m| !m.alignment).filter_map(|m| self.dimensions.get(m.index)).map(|d| Picked::Dimension(d.guid)),
        );
        out.extend((0..self.family.symbols.len()).map(Picked::Symbol));
        out.extend(self.connectors.iter().map(|m| Picked::Connector(m.index)));
        out
    }

    /// What a box from `from` to `to` on screen takes: left to right what lies wholly inside it, right to left what it
    /// touches too.
    pub(super) fn in_box(&self, from: Pos2, to: Pos2) -> Vec<Picked> {
        let mm = |p: Pos2| P2(f64::from(p.x), f64::from(p.y));
        let boxed = SelectBox::dragged(mm(from), mm(to));
        self.candidates()
            .into_iter()
            .filter(|p| {
                let pieces: Vec<(P2, P2)> = self.pieces(*p).into_iter().map(|[a, b]| (mm(a), mm(b))).collect();
                boxed.takes(&pieces)
            })
            .collect()
    }

    /// Whether a click at `p` would select something: a connector, a nested instance, a reference line, a plane or a form.
    pub(super) fn under(&self, p: Pos2) -> bool {
        let q = self.view.mm((p.x - self.min.x, p.y - self.min.y));
        let reach = 6.0 / self.view.scale.max(1e-9);
        let project = |x: [f64; 3]| Some(self.at(x));
        super::connector_marks::pick(self.connectors, &project, p).is_some()
            || fv::nested_at(self.family, self.values, self.staged, self.which, q).is_some()
            || self.staged.line_at(self.which, q, reach).is_some()
            || self.drawing.pick_all((p.x - self.min.x, p.y - self.min.y), 6.0).into_iter().any(|id| self.staged.plane_of(id).is_some())
            || fv::form_at(self.family, self.values, self.staged, self.which, q).is_some()
    }

    /// What is selected, over the picture, as one selected alone is drawn: a form's and a nested instance's outline, a plane's
    /// trace, a reference line, a symbol's strokes, in the selection's colour. A dimension and a connector are lit by their own
    /// painters.
    pub(super) fn paint_selected(&self, painter: &Painter, selected: &[Picked]) {
        let picked = super::views::PICKED;
        for p in selected {
            match p {
                Picked::Form(_) | Picked::Nested(_) => {
                    let ring = self.outline(*p);
                    if ring.len() > 2 {
                        painter.add(Shape::closed_line(ring, Stroke::new(2.0, picked)));
                    }
                }
                Picked::Plane(_) | Picked::Line(_) => {
                    for s in self.pieces(*p) {
                        painter.line_segment(s, Stroke::new(2.5, picked));
                    }
                }
                Picked::Symbol(_) => {
                    for s in self.pieces(*p) {
                        painter.line_segment(s, Stroke::new(2.0, picked));
                    }
                }
                Picked::Dimension(_) | Picked::Connector(_) => {}
            }
        }
    }
}

/// The selection box of a plan or an elevation: begun at `start` (a drag from nothing), drawn while the drag lasts over
/// `pane`, solid left to right and dashed right to left as the project's plans draw theirs; where it began and where it was
/// let go, once it is.
pub(super) fn track(
    sel: &mut Selection,
    ui: &Ui,
    response: &Response,
    painter: &Painter,
    pane: Rect,
    start: Option<Pos2>,
) -> Option<(Pos2, Pos2)> {
    if start.is_some() {
        sel.boxing = start;
    }
    let from = sel.boxing?;
    let now = ui.input(|i| i.pointer.latest_pos()).unwrap_or(from);
    if response.drag_stopped() {
        sel.boxing = None;
        return Some((from, now));
    }
    if !response.dragged_by(PointerButton::Primary) {
        sel.boxing = None;
        return None;
    }
    let local = |p: Pos2| (p.x - pane.min.x, p.y - pane.min.y);
    painter.extend(crate::canvas::box_shapes(pane, local(from), local(now)));
    None
}
