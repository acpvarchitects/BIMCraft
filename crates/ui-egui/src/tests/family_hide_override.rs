//! FE-18, part 2: Hide in View (EH, VH), Reveal Hidden Elements (RH), Unhide (EU, VU) and Override Graphics by Element (EO) in the
//! family document, each view's own for the session, as part 1's Visibility/Graphics. A form hidden by element, or with its
//! category, is left out of that view alone; Reveal Hidden Elements draws what the view hides in magenta inside the project's
//! frame, where it can be picked and unhidden; an override draws a form otherwise in that view alone. None of it is saved, a
//! step of the history, or an unsaved change.
use super::family_views::{around, click_family, darkest, editor, open, select_form, view};
use super::*;
use bimcraft_core::family::views::{Side, Which};
use egui::Color32;
use egui_kittest::kittest::NodeT;

/// Two boxes from Ref. Level to 2 m, A west of B, and a plane of the family's own, Edge, west of both.
const BOXES: &str = r#"family {"name": "Boxes", "category": "generic", "types": {"T": {}}, "forms": [{"profile": [[500, 300], [1000, 300], [1000, 600], [500, 600]], "to": 2000}, {"profile": [[1500, 300], [2000, 300], [2000, 600], [1500, 600]], "to": 2000}], "ref_planes": [{"name": "Edge", "start": [-400, -2000, 0], "end": [-400, 2000, 0], "up": [0, 0, 1]}]}"#;

/// Box A's and box B's front edges in plan, and box A's face in the Front elevation; Edge's trace in plan.
const A: [f64; 3] = [750.0, 300.0, 0.0];
const B: [f64; 3] = [1750.0, 300.0, 0.0];
const A_FRONT: [f64; 3] = [1000.0, 0.0, 1000.0];
const EDGE: [f64; 3] = [-400.0, 800.0, 0.0];

/// Whether the family document is as it was opened: no step of its history, no unsaved change.
fn untouched(h: &Harness<BimApp>) -> bool {
    let ed = editor(h);
    ed.steps() == 0 && !ed.unsaved()
}

/// Magenta, Reveal Hidden Elements' colour.
fn magenta(c: &Color32) -> bool {
    c.r() > 180 && c.b() > 180 && c.g() < 120
}

fn green(c: &Color32) -> bool {
    i16::from(c.g()) - i16::from(c.r()) > 12 && i16::from(c.g()) - i16::from(c.b()) > 8
}

/// Whether Edge's dashed green trace shows near `p`.
fn edge_shown(h: &Harness<BimApp>, pictures: &super::preview_visibility::Pictures) -> bool {
    (0..12).any(|k| around(h, pictures, [EDGE[0], EDGE[1] + f64::from(k) * 25.0, 0.0], 2).iter().any(green))
}

#[test]
fn hide_element_hides_a_form_in_the_view_alone_and_reveal_hidden_unhides_it() {
    let (mut h, pictures) = open(BOXES, "Boxes");
    view(&mut h, Some(Which::Plan));
    select_form(&mut h, 1);
    shortcut(&mut h, "EH");
    let guid = editor(&h).form_guid(0).unwrap();
    assert!(editor(&h).views.kept.graphics_of(Some(Which::Plan)).elements.get(&guid).is_some_and(|o| o.hidden));
    assert!(darkest(&around(&h, &pictures, A, 2)) > 740, "box A hidden in the plan");
    assert!(darkest(&around(&h, &pictures, B, 2)) < 200, "box B stays");
    assert_eq!(editor(&h).selected_form(), None, "the selection goes, as the project's Hide in View lets it go");
    // A click where it was picks nothing: hidden, it can't be selected.
    click_family(&mut h, A);
    assert_eq!(editor(&h).selected_form(), None);
    // The plan's alone.
    view(&mut h, Some(Which::Elevation(Side::Front)));
    assert!(darkest(&around(&h, &pictures, A_FRONT, 2)) < 300, "Front still draws box A");
    view(&mut h, Some(Which::Plan));
    assert!(untouched(&h));
    // Reveal Hidden Elements: box A in magenta, picked by a click and unhidden by EU.
    shortcut(&mut h, "RH");
    assert!(editor(&h).views.kept.revealing(Some(Which::Plan)));
    assert!(around(&h, &pictures, A, 2).iter().any(magenta), "box A revealed in magenta");
    assert!(!around(&h, &pictures, B, 2).iter().any(magenta), "box B as it is");
    click_family(&mut h, A);
    assert_eq!(editor(&h).selected_form(), Some(0), "revealed, it can be picked");
    shortcut(&mut h, "EU");
    assert!(!editor(&h).views.kept.graphics_of(Some(Which::Plan)).elements.contains_key(&guid), "unhidden");
    shortcut(&mut h, "RH");
    assert!(!editor(&h).views.kept.revealing(Some(Which::Plan)));
    assert!(darkest(&around(&h, &pictures, A, 2)) < 200, "box A drawn again");
    assert!(untouched(&h));
}

