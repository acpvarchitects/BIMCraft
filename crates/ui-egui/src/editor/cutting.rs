//! FE-16b: Cut Geometry (CU), Uncut Geometry, Join Geometry (JO), Unjoin Geometry and Switch Join Order in a family document
//! that isn't adaptive, as Revit's family editor has them, on the core's cutting (`Family::cut`, `uncut`, `join`, `unjoin`,
//! `switch_join_order`: what cuts what among a family's forms beyond the list's order).
//!
//! Each is two picks. The first is the form to be cut or joined, the second the form that cuts it (for Unjoin and Switch Join
//! Order, the other of the pair). A form is picked by a click in a plan, an elevation or {3D}, or by its row in the forms list:
//! the views never draw a void, so a void cutter is picked from the list. The first form picked is selected, so it shows; Esc
//! lets it go, and then puts the tool down. A refusal of the core says why in the status bar and starts the pair again; the
//! tool stays in hand for the next pair. Each pair that works is one step of the document's history.
//!
//! A form the document holds always has a GUID (`FormText::of` gives one when it is made), which the core needs to name it in
//! a list; one that doesn't is given one when it is picked, so a form drawn without one can be cut and joined all the same.
use super::*;
use bimcraft_core::ids::Guid;

/// Which of the five tools is in hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CutTool {
    Cut,
    Uncut,
    Join,
    Unjoin,
    Switch,
}

/// The form picked first: by GUID, so a form moved about the list is the same; by its place while it has none yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct First {
    at: usize,
    guid: Guid,
}

/// The tool in hand, with the form picked first once there is one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cutting {
    pub(crate) mode: CutTool,
    pub(crate) first: Option<First>,
}

impl CutTool {
    /// The tool an action of `shortcuts::ACTIONS` names.
    pub(crate) fn of(action: &str) -> Option<CutTool> {
        Some(match action {
            "Cut Geometry" => CutTool::Cut,
            "Uncut Geometry" => CutTool::Uncut,
            "Join Geometry" => CutTool::Join,
            "Unjoin Geometry" => CutTool::Unjoin,
            "Switch Join Order" => CutTool::Switch,
            _ => return None,
        })
    }

    /// Its action's name.
    pub(crate) fn action(self) -> &'static str {
        match self {
            CutTool::Cut => "Cut Geometry",
            CutTool::Uncut => "Uncut Geometry",
            CutTool::Join => "Join Geometry",
            CutTool::Unjoin => "Unjoin Geometry",
            CutTool::Switch => "Switch Join Order",
        }
    }

    /// What the status bar asks for: the first pick, or the second.
    pub(crate) fn prompt(self, second: bool) -> &'static str {
        match (self, second) {
            (CutTool::Cut, false) => msg!("Cut Geometry: click the form to cut, in the view or in the forms list. Esc cancels."),
            (CutTool::Cut, true) => {
                msg!("Cut Geometry: click the form that cuts it, or a void's row in the forms list. Esc lets the first form go.")
            }
            (CutTool::Uncut, false) => msg!("Uncut Geometry: click the form that is cut, in the view or in the forms list. Esc cancels."),
            (CutTool::Uncut, true) => {
                msg!("Uncut Geometry: click the form that cuts it, or a void's row in the forms list. Esc lets the first form go.")
            }
            (CutTool::Join, false) => msg!("Join Geometry: click the form to join, in the view or in the forms list. Esc cancels."),
            (CutTool::Join, true) => {
                msg!("Join Geometry: click the solid form that cuts it where they overlap. Esc lets the first form go.")
            }
            (CutTool::Unjoin, false) => {
                msg!("Unjoin Geometry: click one of two joined forms, in the view or in the forms list. Esc cancels.")
            }
            (CutTool::Unjoin, true) => msg!("Unjoin Geometry: click the form it is joined to. Esc lets the first form go."),
            (CutTool::Switch, false) => {
                msg!("Switch Join Order: click one of two joined forms, in the view or in the forms list. Esc cancels.")
            }
            (CutTool::Switch, true) => msg!("Switch Join Order: click the form it is joined to. Esc lets the first form go."),
        }
    }
}

impl FamilyEditor {
    /// Why the tools can't be used now, if they can't: an adaptive family has no forms to cut, a sketch is a form's own mode.
    pub(crate) fn cutting_blocked(&self) -> Option<String> {
        if self.adaptive.is_some() {
            return Some(t("Cut and Join work on the forms of a family that isn't adaptive.").to_string());
        }
        if self.sketch.is_some() {
            return Some(t("Finish or cancel the sketch first: Cut and Join work on forms, not on a sketch's lines.").to_string());
        }
        None
    }

    /// `mode` in hand, the other tools put down and the selection let go; or why not.
    pub(crate) fn cutting_tool(&mut self, mode: CutTool) -> Result<(), String> {
        if let Some(why) = self.cutting_blocked() {
            return Err(why);
        }
        self.drop_tools();
        self.views.cutting = Some(Cutting { mode, first: None });
        self.last_tool = Some(mode.action());
        Ok(())
    }

