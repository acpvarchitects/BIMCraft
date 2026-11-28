//! GEO-2c-2e: a family's elevation, drawn as a section is, with what its picture was drawn from kept: each piece of a form,
//! a nested instance, a door's or a window's, as the scene built it (finer for the zoom, or its default build), for what a
//! click, an edge click and a box take; and, where the family stands in a wall, the pixels the wall shows, which hide what is
//! behind them. A project's sections keep nothing. (Ported from ec's 7f768c09e and d381329a2.)

use super::*;
use crate::family::Piece;
use crate::family::views::DisplayedForms;
use std::collections::HashMap;
use std::sync::Mutex;

/// What each piece of a door's or a window's mesh is of (its form, its nested path), in the order of its solids.
type OpeningParts = Vec<(Guid, Vec<Guid>)>;

/// What a section's scene built of each component, door and window, kept as it is built (on several threads).
#[derive(Default)]
pub(crate) struct ElevationCapture {
    shown: Mutex<DisplayedForms>,
    openings: Mutex<HashMap<Id, OpeningParts>>,
}

impl ElevationCapture {
    /// Whether the view draws `piece` of an element whose x axis runs along `x`, by its Visibility Settings, as the picture does.
    fn drawn(piece: &Piece, x: P2, look: &Look) -> bool {
        look.forms_show(crate::family::visibility::packed(piece.vis, x), false)
    }

    /// Component pieces the scene is building, placed: their triangles and what of the family each is of.
    pub(crate) fn component(&self, pieces: &[Piece], x: P2, look: &Look) {
        let mut kept = self.shown.lock().unwrap_or_else(|e| e.into_inner());
        for p in pieces.iter().filter(|p| Self::drawn(p, x, look)) {
            kept.push_mesh(p.form, p.path.clone(), p.tris.clone());
        }
    }

    /// What door or window `owner`'s `pieces` are of, as its solids are made of them: paired with the solids the scene takes.
    pub(crate) fn identify_opening(&self, owner: Id, pieces: &[Piece]) {
        if !pieces.is_empty() {
            self.openings
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(owner, pieces.iter().map(|p| (p.form, p.path.clone())).collect());
        }
    }

    /// Door or window `owner`'s `solids` as the scene holds them, each kept with what it is of where the view shows it: the
    /// mesh the picture is drawn from, built once for both.
    pub(crate) fn opening<'a>(&self, owner: Id, solids: impl Iterator<Item = &'a view3d::Solid>, look: &Look) {
        let Some(parts) = self.openings.lock().unwrap_or_else(|e| e.into_inner()).remove(&owner) else { return };
        let mut kept = self.shown.lock().unwrap_or_else(|e| e.into_inner());
        for ((form, path), solid) in parts.into_iter().zip(solids) {
            if let view3d::Solid::Mesh { tris, vis, .. } = solid
                && solid.seen_in(look)
                && look.forms_show(*vis, false)
            {
                kept.push_mesh(form, path, tris.clone());
            }
        }
    }
}

/// Elevation `id` of a family's scratch model through `view`: its drawing, its picture, and what the picture was drawn from,
/// as [`super::section_picture`] and [`super::section_over`] draw them (the section's work done once for both). Where the
/// family stands in a wall, the wall's pixels are kept too: what they show of it hides what is behind.
pub(crate) fn family_elevation(m: &Model, id: Id, view: View) -> Option<(Drawing, Image, DisplayedForms)> {
    let capture = ElevationCapture::default();
    let work = SectionWork::from_geometry(m, view3d::solids_and_extremes_capturing(m, &|| false, Some(&capture))?);
    let image = section_picture_capturing(m, &work, id, (view, view), &[], Some(&capture))?;
    let drawing = section_in(m, &work, id, view, annotation(m, id, view), scale_of(m, id), Some(&image));
    let mut kept = capture.shown.into_inner().unwrap_or_else(|e| e.into_inner());
    // A door's or a window's family stands in one wall of its own: only what of it the picture shows hides a pick.
    if let Some(wall) = m.elements.iter().find_map(|(id, e)| matches!(e, Element::Wall(_)).then_some(*id)) {
        kept.keep_host(view, &image, wall);
    }
    Some((drawing, image, kept))
}

#[cfg(test)]
mod hosted {
    use super::*;
    use crate::engine::Session;
    use crate::family::Family;
    use crate::family::views::{self, Side, Which};
    use crate::graphics::Override;
    use serde_json::json;

    /// A window 1200 wide: a jamb in its opening, and a flange of its own behind the wall past the right jamb.
    fn window() -> (Family, views::Staged, Id, Id) {
        let f: Family = serde_json::from_value(json!({
            "category": "window", "parameters": [{"name": "Width", "value": 1200}, {"name": "Height", "value": 1500}],
            "types": {"A": {}}, "forms": [
                {"guid": "00000000-0000-4000-8000-000000000501", "profile": [[440, -30], [560, -30], [560, 30], [440, 30]], "from": 400, "to": 1000},
                {"guid": "00000000-0000-4000-8000-000000000502", "profile": [[420, 240], [860, 240], [860, 280], [420, 280]], "from": 400, "to": 1000}
            ]
        }))
        .unwrap();
        let mut s = Session::new();
        s.model_mut().families.insert("W".into(), f.clone());
        s.execute("wall", &json!({"points": [[-2500, 0], [2500, 0]]})).unwrap();
        let wall = s.model.elements.iter().find_map(|(id, e)| matches!(e, Element::Wall(_)).then_some(*id)).unwrap();
        let window = s.execute("window", &json!({"at": [0, 0], "family": "W", "type": "A", "sill": 0})).unwrap()["id"].as_u64().unwrap();
        let values = f.values("A", &Default::default()).unwrap();
        let staged = views::stage(std::sync::Arc::unwrap_or_clone(s.model), &f, &values, &|_| true);
        (f, staged, wall, window)
    }