#[test]
fn hide_category_hides_every_form_and_unhide_category_brings_them_back() {
    let (mut h, pictures) = open(BOXES, "Boxes");
    view(&mut h, Some(Which::Plan));
    select_form(&mut h, 2);
    shortcut(&mut h, "VH");
    assert!(darkest(&around(&h, &pictures, A, 2)) > 740 && darkest(&around(&h, &pictures, B, 2)) > 740, "both boxes hidden");
    // It is Visibility/Graphics' row: unticked there.
    shortcut(&mut h, "VV");
    assert_eq!(
        h.get_by_role_and_label(Role::CheckBox, "Generic Components").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    press(&mut h, "Cancel");
    // Revealed, both in magenta; one picked and VU unhides the category.
    shortcut(&mut h, "RH");
    assert!(around(&h, &pictures, A, 2).iter().any(magenta) && around(&h, &pictures, B, 2).iter().any(magenta));
    click_family(&mut h, B);
    shortcut(&mut h, "VU");
    shortcut(&mut h, "RH");
    assert!(darkest(&around(&h, &pictures, A, 2)) < 200 && darkest(&around(&h, &pictures, B, 2)) < 200, "both back");
    assert!(untouched(&h));
}

#[test]
fn hide_element_hides_a_reference_plane_in_the_view() {
    let (mut h, pictures) = open(BOXES, "Boxes");
    view(&mut h, Some(Which::Plan));
    assert!(edge_shown(&h, &pictures));
    click_family(&mut h, [-400.0, -1000.0, 0.0]);
    assert!(editor(&h).selected_plane().is_some(), "Edge");
    shortcut(&mut h, "EH");
    assert!(!edge_shown(&h, &pictures), "Edge hidden in the plan");
    click_family(&mut h, [-400.0, -1000.0, 0.0]);
    assert!(editor(&h).selected_plane().is_none(), "hidden, it can't be picked");
    // Revealed, it is picked and unhidden.
    shortcut(&mut h, "RH");
    assert!(edge_shown(&h, &pictures) || around(&h, &pictures, EDGE, 30).iter().any(magenta), "revealed");
    click_family(&mut h, [-400.0, -1000.0, 0.0]);
    shortcut(&mut h, "EU");
    shortcut(&mut h, "RH");
    assert!(edge_shown(&h, &pictures), "back");
    // VH on a plane alone says why it hides nothing.
    click_family(&mut h, [-400.0, -1000.0, 0.0]);
    shortcut(&mut h, "VH");
    assert!(h.state().status.error, "{}", h.state().status.text);
    assert!(edge_shown(&h, &pictures));
}

#[test]
fn override_graphics_draws_a_form_otherwise_in_the_view_alone() {
    let (mut h, pictures) = open(BOXES, "Boxes");
    view(&mut h, Some(Which::Plan));
    let ink = darkest(&around(&h, &pictures, A, 2));
    select_form(&mut h, 1);
    shortcut(&mut h, "EO");
    assert!(h.query_by_label("View-Specific Element Graphics").is_some(), "EO opens the dialog");
    // Cancel changes nothing.
    pick(&mut h, Role::CheckBox, "Halftone");
    press(&mut h, "Cancel");
    assert!(editor(&h).views.kept.graphics_of(Some(Which::Plan)).elements.is_empty());
    select_form(&mut h, 1);
    shortcut(&mut h, "EO");
    pick(&mut h, Role::CheckBox, "Halftone");
    press(&mut h, "OK");
    let guid = editor(&h).form_guid(0).unwrap();
    assert!(editor(&h).views.kept.graphics_of(Some(Which::Plan)).elements.get(&guid).is_some_and(|o| o.halftone && !o.hidden));
    let half = darkest(&around(&h, &pictures, A, 2));
    assert!(half > ink + 100 && half < 740, "box A halftone: {half} against {ink}");
    assert!(darkest(&around(&h, &pictures, B, 2)) < 200, "box B as it was");
    // The plan's alone.
    view(&mut h, Some(Which::Elevation(Side::Front)));
    assert!(darkest(&around(&h, &pictures, A_FRONT, 2)) < 300, "Front draws box A as it was");
    assert!(untouched(&h));
    // Nothing selected: EO says so.
    view(&mut h, Some(Which::Plan));
    key(&mut h, Key::Escape);
    shortcut(&mut h, "EO");
    assert!(h.query_by_label("View-Specific Element Graphics").is_none());
    assert!(h.state().status.error, "{}", h.state().status.text);
}

#[test]
fn apply_hide_isolate_to_view_keeps_what_is_hidden_for_now_for_good() {
    let (mut h, pictures) = open(BOXES, "Boxes");
    view(&mut h, Some(Which::Plan));
    select_form(&mut h, 1);
    shortcut(&mut h, "HH");
    assert!(editor(&h).temporary().hides());
    press(&mut h, "Temporary Hide/Isolate");
    press(&mut h, "Apply Hide/Isolate to View");
    assert!(!editor(&h).temporary().hides(), "nothing hidden for now any more");
    let guid = editor(&h).form_guid(0).unwrap();
    assert!(editor(&h).views.kept.graphics_of(Some(Which::Plan)).elements.get(&guid).is_some_and(|o| o.hidden), "hidden for good");
    assert!(darkest(&around(&h, &pictures, A, 2)) > 740);
    assert!(untouched(&h));
}

#[test]
fn the_view_control_bar_has_reveal_hidden_elements() {
    let (mut h, _) = open(BOXES, "Boxes");
    view(&mut h, Some(Which::Plan));
    press(&mut h, "Reveal Hidden Elements");
    assert!(editor(&h).views.kept.revealing(Some(Which::Plan)));
    assert!(lit(&h, "Reveal Hidden Elements"));
    press(&mut h, "Reveal Hidden Elements");
    assert!(!editor(&h).views.kept.revealing(Some(Which::Plan)));
}

#[test]
fn hide_and_override_work_in_3d_too() {
    use super::preview_visibility::{differ, preview};
    let (mut h, pictures) = open(BOXES, "Boxes");
    view(&mut h, None);
    let shown = preview(&h, &pictures);
    select_form(&mut h, 1);
    shortcut(&mut h, "EO");
    pick(&mut h, Role::CheckBox, "Halftone");
    press(&mut h, "OK");
    let half = preview(&h, &pictures);
    assert!(differ(&shown, &half).len() > 300, "box A halftone in {{3D}}");
    assert!(editor(&h).views.kept.graphics_of(Some(Which::Plan)).elements.is_empty(), "{{3D}}'s alone");
    select_form(&mut h, 1);
    shortcut(&mut h, "EH");
    let gone = preview(&h, &pictures);
    // Box A stands partly before box B: where it was, the ground or box B.
    let ground = gone.pixels[0];
    let drawn = |p: &egui::ColorImage| p.pixels.iter().filter(|c| **c != ground).count();
    assert!(drawn(&gone) + 300 < drawn(&half), "box A gone: {} drawn against {}", drawn(&gone), drawn(&half));
    shortcut(&mut h, "RH");
    let revealed = preview(&h, &pictures);
    assert!(revealed.pixels.iter().any(magenta), "box A in magenta");
    assert!(untouched(&h));
}

/// Box A hidden and revealed in the plan, and View-Specific Element Graphics, in the three looks, when `BIMCRAFT_FE18_DUMP`
/// names a folder.
#[test]
fn family_hide_and_override_in_three_looks() {
    let Ok(folder) = std::env::var("BIMCRAFT_FE18_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    for (name, dark, light_canvas) in [("light", false, false), ("dark", true, false), ("dark-ui-light-canvas", true, true)] {
        let mut h = app(BOXES);
        h.set_size(egui::vec2(1600.0, 1000.0));
        h.state_mut().edit_family("Boxes");
        h.run();
        h.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
        h.state_mut().light_canvas = light_canvas;
        let save = |h: &mut Harness<BimApp>, what: &str| {
            for _ in 0..4 {
                h.run();
            }
            h.render().unwrap().save(std::path::Path::new(&folder).join(format!("fe18b-{what}-{name}.png"))).unwrap();
        };
        view(&mut h, Some(Which::Plan));
        select_form(&mut h, 1);
        shortcut(&mut h, "EH");
        shortcut(&mut h, "RH");
        save(&mut h, "revealed");
        shortcut(&mut h, "RH");
        select_form(&mut h, 2);
        shortcut(&mut h, "EO");
        save(&mut h, "override");
        press(&mut h, "Cancel");
    }
}
