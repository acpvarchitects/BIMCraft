//! Preselection in a project section, elevation or 3D view. A click takes the ordinary front
//! owner until Tab reaches another projected owner or a shared child.
use bimcraft_core::model::{Element, Id, Model};
use bimcraft_core::render::{self, NestedPick};
use egui::{Painter, Pos2, Shape, Stroke};

use crate::canvas::PICK;
use crate::style::canvas_pal;

/// The view whose projected coordinates are used by the current pick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Frame {
    Orbit(Option<Id>, bimcraft_core::view3d::Camera),
    Section(Id, render::View, render::View),
}

pub(crate) struct Target<'a> {
    pub model: &'a std::sync::Arc<Model>,
    pub frame: Frame,
    pub hovered: Option<Id>,
    pub at: Option<(f32, f32)>,
    pub seen: &'a [NestedPick],
    pub project: &'a dyn Fn([f64; 3]) -> Option<(f32, f32)>,
}

/// Keep an expanded candidate list only for the displayed model, view and pointer location.
#[derive(Clone, Debug, Default)]
pub(crate) struct Cycle {
    model: std::sync::Weak<Model>,
    pointer: Option<(Frame, (i32, i32))>,
    front: Option<Id>,
    under: Vec<Id>,
    at: usize,
    geometry: Vec<bimcraft_core::view3d::PickCandidate>,
    nested: Vec<NestedPick>,
    expanded: bool,
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
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn current(&self) -> Option<Id> {
        self.under.get(self.at).copied()
    }

    pub(crate) fn passed_front(&self, at: Option<(f32, f32)>) -> bool {
        self.at > 0 && self.pointer.map(|(_, point)| point) == at.map(|(x, y)| (x.floor() as i32, y.floor() as i32))
    }

    pub(crate) fn step(
        &mut self,
        target: Target<'_>,
        tabbed: bool,
        candidates: impl FnOnce((f32, f32)) -> (Vec<Id>, Vec<bimcraft_core::view3d::PickCandidate>),
    ) -> Option<Id> {
        let Target { model, frame, hovered, at, seen, project } = target;
        let (Some(at), Some(front)) = (at, hovered) else {
            self.clear();
            return None;
        };
        let pointer = Some((frame, (at.0.floor() as i32, at.1.floor() as i32)));
        let new_front = hovered != self.front && hovered != self.current();
        if pointer != self.pointer || self.model.as_ptr() != std::sync::Arc::as_ptr(model) || new_front {
            self.clear();
            self.model = std::sync::Arc::downgrade(model);
            self.pointer = pointer;
            self.front = hovered;
            self.under.push(front);
        }
        if tabbed && !self.expanded {
            let (overlay, geometry) = candidates(at);
            let mut visited = std::collections::BTreeSet::new();
            self.under.clear();
            for owner in hovered.into_iter().chain(overlay).chain(geometry.iter().map(|p| p.id)) {
                if !visited.insert(owner) {
                    continue;
                }
                self.under.push(owner);
                let children: Vec<_> = if Some(owner) == hovered {
                    seen.iter().filter(|n| n.host == owner).cloned().collect()
                } else {
                    model.nested_seen(owner, project)
                };
                for child in render::nested_at(&children, at, PICK) {
                    if visited.insert(child) {
                        self.under.push(child);
                    }
                }
                if Some(owner) != hovered {
                    self.nested.extend(children);
                }
            }
            self.geometry = geometry;
            self.expanded = true;
        }
        if tabbed && !self.under.is_empty() {
            self.at = (self.at + 1) % self.under.len();
        }
        self.current()
    }

    pub(crate) fn draw(&self, p: &Painter, to_screen: impl Fn(bimcraft_core::geom::P2) -> Pos2) {
        if self.at == 0 {
            return;
        }
        let Some(id) = self.current() else {
            return;
        };
        if let Some(hit) = self.geometry.iter().find(|p| p.id == id) {
            let stroke = Stroke::new(2., canvas_pal().hover);
            for [a, b] in &hit.edges {
                p.line_segment([to_screen(*a), to_screen(*b)], stroke);
            }
        }
    }

    pub(crate) fn nested(&self) -> &[NestedPick] {
        &self.nested
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
