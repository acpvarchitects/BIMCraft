//! Modify > Geometry > Join: Join Geometry, Switch Join Order and Unjoin Geometry in a plan (T23): pick one of the walls,
//! floors, roofs and ceilings, then the other; with Join, the first cuts the second.

use super::*;
use crate::JoinMode;

/// A wall along x at y 2000 and a floor through its foot (from 100 to 300 up), whose south edge runs along y 0; a second
/// wall at y 9000, clear of both; a column, which doesn't join.
const CROSSING: &str = r#"wall {"points": [[0, 2000], [6000, 2000]], "height": 3000}
floor {"boundary": [[0, 0], [6000, 0], [6000, 4000], [0, 4000]], "offset": 300}
wall {"points": [[0, 9000], [6000, 9000]], "height": 3000}
column {"at": [9000, 2000]}"#;

/// On the wall, on the floor's south edge, on the far wall and on the column.
const WALL: P2 = P2(1000.0, 2000.0);
const FLOOR: P2 = P2(3000.0, 0.0);
const FAR: P2 = P2(3000.0, 9000.0);
const COLUMN: P2 = P2(9000.0, 2000.0);

/// The joins there are, as (the one that cuts, the one it cuts).
fn joins(h: &Harness<BimApp>) -> Vec<(Id, Id)> {
    let m = &h.state().session.model;
    m.elements.iter().flat_map(|(id, e)| e.joined_by().iter().filter_map(|g| m.elements.id(*g)).map(|by| (by, *id))).collect()
}

fn wall_and_floor(h: &Harness<BimApp>) -> (Id, Id) {
    (ids(h, "wall")[0], ids(h, "floor")[0])
}

#[test]
fn join_takes_one_then_the_other_and_switch_and_unjoin_change_it() {
    let mut h = app(CROSSING);
    let (w, f) = wall_and_floor(&h);
    topmost(&mut h, "Modify");
    press(&mut h, "Join");
    press(&mut h, "Join Geometry");
    assert!(h.state().prompt().contains("first"), "{}", h.state().prompt());
    // A column doesn't join: a click on it picks nothing.
    click(&mut h, COLUMN);
    assert!(matches!(h.state().tool, Tool::JoinGeometry { first: None, .. }));
    click(&mut h, WALL);
    assert_eq!(h.state().tool, Tool::JoinGeometry { mode: JoinMode::Join, first: Some(w) });
    assert!(h.state().prompt().contains("cuts"), "{}", h.state().prompt());
    // The one picked isn't the second too.
    click(&mut h, WALL);
    assert_eq!(joins(&h), []);
    click(&mut h, FLOOR);
    assert_eq!(joins(&h), [(w, f)], "{:?}", h.state().status);
    // The tool goes on, for the next pair.
    assert_eq!(h.state().tool, Tool::JoinGeometry { mode: JoinMode::Join, first: None });
    // Switch Join Order, the pair picked either way round.
    press(&mut h, "Join");
    press(&mut h, "Switch Join Order");
    click(&mut h, WALL);
    click(&mut h, FLOOR);
    assert_eq!(joins(&h), [(f, w)], "{:?}", h.state().status);
    press(&mut h, "Join");
    press(&mut h, "Unjoin Geometry");
    click(&mut h, WALL);
    click(&mut h, FLOOR);
    assert_eq!(joins(&h), [], "{:?}", h.state().status);
    // One undo step each.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(joins(&h), [(f, w)]);
}

#[test]
fn jo_starts_join_geometry_and_esc_lets_go_of_the_first_then_the_tool() {
    let mut h = app(CROSSING);
    let before = h.state().session.model.elements.clone();
    shortcut(&mut h, "JO");
    assert_eq!(h.state().tool, Tool::JoinGeometry { mode: JoinMode::Join, first: None });
    click(&mut h, WALL);
    assert!(matches!(h.state().tool, Tool::JoinGeometry { first: Some(_), .. }));
    key(&mut h, Key::Escape);
    assert_eq!(h.state().tool, Tool::JoinGeometry { mode: JoinMode::Join, first: None });
    key(&mut h, Key::Escape);
    assert_eq!(h.state().tool, Tool::Modify);
    assert_eq!(h.state().session.model.elements, before, "Esc twice edits nothing");
}

