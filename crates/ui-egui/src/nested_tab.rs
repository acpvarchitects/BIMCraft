//! T6b: Tab over a component in a section, an elevation or the 3D view reaches its shared nested instances, as a plan's Tab
//! does (`canvas.rs`, through `Drawing::pick_all`): the host first, then each of its shared nested instances under the
//! cursor, nearest first; a click takes the one Tab has reached. What such a view sees of them is the core's
//! (`Model::nested_seen`), and the one under the cursor or selected is outlined in the hover or the selection colour.
use bimcraft_core::model::{Element, Id, Model};
use bimcraft_core::render::{self, NestedPick};
use egui::{Painter, Pos2, Shape, Stroke};

use crate::canvas::PICK;
use crate::style::canvas_pal;

/// What Tab steps through in a section or the 3D view: what is under the cursor, host first, and how far Tab has gone.
#[derive(Clone, Debug, Default)]
pub(crate) struct Cycle {
    under: Vec<Id>,
    at: usize,
}

/// The shared nested instances a view through `project` sees of the hosts it may show lit: `hovered`, and the hosts of the
/// selected nested instances.
pub(crate) fn seen(m: &Model, hovered: Option<Id>, selection: &[Id], project: &dyn Fn([f64; 3]) -> Option<(f32, f32)>) -> Vec<NestedPick> {
    let mut hosts: Vec<Id> = hovered.into_iter().collect();
    for id in selection {
        if let Some(Element::Nested(n)) = m.elements.get(id)
            && let Some(h) = m.elements.id(n.host)
            && !hosts.contains(&h)
        {
            hosts.push(h);
        }
    }
    hosts.into_iter().flat_map(|h| m.nested_seen(h, project)).collect()
}

impl Cycle {
    /// What is under the cursor once Tab is counted: `hovered`, what the view picks there, and after it, when it is a
    /// component, its shared nested instances `seen` at `at` (the view's pixels); `tabbed`, whether Tab was pressed.
    pub(crate) fn step(&mut self, hovered: Option<Id>, seen: &[NestedPick], at: Option<(f32, f32)>, tabbed: bool) -> Option<Id> {
        let mut under: Vec<Id> = hovered.into_iter().collect();
        if let (Some(h), Some(at)) = (hovered, at) {
            let mine: Vec<NestedPick> = seen.iter().filter(|n| n.host == h).cloned().collect();
            under.extend(render::nested_at(&mine, at, PICK));
        }
        if under != self.under {
            *self = Cycle { under, at: 0 };
        }
        if tabbed && !self.under.is_empty() {
            self.at = (self.at + 1) % self.under.len();
        }
        self.under.get(self.at).copied()
    }
}

/// The shared nested instances of `seen` that are `hovered` or selected, outlined: `to_screen`, a view's pixel to the screen.
pub(crate) fn outline(p: &Painter, seen: &[NestedPick], hovered: Option<Id>, selection: &[Id], to_screen: &dyn Fn((f32, f32)) -> Pos2) {
    for n in seen {
        let accent = if hovered == Some(n.id) {
            canvas_pal().hover
        } else if selection.contains(&n.id) {
            canvas_pal().accent
        } else {
            continue;
        };
        for ring in &n.rings {
            p.add(Shape::closed_line(ring.iter().map(|q| to_screen(*q)).collect(), Stroke::new(2.0, accent)));
        }
    }
}
