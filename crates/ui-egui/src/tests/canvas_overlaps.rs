//! UI-3: what floats over a view keeps clear of what the view draws and of each other: a plane's name of an elevation mark,
//! the drawing of the view-control bar and of the Options Bar's row, the Options Bar's card of nothing, the refusal card of
//! the view-control bar.
use super::family_views::view;
use super::*;
use bimcraft_core::family::views::{Side, Which};

/// A wall and a floor through its foot.
const WALLS: &str = r#"wall {"points": [[0, 2000], [6000, 2000]], "height": 3000}
floor {"boundary": [[0, 0], [6000, 0], [6000, 4000], [0, 4000]], "offset": 300}"#;

#[test]
fn a_tool_with_nothing_on_the_options_bar_shows_no_card() {
    let mut h = app(WALLS);
    h.set_size(egui::vec2(1600.0, 1000.0));
    h.run();
    for (panel, tool) in [("Join", "Join Geometry"), ("Cut", "Cut Geometry")] {
        topmost(&mut h, "Modify");
        press(&mut h, panel);
        press(&mut h, tool);
        for _ in 0..2 {
            h.run();
        }
        assert_eq!(h.state().options_rect, egui::Rect::NOTHING, "{tool}: no card of nothing");
        key(&mut h, egui::Key::Escape);
    }
    // A tool with options still has its card.
    press(&mut h, "Architecture");
    press(&mut h, "Wall");
    for _ in 0..2 {
        h.run();
    }
    assert!(h.state().options_rect.width() > 50.0, "Wall's options: {:?}", h.state().options_rect);
}

/// Before and after: the new Generic family's views, a tool's Options Bar and a refusal, and a project's Join and Cut Geometry,
/// when `BIMCRAFT_UI3_DUMP` names a folder.
#[test]
fn ui3_screenshots() {
    let Ok(dir) = std::env::var("BIMCRAFT_UI3_DUMP") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    for (prof, own) in [("bimcraft", true), ("rev", false)] {
        for (w, hgt) in [(1600.0, 1000.0), (1400.0, 900.0)] {
            let mut h = if own { bimcraft("") } else { app("") };
            h.set_size(egui::vec2(w, hgt));
            h.run();
            new_family_of(&mut h, "Generic");
            for _ in 0..3 {
                h.run();
            }
            let shot = |h: &mut Harness<BimApp>, name: &str| {
                for _ in 0..3 {
                    h.run();
                }
                h.render().unwrap().save(std::path::Path::new(&dir).join(format!("{prof}-{w}-{name}.png"))).unwrap();
            };
            for (name, which) in [
                ("plan", Some(Which::Plan)),
                ("front", Some(Which::Elevation(Side::Front))),
                ("right", Some(Which::Elevation(Side::Right))),
                ("3d", None),
            ] {
                view(&mut h, which);
                shot(&mut h, name);
            }
            // A tool's Options Bar in an elevation, a refusal in a plan and in {3D}.
            view(&mut h, Some(Which::Elevation(Side::Front)));
            shortcut(&mut h, "RP");
            shot(&mut h, "front-rp");
            key(&mut h, egui::Key::Escape);
            key(&mut h, egui::Key::Escape);
            view(&mut h, Some(Which::Plan));
            shortcut(&mut h, "TR");
            shot(&mut h, "plan-refusal");
            view(&mut h, None);
            shortcut(&mut h, "TR");
            shot(&mut h, "3d-refusal");
        }
    }
}

#[test]
fn ui3_project_join_screenshots() {
    let Ok(dir) = std::env::var("BIMCRAFT_UI3_DUMP") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    for (prof, own) in [("rev", false)] {
        let mut h = if own { bimcraft(WALLS) } else { app(WALLS) };
        h.set_size(egui::vec2(1600.0, 1000.0));
        h.run();
        for label in ["Join Geometry", "Cut Geometry"] {
            topmost(&mut h, "Modify");
            press(&mut h, if label == "Join Geometry" { "Join" } else { "Cut" });
            press(&mut h, label);
            for _ in 0..3 {
                h.run();
            }
            let name = label.replace(' ', "-");
            h.render().unwrap().save(std::path::Path::new(&dir).join(format!("{prof}-project-{name}.png"))).unwrap();
            key(&mut h, egui::Key::Escape);
        }
    }
}
