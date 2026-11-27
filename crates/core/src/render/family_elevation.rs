//! GEO-2c-2e: a family's elevation, drawn as a section is, with what its picture was drawn from kept: each piece of a form,
//! a nested instance, a door's or a window's, as the scene built it (finer for the zoom, or its default build), for what a
//! click, an edge click and a box take. A project's sections keep nothing.

use super::*;
use crate::family::Piece;
use crate::family::views::DisplayedForms;
use std::sync::Mutex;

/// What a section's scene built of each component, door and window, kept as it is built (on several threads).
#[derive(Default)]
pub(crate) struct ElevationCapture(Mutex<DisplayedForms>);

impl ElevationCapture {
    /// Whether the view draws `piece` of an element whose x axis runs along `x`, by its Visibility Settings, as the picture does.
    fn drawn(piece: &Piece, x: P2, look: &Look) -> bool {
        look.forms_show(crate::family::visibility::packed(piece.vis, x), false)
    }

    /// Component pieces the scene is building, placed: their triangles and what of the family each is of.
    pub(crate) fn component(&self, pieces: &[Piece], x: P2, look: &Look) {
        let mut kept = self.0.lock().unwrap_or_else(|e| e.into_inner());
        for p in pieces.iter().filter(|p| Self::drawn(p, x, look)) {
            kept.push_mesh(p.form, p.path.clone(), p.tris.clone());
        }
    }

    /// A door's or a window's pieces, placed, as the scene's prepared solids hold them.
    pub(crate) fn opening(&self, pieces: Vec<Piece>, x: P2, look: &Look) {
        let mut kept = self.0.lock().unwrap_or_else(|e| e.into_inner());
        for p in pieces.into_iter().filter(|p| Self::drawn(p, x, look)) {
            kept.push_mesh(p.form, p.path, p.tris);
        }
    }
}

/// Elevation `id` of a family's scratch model through `view`: its drawing, its picture, and what the picture was drawn from,
/// as [`super::section_picture`] and [`super::section_over`] draw them (the section's work done once for both).
pub(crate) fn family_elevation(m: &Model, id: Id, view: View) -> Option<(Drawing, Image, DisplayedForms)> {
    let work = SectionWork::new(m);
    let capture = ElevationCapture::default();
    let image = section_picture_capturing(m, &work, id, (view, view), &[], Some(&capture))?;
    let drawing = section_in(m, &work, id, view, annotation(m, id, view), scale_of(m, id), Some(&image));
    let kept = capture.0.into_inner().unwrap_or_else(|e| e.into_inner());
    Some((drawing, image, kept))
}
