//! FE-16c-1: Paint (PT) and Remove Paint in a family's {3D}: a click on a form's face paints that face (its GEO-2b key, the
//! family's own GUIDs, not {3D}'s staged copies') the material in the Options Bar, one step of the history; the picture shows
//! it; Remove Paint takes it off; a click on nothing says there's no face there.
use super::family_drag_move::{family, status};
use super::family_modify_3d::click3;
use super::family_views::{editor, open, view};
use super::preview_visibility::Pictures;
use super::*;
use bimcraft_core::family::FormFace;
use bimcraft_core::ids::Guid;
use egui::Color32;

const RIGHT: &str = "fe16c100-0000-4000-8000-000000000002";

/// A box 1000 by 500, 300 high, its lines with GUIDs (the right one `RIGHT`), in Concrete; and a red material.
fn script() -> String {
    let l =
        |n: u32, a: [i32; 2], b: [i32; 2]| format!(r#"{{"guid": "fe16c100-0000-4000-8000-00000000000{n}", "start": {a:?}, "end": {b:?}}}"#);
    format!(
        "material {{\"name\": \"Red\", \"color\": [220, 30, 30]}}\nfamily {{\"name\": \"Box\", \"category\": \"generic\", \"types\": {{\"T\": {{}}}}, \"forms\": [{{\"guid\": \"fe16c100-0000-4000-8000-000000000100\", \"sketch\": [{}, {}, {}, {}], \"from\": 0, \"to\": 300, \"material\": \"Concrete\"}}]}}",
        l(1, [0, 0], [1000, 0]),
        l(2, [1000, 0], [1000, 500]),
        l(3, [1000, 500], [0, 500]),
        l(4, [0, 500], [0, 0]),
    )
}

fn in_3d() -> (Harness<'static, BimApp>, Pictures) {
    let (mut h, pictures) = open(&script(), "Box");
    view(&mut h, None);
    for _ in 0..4 {
        h.run();
    }
    (h, pictures)
}

/// {3D}'s colour at family point `p`, in the last picture drawn.
fn colour(h: &Harness<BimApp>, pictures: &Pictures, p: [f64; 3]) -> Color32 {
    let named = |id: &egui::TextureId| h.ctx.tex_manager().read().meta(*id).is_some_and(|m| m.name == "family-preview");
    let img = pictures.0.lock().unwrap().iter().rev().find(|(id, _)| named(id)).map(|(_, p)| p.clone()).expect("a picture drawn");
    let (x, y, _) = editor(h).drawn_3d().expect("drawn").cam.project(p);
    img.pixels[y as usize * img.size[0] + x as usize]
}

fn red(c: Color32) -> bool {
    i32::from(c.r()) > i32::from(c.g()) + 80 && i32::from(c.r()) > i32::from(c.b()) + 80
}

fn painted(h: &Harness<BimApp>) -> Vec<(FormFace, String)> {
    family(h).forms[0].paint.iter().map(|p| (p.face.clone(), p.material.clone())).collect()
}

const TOP: [f64; 3] = [200.0, 250.0, 300.0];
const SIDE: [f64; 3] = [1000.0, 250.0, 150.0];

#[test]
fn paint_lays_a_material_on_the_face_clicked_in_3d_and_undo_and_remove_paint_take_it_off() {
    let (mut h, pictures) = in_3d();
    let bare = colour(&h, &pictures, TOP);
    assert!(!red(bare), "{bare:?}");
    shortcut(&mut h, "PT");
    assert_eq!(editor(&h).views.painting, Some(false));
    assert!(h.state().prompt().contains("Click a face to paint it"), "{}", h.state().prompt());
    h.state_mut().active_family_mut().unwrap().views.paint_material = "Red".into();
    h.run();
    let steps = editor(&h).steps();
    click3(&mut h, TOP);
    for _ in 0..4 {
        h.run();
    }
    assert_eq!(painted(&h), [(FormFace::End, "Red".to_string())], "{}", status(&h));
    assert_eq!(editor(&h).steps(), steps + 1, "a step of the history");
    let now = colour(&h, &pictures, TOP);
    assert!(red(now), "the top red in {{3D}}: {now:?}");
    // A side, named by the family's own GUID for its line, not the staged copy's.
    click3(&mut h, SIDE);
    h.run();
    assert_eq!(painted(&h)[1], (FormFace::side(Guid::parse(RIGHT).unwrap()), "Red".to_string()), "{}", status(&h));
    // Undo takes the side's paint back.
    h.state_mut().family_undo();
    h.run();
    assert_eq!(painted(&h).len(), 1);
    // Remove Paint takes the top's off; a click on nothing says so.
    press(&mut h, "Remove Paint");
    assert_eq!(editor(&h).views.painting, Some(true));
    click3(&mut h, TOP);
    for _ in 0..4 {
        h.run();
    }
    assert!(painted(&h).is_empty(), "{}", status(&h));
    assert!(!red(colour(&h, &pictures, TOP)));
    // The view's bottom left corner, where nothing stands.
    let empty = h.state().view_rect.left_bottom() + egui::vec2(40.0, -40.0);
    click_at(&mut h, empty);
    assert!(status(&h).contains("no face to paint"), "{}", status(&h));
    key(&mut h, egui::Key::Escape);
    assert_eq!(editor(&h).views.painting, None);
}
