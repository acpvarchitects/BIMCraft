//! FE-14: the Clipboard panel in a family that isn't adaptive, as Revit's family editor has it. Copy to Clipboard (Ctrl+C)
//! and Cut to Clipboard (Ctrl+X) put what is selected on the window's clipboard (`BimApp::clipboard`, [`Held`]), Cut taking it
//! away as one step of the history (refused where Delete refuses it); Paste (Ctrl+V) has the copies follow the pointer in the
//! plan or elevation shown, a click putting them there as one step (Escape gives it up, nothing changed); Paste Aligned puts
//! them back Aligned to Same Place, or, for dimensions copied alone, Aligned to Current View or to Selected Views (a dialog of
//! the views that can draw them). Aligned to Selected Levels and Aligned to Picked Level are greyed: a family has one level.
//! While a form is sketched the same keys copy, cut and paste the lines picked, into the same sketch (its contextual tab has
//! no room for the panel).
//!
//! What is copied, kept and refused is the core's (`bimcraft_core::family::clipboard`): FE-4's copies, new GUIDs at each paste,
//! the constraints among them kept and those to what was left behind dropped (the status bar says how many); a paste that
//! follows the pointer is FE-7's move of the copies (`move_picked`), so a number it would change that is a formula refuses it.
//! The clipboard is the window's, so it outlives views, ribbon tabs and document tabs; pasting into another family's document
//! is refused for now (the copies name the first family's planes, parameters and nested families).
//!
//! A command that can't be used now is greyed with why in its tooltip, as FE-8's tools are, and its key says why. The panel
//! takes a list of states ([`panel`]), so a project's clipboard can draw the same one.
use super::drag_move::{Kept, Names, Origin, copy_said};
use super::select_box::{How, Picked};
use super::tabs::DocId;
use super::*;
use crate::icons::Icon;
use bimcraft_core::family::clipboard::{Clip, PasteRefused, SketchClip};
use bimcraft_core::family::constraints::{Copying, DimView};
use bimcraft_core::family::views::Which;
use bimcraft_core::geom::P2;

/// What the clipboard holds: a family document's elements, or lines of one of its sketches; the document they came from and
/// its family's name.
#[derive(Clone, Debug)]
pub(crate) enum Held {
    Elements { doc: DocId, family: String, clip: Box<Clip> },
    Lines { doc: DocId, family: String, clip: SketchClip },
}

impl Held {
    fn doc(&self) -> DocId {
        match self {
            Held::Elements { doc, .. } | Held::Lines { doc, .. } => *doc,
        }
    }

    fn family(&self) -> &str {
        match self {
            Held::Elements { family, .. } | Held::Lines { family, .. } => family,
        }
    }
}

/// How Paste puts the copies back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Paste {
    /// Following the pointer, where a click puts them.
    Pointer,
    SamePlace,
    CurrentView,
    SelectedViews,
    SelectedLevels,
    PickedLevel,
}

/// A command of the Clipboard panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Copy,
    Cut,
    Paste(Paste),
}

/// Paste Aligned's choices, in the order Revit's drop-down lists them.
pub(crate) const ALIGNED: [Paste; 5] =
    [Paste::SelectedLevels, Paste::SelectedViews, Paste::CurrentView, Paste::SamePlace, Paste::PickedLevel];

impl Command {
    /// Every command, the panel's order.
    pub(crate) const ALL: [Command; 8] = [
        Command::Paste(Paste::Pointer),
        Command::Cut,
        Command::Copy,
        Command::Paste(Paste::SelectedLevels),
        Command::Paste(Paste::SelectedViews),
        Command::Paste(Paste::CurrentView),
        Command::Paste(Paste::SamePlace),
        Command::Paste(Paste::PickedLevel),
    ];

