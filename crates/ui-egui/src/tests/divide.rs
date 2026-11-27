//! Divide Parts in the desktop (HP-7b): the parts selected, a face of them picked in the 3D view, the division sketched on
//! it with the Draw panel's shapes and divided at Finish; Edit Division sketching a division again on its face.

use super::parts::{BOTH, HISTORIC, WALL, part_of, select};
use super::*;
use bimcraft_core::divide::{Cut, Frame};
use bimcraft_core::parts::BuildingPart;
use bimcraft_core::view3d::Camera;
use egui_kittest::kittest::NodeT;

use crate::sketch::Of;

/// The wall's south face, its plaster's outer face: drawn from east to west, the wall has its first layer, the plaster, on
/// its left, the south.
const SOUTH: f64 = -3240.0;

/// The historic wall drawn from east to west, so its plaster faces south.
fn facing_south() -> String {
    HISTORIC.replace("[[0,-3000],[4000,-3000]]", "[[4000,-3000],[0,-3000]]")
}

/// The wall taken apart, its plaster facing south.
fn surveyed() -> Harness<'static, BimApp> {
    let script = format!("{BOX}\n{}\ncreate_parts {{\"id\": {WALL}}}", facing_south());
    assert!(script.contains("[[4000,-3000],[0,-3000]]"));
    app(&script)
}

/// The wall's plaster and its brick.
fn layers(h: &Harness<BimApp>) -> (Id, Id) {
    let parts = ids(h, "part");
    let plaster = *parts.iter().find(|id| field(h, **id, "material") == json!("Plaster")).expect("the plaster");
    let brick = *parts.iter().find(|id| **id != plaster).expect("the brick");
    (plaster, brick)
}

/// The pieces divided from `part`.
fn pieces(h: &Harness<BimApp>, part: Id) -> Vec<Id> {
    let m = &h.state().session.model;
    let guid = m.guid(part);
    ids(h, "part").into_iter().filter(|id| part_of(h, *id).parent == Some(guid)).collect()
}

/// The 3D view turned to look at the wall from the south, a little above.
fn from_the_south(h: &mut Harness<BimApp>) {
    let cam = h.state().orbit.camera.expect("the 3D view's camera");
    h.state_mut().orbit.camera = Some(Camera { azimuth: -90.0, elevation: 15.0, target: [3000.0, -3000.0, 1500.0], lens: None, ..cam });
    h.run();
}

/// The face being divided on, once picked.
fn face(h: &Harness<BimApp>) -> Frame {
    h.state().dividing.as_ref().and_then(|d| d.face).expect("a face picked")
}

/// Where point `q` of the face being divided on shows.
fn on_face(h: &Harness<BimApp>, q: P2) -> Pos2 {
    let p = face(h).point(q, 0.0).expect("a plane");
    h.state().screen3(p)
}

/// The parts selected, Divide Parts pressed, and the wall's south face clicked in the 3D view.
fn dividing(h: &mut Harness<BimApp>, parts: &[Id]) {
    select(h, parts);
    from_list(h, "Part", "Divide Parts");
    assert_eq!(h.state().sheet, Sheet::View3D, "the face is picked in the 3D view");
    assert!(h.state().dividing.is_some() && h.state().sketch.is_none());
    pick_south(h);
}

/// The 3D view turned to the south, and the wall's south face clicked.
fn pick_south(h: &mut Harness<BimApp>) {
    from_the_south(h);
    let at = h.state().screen3([2000.0, SOUTH, 1500.0]);
    click_at(h, at);
}

