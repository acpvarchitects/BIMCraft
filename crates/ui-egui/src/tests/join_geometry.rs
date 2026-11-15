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
    // A join shows in a section and in 3D, not in the plan: the status bar says what was done.
    let said = |h: &Harness<BimApp>| (h.state().status.text.clone(), h.state().status.error);
    assert_eq!(said(&h), (format!("Joined: Wall {w} cuts Floor {f}."), false));
    // The tool goes on, for the next pair.
    assert_eq!(h.state().tool, Tool::JoinGeometry { mode: JoinMode::Join, first: None });
    // Switch Join Order, the pair picked either way round: the cutter first, then the one it cuts first.
    press(&mut h, "Join");
    press(&mut h, "Switch Join Order");
    click(&mut h, WALL);
    click(&mut h, FLOOR);
    assert_eq!(joins(&h), [(f, w)], "{:?}", h.state().status);
    assert_eq!(said(&h), (format!("Switched: Floor {f} now cuts Wall {w}."), false));
    click(&mut h, WALL);
    click(&mut h, FLOOR);
    assert_eq!(joins(&h), [(w, f)], "{:?}", h.state().status);
    assert_eq!(said(&h), (format!("Switched: Wall {w} now cuts Floor {f}."), false));
    // Unjoin Geometry, the one that is cut picked first.
    press(&mut h, "Join");
    press(&mut h, "Unjoin Geometry");
    click(&mut h, FLOOR);
    click(&mut h, WALL);
    assert_eq!(joins(&h), [], "{:?}", h.state().status);
    assert_eq!(said(&h), (format!("Unjoined: Floor {f} and Wall {w}."), false));
    // One undo step each.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(joins(&h), [(w, f)]);
    // And Unjoin the other way round, the cutter first.
    click(&mut h, WALL);
    click(&mut h, FLOOR);
    assert_eq!(joins(&h), [], "{:?}", h.state().status);
}

/// The colour drawn at model point `p`, with the cursor off the canvas's elements.
fn ink(h: &mut Harness<BimApp>, p: P2) -> [u8; 4] {
    h.hover_at(h.state().screen(P2(9000.0, 6000.0)));
    h.run();
    h.run();
    let at = h.state().screen(p);
    h.render().unwrap().get_pixel(at.x.round() as u32, at.y.round() as u32).0
}

#[test]
fn the_first_pick_stays_lit_until_esc_or_the_second() {
    let mut h = app(CROSSING);
    let plain = ink(&mut h, WALL);
    shortcut(&mut h, "JO");
    assert_eq!(ink(&mut h, WALL), plain, "the tool alone lights nothing");
    click(&mut h, WALL);
    let lit = ink(&mut h, WALL);
    assert_ne!(lit, plain, "the first pick is lit");
    // As what is selected is.
    key(&mut h, Key::Escape);
    assert_eq!(ink(&mut h, WALL), plain, "Esc lets go of it");
    click(&mut h, WALL);
    click(&mut h, FLOOR);
    assert_eq!(ink(&mut h, WALL), plain, "the pair joined, nothing is held");
    key(&mut h, Key::Escape);
    click(&mut h, WALL);
    assert_eq!(ink(&mut h, WALL), lit, "selected, it is drawn as the first pick was");
}

/// A room's four walls and its floor drawn on their centre lines, through their feet: at every edge of the floor a wall is
/// drawn too.
const ROOM: &str = r#"wall {"points": [[0,0],[8000,0],[8000,6000],[0,6000]], "closed": true, "height": 3000}
floor {"boundary": [[0, 0], [8000, 0], [8000, 6000], [0, 6000]], "offset": 300}"#;

#[test]
fn a_floor_on_its_walls_centre_lines_is_picked_after_its_wall_and_tab_picks_it_first() {
    let mut h = app(ROOM);
    let f = ids(&h, "floor")[0];
    let m = h.state().session.model.clone();
    // The south wall, whichever way it was drawn.
    let south = ids(&h, "wall").into_iter().find(|id| m.wall(*id).is_some_and(|w| w.start.1 == 0.0 && w.end.1 == 0.0)).unwrap();
    let edge = P2(4000.0, 0.0);
    shortcut(&mut h, "JO");
    // The wall is what the plan draws nearest; the second pick, at the same place, takes the other kind there: the floor.
    click(&mut h, edge);
    assert_eq!(h.state().tool, Tool::JoinGeometry { mode: JoinMode::Join, first: Some(south) });
    assert!(h.state().prompt().contains("Tab"), "{}", h.state().prompt());
    click(&mut h, edge);
    assert_eq!(joins(&h), [(south, f)], "{:?}", h.state().status);
    // Unjoined again; then the floor first: Tab steps on from the wall to it.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(joins(&h), []);
    h.hover_at(h.state().screen(edge));
    h.run();
    key(&mut h, Key::Tab);
    click(&mut h, edge);
    assert_eq!(h.state().tool, Tool::JoinGeometry { mode: JoinMode::Join, first: Some(f) });
    click(&mut h, edge);
    assert_eq!(joins(&h), [(f, south)], "{:?}", h.state().status);
    // At a corner two walls and the floor are under the cursor: with a wall first, the floor, and Tab steps to the other wall.
    let corner = P2(0.0, 0.0);
    click(&mut h, P2(0.0, 3000.0));
    let west = match h.state().tool {
        Tool::JoinGeometry { first: Some(w), .. } => w,
        ref tool => panic!("{tool:?}"),
    };
    assert!(west != south && west != f);
    h.hover_at(h.state().screen(corner));
    h.run();
    key(&mut h, Key::Tab);
    click(&mut h, corner);
    assert!(joins(&h).contains(&(west, south)), "{:?} {:?}", joins(&h), h.state().status);
}