    /// Its name: on the ribbon and in `shortcuts::ACTIONS`.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Command::Copy => msg!("Copy to Clipboard"),
            Command::Cut => msg!("Cut to Clipboard"),
            Command::Paste(Paste::Pointer) => msg!("Paste"),
            Command::Paste(Paste::SamePlace) => msg!("Aligned to Same Place"),
            Command::Paste(Paste::CurrentView) => msg!("Aligned to Current View"),
            Command::Paste(Paste::SelectedViews) => msg!("Aligned to Selected Views"),
            Command::Paste(Paste::SelectedLevels) => msg!("Aligned to Selected Levels"),
            Command::Paste(Paste::PickedLevel) => msg!("Aligned to Picked Level"),
        }
    }

    /// The command an action of `shortcuts::ACTIONS` names.
    pub(crate) fn of(action: &str) -> Option<Command> {
        Command::ALL.into_iter().find(|c| c.label() == action)
    }

    fn tip(self) -> &'static str {
        match self {
            Command::Copy => msg!("Copy to Clipboard (Ctrl+C): what is selected, or the lines picked in a sketch, for Paste"),
            Command::Cut => msg!("Cut to Clipboard (Ctrl+X): what is selected taken away, for Paste"),
            Command::Paste(Paste::Pointer) => msg!("Paste (Ctrl+V): the copies follow the pointer; click where they go, Esc cancels"),
            Command::Paste(Paste::SamePlace) => msg!("Aligned to Same Place: the copies where what was copied is"),
            Command::Paste(Paste::CurrentView) => msg!("Aligned to Current View: dimensions copied alone, drawn in the view shown"),
            Command::Paste(Paste::SelectedViews) => msg!("Aligned to Selected Views: dimensions copied alone, drawn in the views picked"),
            Command::Paste(Paste::SelectedLevels) => msg!("Aligned to Selected Levels: the copies on each level picked"),
            Command::Paste(Paste::PickedLevel) => msg!("Aligned to Picked Level: the copies on a level clicked in an elevation"),
        }
    }

    fn icon(self) -> Icon {
        match self {
            Command::Copy => Icon::Copy,
            Command::Cut => Icon::Cut,
            Command::Paste(_) => Icon::Paste,
        }
    }
}

/// The Clipboard panel: Paste, Paste Aligned's drop-down, Cut and Copy, each greyed with why it can't be used now
/// (`states`, each command's reason, if any). The command clicked.
pub(crate) fn panel(ui: &mut Ui, states: &[(Command, Option<String>)]) -> Option<Command> {
    let why = |c: Command| states.iter().find(|(k, _)| *k == c).and_then(|(_, w)| w.clone());
    let clicked = std::cell::Cell::new(None);
    let button = |ui: &mut Ui, c: Command, big: bool| {
        let blocked = why(c);
        let r = ui
            .add_enabled_ui(blocked.is_none(), |ui| {
                if big {
                    crate::big_button(ui, c.icon(), c.label(), false, c.tip())
                } else {
                    crate::stacked_button(ui, c.icon(), c.label(), false, c.tip())
                }
            })
            .inner;
        let r = match blocked {
            Some(w) => r.on_disabled_hover_text(w),
            None => r,
        };
        if r.clicked() {
            clicked.set(Some(c));
        }
    };
    let aligned = |ui: &mut Ui| {
        let tip = msg!("Paste Aligned: the copies where what was copied is, or its dimensions in other views");
        let r = crate::stacked_button(ui, Icon::Paste, msg!("Paste Aligned"), false, tip);
        crate::ribbon::choices(ui, &r, false, |ui| {
            for p in ALIGNED {
                let c = Command::Paste(p);
                let blocked = why(c);
                let r = ui.add_enabled(blocked.is_none(), egui::Button::new(t(c.label())));
                let r = match blocked {
                    Some(w) => r.on_disabled_hover_text(w),
                    None => r.on_hover_text(t(c.tip())),
                };
                if r.clicked() {
                    clicked.set(Some(c));
                    ui.close();
                }
            }
        });
    };
    button(ui, Command::Paste(Paste::Pointer), true);
    crate::stacked(ui, |ui| {
        button(ui, Command::Cut, false);
        button(ui, Command::Copy, false);
        aligned(ui);
    });
    clicked.get()
}

/// A paste following the pointer in a plan or an elevation: what it pastes, and where the copies' middle is in the view it was
/// last drawn in (what the pointer holds).
#[derive(Clone, Debug)]
pub(crate) struct Pasting {
    pub(crate) clip: Clip,
    pub(crate) anchor: Option<(Which, P2)>,
}