#[test]
fn a_join_the_core_refuses_says_why_in_the_windows_words() {
    // Two that don't meet.
    let mut h = app(CROSSING);
    shortcut(&mut h, "JO");
    click(&mut h, WALL);
    click(&mut h, FAR);
    assert_eq!(joins(&h), []);
    let status = &h.state().status;
    assert!(status.error, "{status:?}");
    assert_eq!(status.text, "These two don't meet. Move one so they overlap or share a face, then join them.");
    // Joined already: Switch Join Order is what changes it.
    click(&mut h, WALL);
    click(&mut h, FLOOR);
    assert_eq!(joins(&h).len(), 1, "{:?}", h.state().status);
    click(&mut h, FLOOR);
    click(&mut h, WALL);
    assert_eq!(joins(&h).len(), 1);
    assert_eq!(h.state().status.text, "These two are joined already. Switch Join Order changes which cuts which.");
    // Unjoin and Switch on two that aren't joined.
    for item in ["Unjoin Geometry", "Switch Join Order"] {
        topmost(&mut h, "Modify");
        press(&mut h, "Join");
        press(&mut h, item);
        click(&mut h, WALL);
        click(&mut h, FAR);
        assert_eq!(joins(&h).len(), 1);
        assert_eq!(h.state().status.text, "These two aren't joined.", "{item}");
    }
}

#[test]
fn a_join_that_would_close_a_loop_is_refused_naming_the_loop() {
    // A third, a wall across the first and standing in the floor: the wall cuts the floor, the floor cuts the third, and
    // the third cutting the wall would close the loop.
    let mut h = app(&format!("{CROSSING}\nwall {{\"points\": [[3000, 1000], [3000, 3000]], \"height\": 3000}}"));
    let third = P2(3000.0, 1200.0);
    shortcut(&mut h, "JO");
    for (first, second) in [(WALL, FLOOR), (FLOOR, third)] {
        click(&mut h, first);
        click(&mut h, second);
    }
    assert_eq!(joins(&h).len(), 2, "{:?}", h.state().status);
    click(&mut h, third);
    click(&mut h, WALL);
    assert_eq!(joins(&h).len(), 2);
    let status = &h.state().status;
    assert!(status.error, "{status:?}");
    let (w, f) = wall_and_floor(&h);
    let x = ids(&h, "wall")[2];
    assert_eq!(
        status.text,
        format!(
            "That would close a loop of joins, each cutting the next: Wall {w} → Floor {f} → Wall {x} → Wall {w}. Join these two the other way round, or unjoin one of the loop first."
        )
    );
}

/// The Join button and its drop-down in a wide window and a narrow one, its three icons, the first picked with the prompt, and
/// a refusal in the status bar: saved where `BIMCRAFT_T23_DUMP` says (only then).
#[test]
fn join_geometry_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_T23_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    let save = |h: &mut Harness<BimApp>, name: &str| {
        h.run();
        h.run();
        h.render().unwrap().save(std::path::Path::new(&folder).join(format!("t23-join-{name}.png"))).unwrap();
    };
    for (w, name) in [(1920.0, "wide"), (1280.0, "narrow")] {
        let mut h = app(CROSSING);
        h.set_size(egui::vec2(w, 1000.0));
        h.run();
        topmost(&mut h, "Modify");
        save(&mut h, &format!("ribbon-{name}"));
        if h.query_by_label("Join").is_none() {
            press(&mut h, "Geometry ▾");
            save(&mut h, &format!("ribbon-{name}-list"));
        }
        press(&mut h, "Join");
        save(&mut h, &format!("dropdown-{name}"));
        press(&mut h, "Join Geometry");
        click(&mut h, WALL);
        h.hover_at(h.state().screen(FLOOR));
        save(&mut h, &format!("picked-{name}"));
        click(&mut h, FLOOR);
        for (item, icon) in [("Switch Join Order", "switch"), ("Unjoin Geometry", "unjoin")] {
            topmost(&mut h, "Modify");
            if h.query_by_label("Join").is_none() {
                press(&mut h, "Geometry ▾");
            }
            press(&mut h, "Join");
            press(&mut h, item);
            save(&mut h, &format!("{icon}-icon-{name}"));
        }
    }
    let mut h = app(CROSSING);
    h.set_size(egui::vec2(1920.0, 1000.0));
    shortcut(&mut h, "JO");
    click(&mut h, WALL);
    click(&mut h, FAR);
    save(&mut h, "refused");
}