#[test]
fn divide_parts_sketches_an_outline_on_the_face_picked_in_3d_and_finish_divides_the_part_in_one_step() {
    let mut h = surveyed();
    let (plaster, _) = layers(&h);
    select(&mut h, &[plaster]);
    assert!(!disabled_button(&h, "Divide Parts"));
    from_list(&mut h, "Part", "Divide Parts");
    assert!(h.state().status.text.contains("face"), "it asks for the face: {}", h.state().status.text);
    from_the_south(&mut h);
    let at = h.state().screen3([2000.0, SOUTH, 1500.0]);
    click_at(&mut h, at);
    assert_eq!(h.state().sketch.as_ref().map(|s| s.of), Some(Of::Divide), "{}", h.state().status.text);
    let f = face(&h);
    assert!((f.normal[1] + 1.0).abs() < 1e-9 && (f.origin[1] - SOUTH).abs() < 1.0, "the plaster's outer face: {f:?}");
    let cam = h.state().orbit.camera.expect("a camera");
    assert_eq!((cam.azimuth.round(), cam.elevation.round()), (-90.0, 0.0), "the view turned square to the face");
    assert!(h.query_by_label("Modify | Divide Parts").is_some());

    // A rectangle drawn on the face, its joint 10 mm wide.
    type_role(&mut h, Role::TextInput, "Gap", "10");
    press(&mut h, "Rectangle");
    let (a, b) = (on_face(&h, P2(-1000.0, -1000.0)), on_face(&h, P2(500.0, 500.0)));
    click_at(&mut h, a);
    click_at(&mut h, b);
    assert!(!h.state().sketch.as_ref().is_some_and(|s| s.lines.is_empty()), "{}", h.state().status.text);
    let before = h.state().session.history().len();
    press(&mut h, "Finish Edit Mode");
    assert!(h.state().sketch.is_none() && h.state().dividing.is_none(), "{}", h.state().status.text);

    let p = part_of(&h, plaster);
    let division = p.division.expect("divided");
    assert_eq!(division.cuts.len(), 1);
    assert_eq!(division.cuts[0].gap, 10.0);
    assert!(matches!(division.cuts[0].cut, Cut::Sketch { closed: true, .. }), "{division:?}");
    let made = pieces(&h, plaster);
    assert_eq!(made.len(), 2, "inside the outline and outside it");
    assert_eq!(h.state().selection, made, "the pieces selected");
    assert_eq!((h.state().session.history().len(), h.state().session.history().last()), (before + 1, Some(&"Divide Parts")));
    let shown = h.state().graphics_shown().parts_visibility;
    assert!(shown.unwrap_or_default().shows_parts(), "the 3D view shows the pieces");
    press(&mut h, "Undo");
    assert!(part_of(&h, plaster).division.is_none() && pieces(&h, plaster).is_empty());
}

#[test]
fn edit_division_sketches_the_division_again_on_its_face_and_a_piece_left_as_it_was_keeps_its_values() {
    let mut h = surveyed();
    let (plaster, _) = layers(&h);
    let frame = json!({"origin": [0.0, SOUTH, 0.0], "x": [1.0, 0.0, 0.0], "normal": [0.0, -1.0, 0.0]});
    let outline = json!([[1000.0, 500.0], [2000.0, 500.0], [2000.0, 1500.0], [1000.0, 1500.0]]);
    let cut = json!({"cut": {"sketch": {"frame": frame, "points": outline, "closed": true}}});
    h.state_mut().session.execute("divide_parts", &json!({"id": plaster, "cuts": [cut]})).expect("divided");
    let inside = *pieces(&h, plaster).iter().find(|id| field(&h, **id, "piece") == json!("0.inside.0")).expect("the inside");
    h.state_mut().session.execute("part", &json!({"id": inside, "decay_pathology": "Crust"})).expect("set");
    h.run();

    // Edit Division from a piece: the sketch on the cut's face at once, its outline one line.
    select(&mut h, &[inside]);
    assert!(disabled_button(&h, "Divide Parts"), "a piece of a division is divided again by Edit Division");
    from_list(&mut h, "Part", "Edit Division");
    assert_eq!(h.state().sheet, Sheet::View3D);
    assert_eq!(h.state().sketch.as_ref().map(|s| (s.of, s.lines.len())), Some((Of::Divide, 1)), "{}", h.state().status.text);
    assert!((face(&h).normal[1] + 1.0).abs() < 1e-9);

    // A line drawn right across the face above the outline.
    press(&mut h, "Line");
    let (a, b) = (on_face(&h, P2(-200.0, 2500.0)), on_face(&h, P2(4200.0, 2500.0)));
    click_at(&mut h, a);
    click_at(&mut h, b);
    press(&mut h, "Finish Edit Mode");
    assert!(h.state().sketch.is_none(), "{}", h.state().status.text);
    let division = part_of(&h, plaster).division.expect("divided");
    assert_eq!(division.cuts.len(), 2);
    assert!(matches!(division.cuts[1].cut, Cut::Sketch { closed: false, .. }));
    assert_eq!(pieces(&h, plaster).len(), 3, "the outline, and above and below the line round it");
    assert!(pieces(&h, plaster).contains(&inside), "the piece in the outline is the same piece");
    assert_eq!(field(&h, inside, "Decay_Pathology"), json!("Crust"), "with its values");
}

