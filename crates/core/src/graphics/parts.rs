//! What a view draws of the parts of building elements (`parts.rs`), as Revit's Parts Visibility: the original element,
//! the parts it was taken apart into, or both.
//!
//! A view's [`Look`] says it, as it says everything else a view hides and paints: a part is hidden unless the view shows
//! parts, and an element the view draws as its parts is hidden in its turn. So a plan, a section, an elevation and a 3D
//! view, which all ask the look whether to draw an element, agree with no code of their own; the 3D scene holds every
//! part's solid beside its host's, whatever any view shows of them.
//!
//! A part is drawn where its recipe holds, it isn't excluded and it wasn't divided into others ([`drawable`]), in a view
//! that draws its host. It is painted as its host is, then as the view filters that pick it say.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{Applied, Graphics, Look};
use crate::model::{Element, Id, Model};
use crate::parts::BuildingPart;

/// Which of an element and its parts a view draws, as Revit's Parts Visibility.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartsVisibility {
    /// Show Original: every element whole, and no part. What a view does unless it says.
    #[default]
    Original,
    /// Show Parts: an element that has parts as its parts, and no more of it; one that has none, whole.
    Parts,
    /// Show Both: every element whole, and its parts too.
    Both,
}

impl PartsVisibility {
    pub const ALL: [PartsVisibility; 3] = [PartsVisibility::Original, PartsVisibility::Parts, PartsVisibility::Both];

    /// Its name in commands and files.
    pub fn name(self) -> &'static str {
        match self {
            PartsVisibility::Original => "original",
            PartsVisibility::Parts => "parts",
            PartsVisibility::Both => "both",
        }
    }

    /// As Revit's Properties name it.
    pub fn label(self) -> &'static str {
        match self {
            PartsVisibility::Original => bimcraft_i18n::ui_msg!("Show Original"),
            PartsVisibility::Parts => bimcraft_i18n::ui_msg!("Show Parts"),
            PartsVisibility::Both => bimcraft_i18n::ui_msg!("Show Both"),
        }
    }

    /// By its name or its label, whatever its case, spaces, hyphens or underscores (`parts`, `Show Parts`, `show_parts`).
    pub fn parse(text: &str) -> Option<PartsVisibility> {
        let key: String = text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect();
        let key = key.strip_prefix("show").unwrap_or(&key);
        PartsVisibility::ALL.into_iter().find(|v| v.name() == key)
    }

    /// Whether a view draws parts at all.
    pub fn shows_parts(self) -> bool {
        self != PartsVisibility::Original
    }
}

/// Whether the project has parts: from the lookup when it is made, else from among every element.
pub fn any(m: &Model) -> bool {
    match m.derived.made(&m.elements) {
        Some(look) => look.any_parts(),
        None => m.elements.values().any(|e| matches!(e, Element::Part(_))),
    }
}

/// Whether part `p`'s recipe still applies to its host, as far as its host's type and what it was divided from say: the
/// conflicts [`Model::part_conflict`] finds without making its solid (a host with no solid there is the one it leaves out,
/// and then there is nothing to draw of the host either). What a view asks each time it is drawn.
pub fn holds(m: &Model, p: &BuildingPart) -> bool {
    let mut up = p.parent;
    let mut depth = 0;
    while let Some(g) = up {
        let Some(Element::Part(parent)) = m.elements.get(g) else { return false };
        if m.part_layer(parent).is_err() {
            return false;
        }
        depth += 1;
        if depth > crate::parts::MAX_DEPTH {
            return false;
        }
        up = parent.parent;
    }
    match (m.part_layer(p), m.elements.get(p.host)) {
        (Ok(layer), Some(host)) => layer.is_none_or(|(_, l)| l.thickness > 0.0) && crate::parts::HOSTS.contains(&host.category()),
        _ => false,
    }
}

/// Whether part `id` was divided into other parts, which stand for it.
pub fn divided(m: &Model, id: Id) -> bool {
    let Some(guid) = m.elements.guid(id) else { return false };
    m.lookup().referrers(&guid).any(|other| matches!(m.elements.get(other), Some(Element::Part(child)) if child.parent == Some(guid)))
}

/// Whether part `id`, `p`, is one a view that shows parts draws: its recipe holds, it isn't excluded, and it wasn't
/// divided into others. The parts the 3D scene makes solids of.
pub fn drawable(m: &Model, id: Id, p: &BuildingPart) -> bool {
    !p.excluded && holds(m, p) && !divided(m, id)
}

impl Look {
    /// The parts of the project, `parts`, as a view with graphics `g` shows them, once the look says what it shows of every
    /// other element. A part is hidden unless the view shows parts, it is one to draw ([`drawable`]) and the view draws its
    /// host; shown, it is painted as its host is, then as each of the `applied` filters that pick it says, the first
    /// prevailing. Showing parts alone, an element with a part to draw is hidden, its parts standing for it, but one with a
    /// part in conflict, which stays whole so that nothing of it goes missing.
    pub(super) fn parts(&mut self, m: &Model, g: &Graphics, parts: &[(Id, &BuildingPart)], applied: &[(&Applied, BTreeSet<Id>)]) {
        let mode = g.parts_visibility.unwrap_or_default();
        let (mut stood_for, mut broken) = (BTreeSet::new(), BTreeSet::new());
        for &(id, p) in parts {
            let host = m.elements.id(p.host);
            let holds = holds(m, p);
            if !holds && !p.excluded {
                broken.extend(host);
            }
            let Some(host) = host.filter(|h| mode.shows_parts() && holds && !p.excluded && !self.hidden.contains(h) && !divided(m, id))
            else {
                self.hidden.insert(id);
                continue;
            };
            stood_for.insert(host);
            let mut over = self.paint.get(&host).cloned().unwrap_or_default();
            for (a, _) in applied.iter().rev().filter(|(_, picked)| picked.contains(&id)) {
                over = a.over.over(&over);
            }
            if over.hidden {
                self.hidden.insert(id);
            } else if !over.is_empty() {
                self.paint.insert(id, over);
            }
        }
        if mode == PartsVisibility::Parts {
            for host in stood_for.difference(&broken) {
                self.hidden.insert(*host);
                self.paint.remove(host);
            }
        }
    }
}
