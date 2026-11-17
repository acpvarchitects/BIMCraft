//! FE-23a: Selection Box (BX) in the family's {3D}, as the project's: with forms or nested instances selected, {3D} is cut to a
//! box round them, its cut faces filled; BX with nothing selected takes the box away. The view's own for the session, as its
//! other graphics are: not saved, no step of the history, no unsaved change; the inactive {3D} tile draws it too.
use super::preview_visibility::{Pictures, differ, preview};
use super::*;
use bimcraft_core::family::views::Which;

/// Preview Visibility's Cabinet, its two forms side by side (x 0 to 1000 and 2000 to 3000), on a screen tall enough for the
/// browser's Forms list.
fn cabinet() -> (Harness<'static, BimApp>, Pictures) {
    let (mut h, pictures) = super::preview_visibility::cabinet();
    h.set_size(vec2(1600.0, 1200.0));
    h.run();
    (h, pictures)
}

/// The section box {3D} keeps for now, if any.
fn boxed(h: &Harness<BimApp>) -> Option<([f64; 3], [f64; 3])> {
    h.state().active_family().and_then(|ed| ed.views.kept.section_box)
}

/// The last picture of the inactive {3D} tile.
fn tile(h: &Harness<BimApp>, pictures: &Pictures) -> egui::ColorImage {
    let named = |id: &egui::TextureId| h.ctx.tex_manager().read().meta(*id).is_some_and(|m| m.name == "family-tile-3d");
    let kept = pictures.0.lock().unwrap();
    kept.iter().rev().find(|(id, _)| named(id)).map(|(_, p)| p.clone()).expect("the tile was drawn")
}

#[test]
fn bx_cuts_3d_to_a_box_round_the_form_selected_and_bx_on_nothing_takes_it_away() {
    let (mut h, pictures) = cabinet();
    let shown = preview(&h, &pictures);
    let ground = shown.pixels[0];
    // With nothing selected, BX has nothing to box: it says so and changes nothing.
    shortcut(&mut h, "BX");
    assert!(boxed(&h).is_none());
    assert!(h.state().status.text.contains("Select"), "{:?}", h.state().status);
    // The first form selected: a box round it, a little bigger, which leaves the second form out.
    press(&mut h, "1. Extrusion");
    shortcut(&mut h, "BX");
    let (lo, hi) = boxed(&h).expect("a box round the form");
    assert!(lo[0] < 0.0 && hi[0] > 1000.0 && hi[0] < 2000.0, "round the first form, short of the second: {lo:?} {hi:?}");
    assert!(lo[2] < 0.0 && hi[2] > 500.0);
    let cut = preview(&h, &pictures);
    let gone = differ(&shown, &cut);
    assert!(gone.iter().filter(|&&i| cut.pixels[i] == ground).count() > 500, "the second form is gone");
    // Not the family's: no unsaved change, and nothing to undo.
    let ed = h.state().active_family().unwrap();
    assert!(!ed.unsaved());
    // BX on nothing: the box goes, and {3D} is drawn as it was.
    key(&mut h, Key::Escape);
    shortcut(&mut h, "BX");
    assert!(boxed(&h).is_none());
    assert_eq!(differ(&preview(&h, &pictures), &shown), Vec::<usize>::new());
}

#[test]
fn the_inactive_3d_tile_draws_the_box_too() {
    let (mut h, pictures) = cabinet();
    let doc = h.state().active_doc().unwrap();
    super::family_views::view(&mut h, Some(Which::Plan));
    shortcut(&mut h, "WT");
    h.run();
    let whole = tile(&h, &pictures);
    h.state_mut().open_sheet(Sheet::Family(doc, None));
    h.run();
    press(&mut h, "1. Extrusion");
    shortcut(&mut h, "BX");
    assert!(boxed(&h).is_some());
    super::family_views::view(&mut h, Some(Which::Plan));
    h.run();
    let cut = tile(&h, &pictures);
    assert!(differ(&whole, &cut).len() > 500, "the tile is drawn again, cut to the box");
}

#[test]
fn the_box_goes_with_the_family() {
    let (mut h, _) = cabinet();
    press(&mut h, "1. Extrusion");
    shortcut(&mut h, "BX");
    assert!(boxed(&h).is_some());
    press(&mut h, "Close Family");
    h.state_mut().edit_family("Cabinet");
    h.run();
    assert!(boxed(&h).is_none(), "the view's for the session: opened again, no box");
}

/// The pictures of FE-23a's PR: {3D} before and after BX on the first form, written to the folder `BIMCRAFT_FE23_DUMP` names.
#[test]
fn selection_box_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_FE23_DUMP") else { return };
    let save = |p: &egui::ColorImage, name: &str| {
        let bytes: Vec<u8> = p.pixels.iter().flat_map(|c| c.to_array()).collect();
        let image = image::RgbaImage::from_raw(p.size[0] as u32, p.size[1] as u32, bytes).unwrap();
        image.save(std::path::Path::new(&folder).join(name)).unwrap();
    };
    let (mut h, pictures) = cabinet();
    save(&preview(&h, &pictures), "fe23a-before.png");
    press(&mut h, "1. Extrusion");
    shortcut(&mut h, "BX");
    key(&mut h, Key::Escape);
    save(&preview(&h, &pictures), "fe23a-boxed.png");
}