/// Aligned to Selected Views' dialog: what it pastes, and each view that can draw it with whether it is picked.
#[derive(Clone, Debug)]
pub(crate) struct PasteViews {
    pub(crate) clip: Clip,
    pub(crate) views: Vec<(Which, bool)>,
}

/// What a refusal of a paste says.
fn paste_said(e: &PasteRefused) -> String {
    match e {
        PasteRefused::Host(_) => {
            t("A copy is drawn on a reference plane or a reference line the family no longer has: it can't be pasted.").to_string()
        }
        PasteRefused::Gone => t("The dimensions measure what the family no longer has: they can't be pasted.").to_string(),
        PasteRefused::Kind => {
            t("Dimensions copied alone are pasted into views: use Aligned to Current View or Aligned to Selected Views.").to_string()
        }
    }
}

/// What the status bar says of the constraints left behind.
fn dropped_said(n: usize) -> Option<String> {
    (n > 0).then(|| tf("{} constraint(s) to what wasn't copied were dropped.", &[&n.to_string()]))
}

/// The labels the family's reporting parameters give: what holds nothing.
fn reporting(f: &Family) -> impl Fn(&str) -> bool + '_ {
    |l: &str| f.parameters.iter().any(|p| p.name == l && p.reporting)
}

impl FamilyEditor {
    /// Why `c` can't be used now, if it can't: what its greyed button's tooltip and its key say. `held`, the clipboard;
    /// `doc`, this document.
    pub(crate) fn clipboard_blocked(&self, c: Command, held: Option<&Held>, doc: DocId) -> Option<String> {
        let said = |s: &str| Some(t(s).to_string());
        if self.adaptive.is_some() {
            return said(msg!("An adaptive family's clipboard isn't there yet."));
        }
        let paste = match c {
            Command::Copy | Command::Cut => {
                if let Some(s) = self.sketch.as_ref() {
                    if s.sketch.picked.is_empty() {
                        return said(msg!("Pick lines of the sketch first."));
                    }
                    return None;
                }
                if !self.selection.items().iter().any(|p| self.holds(*p)) {
                    return said(msg!("Select what to copy first."));
                }
                if c == Command::Cut && !self.deletable() {
                    return said(msg!("Nothing selected can be taken away."));
                }
                return None;
            }
            Command::Paste(p) => p,
        };
        if matches!(paste, Paste::SelectedLevels | Paste::PickedLevel) {
            return said(msg!("A family has one level, Ref. Level, which what is copied is on already: use Aligned to Same Place."));
        }
        let Some(held) = held else { return said(msg!("Nothing has been copied to the clipboard.")) };
        if held.doc() != doc {
            return Some(tf(
                "The clipboard holds what was copied in {}: paste it there. Pasting from one family into another isn't done yet.",
                &[held.family()],
            ));
        }
        match (held, self.sketch.as_ref()) {
            (Held::Lines { .. }, None) => said(msg!("The clipboard holds a sketch's lines: paste them into that sketch.")),
            (Held::Elements { .. }, Some(_)) => said(msg!("The clipboard holds elements: finish the sketch, then paste them.")),
            (Held::Lines { clip, .. }, Some(s)) => {
                if clip.form != s.guid || clip.part != s.part as usize {
                    return said(msg!("The clipboard holds lines of another sketch: paste them into the sketch they were copied from."));
                }
                match paste {
                    Paste::Pointer | Paste::SamePlace => None,
                    _ => said(msg!("In a sketch, lines are pasted with Paste or Aligned to Same Place.")),
                }
            }
            (Held::Elements { clip, .. }, None) => match paste {
                Paste::Pointer | Paste::SamePlace if clip.view_specific() => {
                    said(msg!("Dimensions copied alone are pasted into views: use Aligned to Current View or Aligned to Selected Views."))
                }
                Paste::Pointer if !self.editing_2d() => {
                    said(msg!("Paste follows the pointer in a plan or an elevation; in {3D}, use Aligned to Same Place."))
                }
                Paste::CurrentView | Paste::SelectedViews if !clip.view_specific() => said(msg!(
                    "Aligned to Current View and to Selected Views paste dimensions copied alone; forms, planes and lines go Aligned to Same Place."
                )),
                Paste::CurrentView if !self.editing_2d() => said(msg!("Dimensions are drawn in a plan or an elevation, not in {3D}.")),
                _ => None,
            },
        }
    }