#[test]
fn divide_parts_divides_every_part_selected_and_a_wrong_face_esc_or_cancel_change_nothing() {
    let mut h = surveyed();
    let (plaster, brick) = layers(&h);
    // A face of something else is not taken; Esc gives up.
    select(&mut h, &[plaster, brick]);
    from_list(&mut h, "Part", "Divide Parts");
    from_the_south(&mut h);
    let other = h.state().screen3([6000.0, -100.0, 1500.0]);
    click_at(&mut h, other);
    assert!(h.state().status.error && h.state().sketch.is_none(), "{}", h.state().status.text);
    key(&mut h, Key::Escape);
    assert!(h.state().dividing.is_none());
    assert_eq!(h.state().selection, vec![plaster, brick]);

    // The view shows the wall, not its parts: its south face is the plaster's, not the brick's, so the brick alone isn't
    // divided on it.
    select(&mut h, &[brick]);
    from_list(&mut h, "Part", "Divide Parts");
    pick_south(&mut h);
    assert!(h.state().status.error && h.state().sketch.is_none(), "{}", h.state().status.text);
    key(&mut h, Key::Escape);

    // Cancel Edit Mode leaves the parts whole.
    dividing(&mut h, &[plaster, brick]);
    press(&mut h, "Rectangle");
    let (a, b) = (on_face(&h, P2(-500.0, -500.0)), on_face(&h, P2(500.0, 500.0)));
    click_at(&mut h, a);
    click_at(&mut h, b);
    press(&mut h, "Cancel Edit Mode");
    assert!(h.state().sketch.is_none() && h.state().dividing.is_none());
    assert!(pieces(&h, plaster).is_empty() && pieces(&h, brick).is_empty());
    assert_eq!(h.state().selection, vec![plaster, brick]);

    // Finished, the same outline divides both.
    dividing(&mut h, &[plaster, brick]);
    press(&mut h, "Rectangle");
    let (a, b) = (on_face(&h, P2(-500.0, -500.0)), on_face(&h, P2(500.0, 500.0)));
    click_at(&mut h, a);
    click_at(&mut h, b);
    press(&mut h, "Finish Edit Mode");
    assert!(h.state().sketch.is_none(), "{}", h.state().status.text);
    assert_eq!((pieces(&h, plaster).len(), pieces(&h, brick).len()), (2, 2));
    let p: BuildingPart = part_of(&h, brick);
    assert_eq!(p.division.map(|d| d.cuts.len()), Some(1));
}

fn disabled_button(h: &Harness<BimApp>, label: &str) -> bool {
    h.get_by_label(label).accesskit_node().is_disabled()
}

/// `BIMCRAFT_PARTS_DUMP=<folder>`: Divide Parts as it looks, in both profiles, light and dark: the Part panel, the sketch on
/// the face, and the pieces made.
#[test]
fn divide_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_PARTS_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    for (profile, build) in BOTH {
        for (dark, look) in [(false, "light"), (true, "dark")] {
            let shot = |h: &mut Harness<BimApp>, name: &str| {
                if dark {
                    h.ctx.set_theme(egui::Theme::Dark);
                }
                h.run();
                h.render().unwrap().save(std::path::Path::new(&folder).join(format!("divide-{profile}-{look}-{name}.png"))).unwrap();
            };
            let mut h = build(&format!("{BOX}\n{}\ncreate_parts {{\"id\": {WALL}}}", facing_south()));
            let (plaster, _) = layers(&h);
            select(&mut h, &[plaster]);
            shot(&mut h, "panel");
            if profile == "Rev" {
                dividing(&mut h, &[plaster]);
            } else {
                h.state_mut().divide_parts(vec![plaster]);
                h.run();
                pick_south(&mut h);
            }
            type_role(&mut h, Role::TextInput, "Gap", "10");
            press(&mut h, "Rectangle");
            for q in [P2(-1200.0, -1000.0), P2(300.0, 400.0)] {
                let at = on_face(&h, q);
                click_at(&mut h, at);
            }
            press(&mut h, "Line");
            for q in [P2(-2200.0, 900.0), P2(2200.0, 1100.0)] {
                let at = on_face(&h, q);
                click_at(&mut h, at);
            }
            key(&mut h, Key::Escape);
            shot(&mut h, "sketch");
            press(&mut h, "Finish Edit Mode");
            shot(&mut h, "pieces");
        }
    }
}