    /// Form `i` as a draft drawn without a GUID has it.
    #[cfg(test)]
    pub(crate) fn forget_guid(&mut self, i: usize) {
        self.forms[i].original.guid = Guid::default();
    }

    /// What the tool in hand asks for, while one is.
    pub(crate) fn cutting_prompt(&self) -> Option<&'static str> {
        self.views.cutting.map(|c| c.mode.prompt(c.first.is_some()))
    }

    /// Escape with a tool in hand: the first form let go, else the tool put down. Whether one was in hand.
    pub(super) fn cutting_escape(&mut self) -> bool {
        match self.views.cutting {
            Some(Cutting { mode, first: Some(_) }) => {
                self.views.cutting = Some(Cutting { mode, first: None });
                self.let_everything_go();
            }
            Some(_) => {
                self.views.cutting = None;
                self.let_everything_go();
            }
            None => return false,
        }
        true
    }

    /// A click on form `form` (a place in the draft's list), or on nothing, with a tool in hand: its first pick, or its second,
    /// which makes the change. None where no tool is in hand; else what to say, said as done (`Ok`, maybe nothing) or refused.
    pub(crate) fn cutting_click(&mut self, form: Option<usize>) -> Option<Result<String, String>> {
        let Cutting { mode, first } = self.views.cutting?;
        let Some(i) = form.filter(|i| *i < self.forms.len()) else {
            return Some(Err(t("Click a form: in the view, or its row in the forms list.").to_string()));
        };
        // A form with no GUID yet is given one only once the pair works, so a refusal changes nothing.
        let guid = self.forms[i].original.guid;
        let Some(first) = first else {
            self.views.cutting = Some(Cutting { mode, first: Some(First { at: i, guid }) });
            self.select_only_form(i);
            return Some(Ok(String::new()));
        };
        if first.at == i && first.guid == guid {
            return Some(Err(t("That is the first form: pick another one.").to_string()));
        }
        // The first form may have gone since (an undo, a delete): the pair begins again.
        let a = if first.guid.is_nil() {
            Some(first.at).filter(|k| self.forms.get(*k).is_some_and(|f| f.original.guid.is_nil()))
        } else {
            self.forms.iter().position(|f| f.original.guid == first.guid)
        };
        let Some(a) = a else {
            self.views.cutting = Some(Cutting { mode, first: None });
            self.let_everything_go();
            return Some(Err(t("The first form is gone: pick it again.").to_string()));
        };
        // Either way the pair is done: the next click begins another.
        self.views.cutting = Some(Cutting { mode, first: None });
        self.let_everything_go();
        if a == i {
            return Some(Err(t("That is the first form: pick another one.").to_string()));
        }
        Some(self.cut_forms(mode, a, i))
    }

    /// `mode` made between forms `a` (the first picked) and `b`, the core's way, and its lists kept in the draft; or why not,
    /// the draft as it was.
    fn cut_forms(&mut self, mode: CutTool, a: usize, b: usize) -> Result<String, String> {
        let had: Vec<Guid> = self.forms.iter().map(|f| f.original.guid).collect();
        for i in [a, b] {
            if self.forms[i].original.guid.is_nil() {
                self.forms[i].original.guid = Guid::new();
            }
        }
        let done = self.cut_made(mode, a, b);
        if done.is_err() {
            for (form, guid) in self.forms.iter_mut().zip(had) {
                form.original.guid = guid;
            }
        }
        done
    }

    fn cut_made(&mut self, mode: CutTool, a: usize, b: usize) -> Result<String, String> {
        let mut family = self.family()?;
        match mode {
            CutTool::Cut => family.cut(a, b),
            CutTool::Uncut => family.uncut(a, b),
            CutTool::Join => family.join(a, b),
            CutTool::Unjoin => family.unjoin(a, b),
            CutTool::Switch => family.switch_join_order(a, b),
        }?;
        if family.forms.len() != self.forms.len() {
            return Err(t("The forms changed while they were cut: nothing was done.").to_string());
        }
        for (form, made) in self.forms.iter_mut().zip(family.forms) {
            (form.original.cut_by, form.original.uncut_by, form.original.joined_by) = (made.cut_by, made.uncut_by, made.joined_by);
        }
        let (x, y) = ((a + 1).to_string(), (b + 1).to_string());
        Ok(match mode {
            CutTool::Cut => tf("Form {} now cuts form {}.", &[&y, &x]),
            CutTool::Uncut => tf("Form {} no longer cuts form {}.", &[&y, &x]),
            CutTool::Join => tf("Form {} is joined to form {}, and cuts it.", &[&y, &x]),
            CutTool::Unjoin => tf("Forms {} and {} are not joined any more.", &[&x, &y]),
            CutTool::Switch => tf("The join of forms {} and {} is switched.", &[&x, &y]),
        })
    }
}