    /// Copy to Clipboard, or Cut (`cut`) with the selection taken away as Delete takes it: what the clipboard then holds, and
    /// what to say; or why not, nothing changed.
    fn clip_selection(&mut self, doc: DocId, cut: bool) -> Result<(Held, String), String> {
        let family = self.title().to_string();
        if self.sketch.is_some() {
            let f = self.family()?;
            let constraints = self.sketch_dimensions();
            let Some(s) = self.sketch.as_mut() else { return Err(String::new()) };
            let lines: Vec<bimcraft_core::sketch::SketchLine> = s
                .sketch
                .picked
                .iter()
                .filter_map(|i| s.sketch.lines.get(*i))
                .filter(|l| l.style.as_deref() != Some(super::sketch::AXIS))
                .cloned()
                .collect();
            if lines.is_empty() {
                return Err(t("The Axis Line isn't copied: pick lines of the profile.").to_string());
            }
            let clip = SketchClip::of(s.guid, s.part as usize, &lines, &constraints, &reporting(&f));
            let n = clip.lines.len();
            if cut {
                s.delete_picked();
            }
            let said = tf("{} line(s) on the clipboard.", &[&n.to_string()]);
            return Ok((Held::Lines { doc, family, clip }, said));
        }
        let f = self.family()?;
        let mut c = Copying::default();
        for p in self.selection.items().iter().copied().filter(|p| self.holds(*p)) {
            match p {
                Picked::Plane(g) => c.planes.push(g),
                Picked::Line(g) => c.lines.push(g),
                Picked::ModelLine(g) => c.model_lines.push(g),
                Picked::Form(i) => c.forms.push(i),
                Picked::Symbol(i) => c.symbols.push(i),
                Picked::Connector(i) => c.connectors.push(i),
                Picked::Nested(i) => c.nested.push(i),
                Picked::Dimension(g) => c.dimensions.push(g),
            }
        }
        let clip = f.clip(&c).map_err(|r| copy_said(&r, &f))?;
        if clip.is_empty() {
            return Err(t("Nothing selected here can be copied.").to_string());
        }
        if cut {
            self.delete_selected()?;
        }
        let said = tf("{} element(s) on the clipboard.", &[&clip.count().to_string()]);
        Ok((Held::Elements { doc, family, clip: Box::new(clip) }, said))
    }

    /// Paste `how` of what `held` holds: done (what to say), begun (Paste following the pointer, the views' dialog), or why
    /// not, nothing changed.
    fn paste_held(&mut self, how: Paste, held: &Held) -> Result<Option<String>, String> {
        match held {
            Held::Lines { clip, .. } => {
                let Some(s) = self.sketch.as_mut() else { return Ok(None) };
                s.set_tool(None, false);
                s.edit = None;
                if how == Paste::SamePlace {
                    let frame = self.sketch_frame();
                    return Ok(self.paste_lines(clip, P2(0.0, 0.0), frame.as_ref()));
                }
                s.pasting = Some(clip.clone());
                Ok(None)
            }
            Held::Elements { clip, .. } => match how {
                Paste::Pointer => {
                    self.put_view_tools_down();
                    self.views.pasting = Some(Pasting { clip: (**clip).clone(), anchor: None });
                    Ok(None)
                }
                Paste::SamePlace => self.paste_elements(clip, None).map(Some),
                Paste::CurrentView => {
                    let Some(which) = self.views.shown else { return Ok(None) };
                    self.paste_dimensions(clip, &[which]).map(Some)
                }
                Paste::SelectedViews => {
                    let views = self.views_for(clip);
                    if views.is_empty() {
                        return Err(t("No view can draw the dimensions copied.").to_string());
                    }
                    self.views.paste_views =
                        Some(PasteViews { clip: (**clip).clone(), views: views.into_iter().map(|w| (w, false)).collect() });
                    Ok(None)
                }
                Paste::SelectedLevels | Paste::PickedLevel => Ok(None),
            },
        }
    }