#[test]
fn the_prompt_names_what_the_plan_shows_and_a_first_pick_is_this_plans() {
    let mut h = app(CROSSING);
    shortcut(&mut h, "JO");
    assert_eq!(h.state().prompt(), "Pick the first to join: a wall, a floor or a roof. Tab steps through what is under the cursor.");
    click(&mut h, WALL);
    assert!(matches!(h.state().tool, Tool::JoinGeometry { first: Some(_), .. }));
    // Another plan: the pick was the first one's, and is let go of.
    let level = match h.state().sheet {
        Sheet::Plan(level) => level,
        sheet => panic!("{sheet:?}"),
    };
    h.state_mut().open_sheet(Sheet::Ceiling(level));
    h.run();
    assert_eq!(h.state().tool, Tool::JoinGeometry { mode: JoinMode::Join, first: None });
    // A ceiling plan shows walls and ceilings, not floors.
    assert_eq!(h.state().prompt(), "Pick the first to join: a wall or a ceiling. Tab steps through what is under the cursor.");
}

#[test]
fn a_first_pick_that_is_gone_is_let_go_of_for_join_and_for_cut() {
    for letters in ["JO", "CU"] {
        let mut h = app(CROSSING);
        let w = ids(&h, "wall")[0];
        shortcut(&mut h, letters);
        click(&mut h, WALL);
        assert!(
            matches!(h.state().tool, Tool::JoinGeometry { first: Some(_), .. } | Tool::CutGeometry { wall: Some(_), .. }),
            "{letters}: {:?}",
            h.state().tool
        );
        // The wall goes, as an undo of its making takes it.
        h.state_mut().session.execute("delete", &json!({"ids": [w]})).unwrap();
        h.run();
        assert!(
            matches!(h.state().tool, Tool::JoinGeometry { first: None, .. } | Tool::CutGeometry { wall: None, .. }),
            "{letters}: {:?}",
            h.state().tool
        );
        // The next click is a first pick again, not a second with nothing before it.
        click(&mut h, FAR);
        assert!(!h.state().status.error, "{letters}: {:?}", h.state().status);
    }
}

#[test]
fn a_join_onto_one_cut_by_as_many_as_may_be_is_refused_in_the_windows_words() {
    // A hundred walls far off, each named in the floor's `joined_by`: the most one may have.
    let most = bimcraft_core::model::MAX_JOINS;
    let far: String = (0..most).map(|i| format!("\nwall {{\"points\": [[0, {y}], [1000, {y}]]}}", y = 20000 + 500 * i)).collect();
    let mut h = app(&format!("{CROSSING}{far}"));
    let (_, f) = wall_and_floor(&h);
    let m = h.state_mut().session.model_mut();
    let cutters: Vec<Guid> = m.elements.iter().filter(|(_, e)| e.category() == "wall").skip(2).map(|(id, _)| m.guid(*id)).collect();
    assert_eq!(cutters.len(), most);
    *m.elements.get_mut(f).and_then(Element::joined_by_mut).unwrap() = cutters;
    h.run();
    shortcut(&mut h, "JO");
    click(&mut h, WALL);
    click(&mut h, FLOOR);
    assert_eq!(joins(&h).len(), most);
    let status = &h.state().status;
    assert!(status.error, "{status:?}");
    assert_eq!(status.text, "The one to cut is joined to 100 that cut it already, the most there may be. Unjoin one of them first.");
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
/// what the status bar says of a join made, a refusal there, and the ribbon in Italian: saved where `BIMCRAFT_T23_DUMP` says
/// (only then).
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
        save(&mut h, &format!("joined-{name}"));
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
    // In Italian: the button is geometry's Join, not a session's.
    key(&mut h, Key::Escape);
    key(&mut h, Key::Escape);
    h.state_mut().lang = crate::lang::Lang::Italian;
    h.run();
    topmost(&mut h, "Modifica");
    h.run();
    save(&mut h, "italian");
}