    fn draw(st: &mut views::Staged, which: Which) -> views::Lines {
        let middle = st.to_view(which, [620.0, 0.0, 700.0]);
        let view = View { cx: middle.0, cy: middle.1, width: 480, height: 320, scale: 0.6, angle: 0.0 };
        st.draw_lines_at(which, view, 1.0, P2(0.0, 0.0)).unwrap()
    }

    /// The window's placed pieces are built once for the picture and what picks in it (ec's review of #106).
    #[test]
    fn hosted_elevation_reuses_the_placed_mesh_for_the_picture_and_its_jamb_pick() {
        let (f, mut st, _, window) = window();
        let which = Which::Elevation(Side::Front);
        let jamb = st.to_view(which, [500.0, 0.0, 700.0]);
        // Counted without the fork's test-only twins (the picture's and the drawing's checks against the whole scene).
        crate::model::PLACED_OPENING_BUILDS.set(0);
        let drawn = without_twins(|| draw(&mut st, which));
        let builds = crate::model::PLACED_OPENING_BUILDS.get();
        let px = drawn.view.px(jamb);
        let picture = drawn.picture.unwrap();
        assert_eq!(picture.pick(px.0.into(), px.1.into()), Some(window));
        assert_eq!(views::form_at(&f, &f.values("A", &Default::default()).unwrap(), &st, which, jamb), Some(0));
        // What the drawing over the picture builds of it, counted apart.
        let m = &st.model;
        let id = st.elevation(Side::Front).unwrap();
        let work = SectionWork::new(m);
        crate::model::PLACED_OPENING_BUILDS.set(0);
        without_twins(|| section_in(m, &work, id, drawn.view, annotation(m, id, drawn.view), scale_of(m, id), Some(&picture)));
        let overlay = crate::model::PLACED_OPENING_BUILDS.get();
        assert_eq!(builds, overlay + 1, "one placed window mesh for both the picture and what picks");
    }

    /// Front: the jamb in the opening is the window's; the flange behind the wall is no one's where the picture shows the wall,
    /// to a click, an edge click and a box, until the wall is hidden.
    #[test]
    fn hosted_elevation_picks_the_jamb_and_the_flange_only_where_the_picture_exposes_it() {
        let (f, mut st, wall, window) = window();
        let which = Which::Elevation(Side::Front);
        let elevation = st.elevation(Side::Front).unwrap();
        let values = f.values("A", &Default::default()).unwrap();
        for (hidden, transparency) in [(false, 0), (true, 0), (false, 100)] {
            let Some(Element::Elevation(e)) = st.model.elements.get_mut(elevation) else { panic!("an elevation") };
            e.graphics.categories.insert("wall".into(), Override { hidden, transparency, ..Default::default() });
            let drawn = draw(&mut st, which);
            let picture = drawn.picture.unwrap();
            for (x, owner, form) in [(500.0, window, Some(0)), (750.0, if hidden { window } else { wall }, hidden.then_some(1))] {
                let q = st.to_view(which, [x, 0.0, 700.0]);
                let px = drawn.view.px(q);
                let why = format!("hidden {hidden}, transparency {transparency}, x {x}");
                assert_eq!(picture.pick(px.0.into(), px.1.into()), Some(owner), "{why}");
                assert_eq!(views::form_at(&f, &values, &st, which, q), form, "{why}");
            }
            let rim = st.to_view(which, [860.0, 0.0, 700.0]);
            assert_eq!(views::form_edge_at(&f, &values, &st, which, rim, 2.0), hidden.then_some(1));
            let q = st.to_view(which, [750.0, 0.0, 700.0]);
            let boxed = crate::geom::SelectBox::dragged(q + P2(4.0, -4.0), q + P2(-4.0, 4.0));
            assert_eq!(views::face_in_box(&f, &values, &st, which, views::Part::Form(1), &boxed), hidden);
        }
    }

    #[test]
    fn hosted_elevation_with_the_normal_comparison_oracles_reuses_the_displayed_mesh() {
        let (f, mut st, _, window) = window();
        let which = Which::Elevation(Side::Front);
        let jamb = st.to_view(which, [500., 0., 700.]);
        crate::model::PLACED_OPENING_BUILDS.set(0);
        let drawn = draw(&mut st, which);
        let builds = crate::model::PLACED_OPENING_BUILDS.get();
        let px = drawn.view.px(jamb);
        let picture = drawn.picture.unwrap();
        assert_eq!(picture.pick(px.0.into(), px.1.into()), Some(window));
        assert_eq!(views::form_at(&f, &f.values("A", &Default::default()).unwrap(), &st, which, jamb), Some(0));
        assert_eq!(crate::model::PLACED_OPENING_BUILDS.get(), builds, "picking the jamb reuses the displayed mesh");
        // Both the fork's section picture and its overlay can build comparison scenes in tests. Measure the
        // ordinary draw using previously prepared work, so these checks remain active in the editor's draw.
        let m = &st.model;
        let id = st.elevation(Side::Front).unwrap();
        let work = SectionWork::new(m);
        crate::model::PLACED_OPENING_BUILDS.set(0);
        let reference = section_picture_capturing(m, &work, id, (drawn.view, drawn.view), &[], None).unwrap();
        section_in(m, &work, id, drawn.view, annotation(m, id, drawn.view), scale_of(m, id), Some(&reference));
        let reference_builds = crate::model::PLACED_OPENING_BUILDS.get();
        assert_eq!(builds, reference_builds + 1, "one placed window mesh supplies both the picture and its pick identity");
    }
}