    /// The views' tools put down for a paste: drawing, picking, dimensioning, Align, Pivot, a Modify tool, Component.
    fn put_view_tools_down(&mut self) {
        self.views.drawing = None;
        self.views.picking = None;
        self.views.dimensioning = None;
        self.views.aligning = None;
        self.views.align_lock = None;
        self.views.pivoting = None;
        self.views.modify = None;
        self.placing = None;
    }

    /// The clipboard's lines pasted into the sketch moved `by` on its plane (`frame` its axes), picked after, with the
    /// constraints among them: what to say.
    fn paste_lines(&mut self, clip: &SketchClip, by: P2, frame: Option<&bimcraft_core::family::Frame>) -> Option<String> {
        let s = self.sketch.as_mut()?;
        let along = frame.map_or([0.0; 3], |f| f.dir(by));
        let pasted = clip.paste(by, along);
        let start = s.sketch.lines.len();
        s.sketch.lines.extend(pasted.lines);
        for d in pasted.constraints {
            if d.alignment { s.locks.push(d) } else { s.dims.push(d) }
        }
        s.sketch.changed();
        s.sketch.picked = (start..s.sketch.lines.len()).collect();
        s.pasting = None;
        dropped_said(clip.dropped)
    }

    /// The clipboard's model elements pasted where they were copied, moved by `d` (the family's axes): one step, the copies
    /// selected; else everything as it was and why.
    pub(super) fn paste_elements(&mut self, clip: &Clip, d: Option<[f64; 3]>) -> Result<String, String> {
        let kept = Kept::of(self);
        let selected = self.selection.items().to_vec();
        let broken = self.broken_types();
        let sound = self.sound();
        let done = self.paste_inner(clip, d, &broken, sound);
        if done.is_err() {
            kept.put_back(self);
            self.selection.apply(&selected, How::Replace);
        }
        done
    }

    fn paste_inner(&mut self, clip: &Clip, d: Option<[f64; 3]>, broken: &[String], sound: bool) -> Result<String, String> {
        let f = self.family()?;
        let copies = f.paste(clip).map_err(|e| paste_said(&e))?;
        let (moved, chosen, _) = self.add_copies(copies, None);
        // A refusal names the copies where they are in the draft.
        let now = self.family()?;
        let names = Names { family: &now, origin: &Origin::default() };
        match d {
            Some(d) if d.iter().any(|v| v.abs() > 1e-9) => self.move_picked(&moved, d, broken, &names)?,
            _ => self.builds(broken, &names)?,
        }
        if sound && let Err(e) = self.family().and_then(|f| f.check(&|_| true)) {
            return Err(e);
        }
        self.selection.apply(&chosen, How::Replace);
        self.views.pasting = None;
        Ok(dropped_said(clip.dropped).unwrap_or_default())
    }

    /// The views that can draw the clipboard's dimensions: those where they pass the family's checks.
    fn views_for(&self, clip: &Clip) -> Vec<Which> {
        let Ok(f) = self.family() else { return Vec::new() };
        Which::ALL
            .into_iter()
            .filter(|w| {
                f.paste_into_view(clip, DimView::of(*w))
                    .is_ok_and(|made| Family { dimensions: [f.dimensions.clone(), made].concat(), ..f.clone() }.check(&|_| true).is_ok())
            })
            .collect()
    }

    /// The clipboard's dimensions drawn in each of `views`: one step, the new ones selected; else why not, nothing changed.
    fn paste_dimensions(&mut self, clip: &Clip, views: &[Which]) -> Result<String, String> {
        let f = self.family()?;
        let sound = self.sound();
        let mut made = Vec::new();
        for w in views {
            made.extend(f.paste_into_view(clip, DimView::of(*w)).map_err(|e| paste_said(&e))?);
        }
        let kept = self.dimensions.clone();
        let picked: Vec<Picked> = made.iter().map(|d| Picked::Dimension(d.guid)).collect();
        self.dimensions.extend(made);
        if sound && let Err(e) = self.family().and_then(|f| f.check(&|_| true)) {
            self.dimensions = kept;
            return Err(tf("The dimensions can't be drawn there: {}", &[&e]));
        }
        self.selection.apply(&picked, How::Replace);
        Ok(dropped_said(clip.dropped).unwrap_or_default())
    }

    /// A click at `q` (the view's mm) while Paste follows the pointer in view `which` (`space` its way into the family): the
    /// copies put there, their middle where the click is. What to say, or why not.
    pub(super) fn paste_click(&mut self, q: P2, which: Which, space: super::modify::Space) -> Result<String, String> {
        let Some(p) = self.views.pasting.clone() else { return Ok(String::new()) };
        let Some((_, anchor)) = p.anchor.filter(|(w, _)| *w == which) else {
            return Err(t("Nothing on the clipboard is drawn in this view: paste it in a view that shows it, or Aligned to Same Place.")
                .to_string());
        };
        let d = space.vector(q - anchor);
        self.paste_elements(&p.clip, Some(d))
    }

    /// The copies a paste following the pointer would make where they were copied, and the family with them: what its preview
    /// draws, moved with the pointer.
    pub(super) fn paste_preview(&self) -> Option<(Family, Vec<Picked>)> {
        let p = self.views.pasting.as_ref()?;
        let mut f = self.family().ok()?;
        let c = f.paste(&p.clip).ok()?;
        let mut items: Vec<Picked> = Vec::new();
        items.extend((0..c.forms.len()).map(|k| Picked::Form(f.forms.len() + k)));
        items.extend(c.planes.iter().map(|x| Picked::Plane(x.guid)));
        items.extend(c.lines.iter().map(|x| Picked::Line(x.guid)));
        items.extend(c.model_lines.iter().map(|x| Picked::ModelLine(x.guid)));
        items.extend((0..c.symbols.len()).map(|k| Picked::Symbol(f.symbols.len() + k)));
        items.extend((0..c.nested.len()).map(|k| Picked::Nested(f.nested_instances.len() + k)));
        f.forms.extend(c.forms);
        f.planes.extend(c.planes);
        f.lines.extend(c.lines);
        f.model_lines.extend(c.model_lines);
        f.symbols.extend(c.symbols);
        f.nested_instances.extend(c.nested);
        f.connectors.extend(c.connectors);
        f.dimensions.extend(c.dimensions);
        Some((f, items))
    }

    /// Aligned to Selected Views' dialog: the views that can draw the dimensions copied, each to tick; OK pastes into those
    /// ticked as one step. What to say, once it is done.
    pub(crate) fn paste_views_dialog(&mut self, ctx: &Context) -> Option<Result<String, String>> {
        let mut d = self.views.paste_views.clone()?;
        let (mut ok, mut cancel) = (false, false);
        let r = Modal::new(UiId::new("family-paste-views"), t("Paste Aligned to Selected Views")).width(360.0).show(ctx, |ui| {
            crate::popup::body(ui, |ui| {
                ui.label(t("Paste the dimensions into these views:"));
                ui.add_space(6.0);
                for (which, on) in &mut d.views {
                    ui.checkbox(on, super::views::view_title(Some(*which)));
                }
            });
            crate::popup::footer(ui, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    cancel = ui.button(t("Cancel")).clicked();
                    let any = d.views.iter().any(|(_, on)| *on);
                    ok = ui.add_enabled(any, egui::Button::new(t("OK"))).clicked();
                });
            });
        });
        if ok {
            self.views.paste_views = None;
            let views: Vec<Which> = d.views.iter().filter(|(_, on)| *on).map(|(w, _)| *w).collect();
            return Some(self.paste_dimensions(&d.clip, &views));
        }
        if cancel || r.should_close() {
            self.views.paste_views = None;
        } else {
            self.views.paste_views = Some(d);
        }
        None
    }

    /// Whether a paste follows the pointer, in a view or a sketch.
    pub(crate) fn pasting(&self) -> bool {
        self.views.pasting.is_some() || self.sketch.as_ref().is_some_and(|s| s.pasting.is_some())
    }

    /// A click on the sketch's plane at `p` while its paste follows the pointer: the lines put there, their middle at `p`.
    pub(crate) fn sketch_paste_click(&mut self, p: P2) -> Option<String> {
        let clip = self.sketch.as_ref()?.pasting.clone()?;
        let centre = clip.centre()?;
        let frame = self.sketch_frame();
        self.paste_lines(&clip, p - centre, frame.as_ref())
    }
}

/// Each command and why it can't be used now, if it can't, in the active document `doc`: what [`panel`] draws.
pub(crate) fn states(ed: &FamilyEditor, held: Option<&Held>, doc: DocId) -> Vec<(Command, Option<String>)> {
    Command::ALL.into_iter().map(|c| (c, ed.clipboard_blocked(c, held, doc))).collect()
}

/// `c` done in `ed` (document `doc`) with the window's clipboard `held`: what to say, or why not. A copy puts what it took
/// on `held`.
pub(crate) fn run(ed: &mut FamilyEditor, held: &mut Option<Held>, doc: DocId, c: Command) -> Result<Option<String>, String> {
    if let Some(why) = ed.clipboard_blocked(c, held.as_ref(), doc) {
        return Err(why);
    }
    match c {
        Command::Copy | Command::Cut => {
            let (new, said) = ed.clip_selection(doc, c == Command::Cut)?;
            *held = Some(new);
            Ok(Some(said))
        }
        Command::Paste(how) => match held.as_ref() {
            Some(h) => ed.paste_held(how, h),
            None => Ok(None),
        },
    }
}

/// The clipboard's key in the events of the frame: Ctrl+C, Ctrl+X or Ctrl+V, taken from the frame. Ctrl+C and Ctrl+X come as
/// the window's Copy and Cut. Ctrl+V comes as Paste only when the system's clipboard holds text, and then as nothing more, but
/// V let go with Ctrl still held always comes as a key: so Paste is heard either way, the system's clipboard never written or
/// read, and `heard` (kept from frame to frame) says a Paste came with this press, so its V let go isn't heard twice. A
/// harness's keys come as keys, down and up.
pub(crate) fn key(ctx: &Context, heard: &mut bool) -> Option<Command> {
    ctx.input_mut(|i| {
        let mut found = None;
        i.events.retain(|e| {
            let c = match e {
                egui::Event::Copy => Command::Copy,
                egui::Event::Cut => Command::Cut,
                egui::Event::Paste(_) => {
                    *heard = true;
                    Command::Paste(Paste::Pointer)
                }
                egui::Event::Key { key: egui::Key::V, pressed: true, modifiers, .. } if modifiers.command => {
                    *heard = true;
                    Command::Paste(Paste::Pointer)
                }
                egui::Event::Key { key: egui::Key::V, pressed: false, modifiers, .. } => {
                    // Its press was heard (a Paste, or the key down): this is the same Ctrl+V.
                    if std::mem::take(heard) || !modifiers.command {
                        return !modifiers.command;
                    }
                    Command::Paste(Paste::Pointer)
                }
                _ => return true,
            };
            found = found.or(Some(c));
            false
        });
        for (k, c) in [(egui::Key::C, Command::Copy), (egui::Key::X, Command::Cut)] {
            if i.consume_key(egui::Modifiers::COMMAND, k) {
                found = found.or(Some(c));
            }
        }
        found
    })
}

impl BimApp {
    /// `c` on the active family document, as its ribbon's Clipboard panel and its keys do it: the status bar says what came
    /// of it.
    pub(crate) fn family_clipboard(&mut self, c: Command) {
        let Some(doc) = self.active_doc() else { return };
        let mut held = self.clipboard.take();
        let done = match self.active_family_mut() {
            Some(ed) => run(ed, &mut held, doc, c),
            None => Ok(None),
        };
        self.clipboard = held;
        self.clipboard_said(done);
    }

    /// What came of a clipboard command, in the status bar.
    pub(crate) fn clipboard_said(&mut self, done: Result<Option<String>, String>) {
        match done {
            Ok(said) => self.status = Status { text: said.unwrap_or_default(), error: false },
            Err(text) => self.status = Status { text, error: true },
        }
    }
}
