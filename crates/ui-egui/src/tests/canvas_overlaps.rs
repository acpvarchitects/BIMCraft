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

#[test]
fn the_refusal_card_stands_above_the_view_control_bar() {
    use super::family_views::view;
    for (own, size) in [(true, (1400.0, 900.0)), (false, (1600.0, 1000.0)), (false, (1100.0, 800.0))] {
        let mut h = if own { bimcraft("") } else { app("") };
        h.set_size(egui::vec2(size.0, size.1));
        h.run();
        new_family_of(&mut h, "Generic");
        for which in [Some(Which::Plan), None] {
            view(&mut h, which);
            h.run();
            shortcut(&mut h, "TR");
            for _ in 0..3 {
                h.run();
            }
            assert!(h.state().status.error, "a refusal");
            let card = h.get_by_label("Dismiss").rect();
            let said = h.get_by_label(&h.state().status.text.clone()).rect();
            // The family's view-control bar's card, over the view's foot.
            let bar = h.state().family_bar_top;
            assert!(bar.is_finite(), "the view-control bar drawn");
            assert!(
                said.bottom().max(card.bottom()) < bar - 4.0,
                "{own} {size:?} {which:?}: the card down to {} over the bar at {bar}",
                said.bottom()
            );
            h.state_mut().status = crate::Status::default();
        }
    }
}

#[test]
fn a_family_view_is_drawn_between_the_options_bar_row_and_the_view_control_bar_and_stays_put() {
    use super::family_views::{editor, view};
    for (own, size) in [(true, (1400.0, 900.0)), (false, (1600.0, 1000.0))] {
        let mut h = if own { bimcraft("") } else { app("") };
        h.set_size(egui::vec2(size.0, size.1));
        h.run();
        new_family_of(&mut h, "Generic");
        for which in [Some(Which::Plan), Some(Which::Elevation(Side::Front)), None] {
            view(&mut h, which);
            for _ in 0..3 {
                h.run();
            }
            let drawn = editor(&h).views.drawn.expect("drawn");
            let bar = h.state().family_bar_top;
            assert!(drawn.bottom() <= bar - 2.0, "{own} {which:?}: drawn down to {} over the bar at {bar}", drawn.bottom());
            // A tool's card on the Options Bar's row, over none of the drawing; the view as it was.
            if which.is_some() {
                shortcut(&mut h, "RP");
                for _ in 0..3 {
                    h.run();
                }
                let card = h.state().options_rect;
                assert!(card.height() > 10.0, "{which:?}: Reference Plane's card");
                let now = editor(&h).views.drawn.expect("drawn");
                assert!(
                    card.bottom() <= now.top() + 0.5,
                    "{own} {which:?}: the card down to {} over the drawing from {}",
                    card.bottom(),
                    now.top()
                );
                assert_eq!(now, drawn, "{own} {which:?}: the view stays put as the card comes");
                key(&mut h, egui::Key::Escape);
                key(&mut h, egui::Key::Escape);
            }
        }
    }
}

#[test]
fn a_family_view_in_a_window_too_small_for_it_draws_without_a_panic() {
    use super::family_views::view;
    let mut h = app("");
    h.set_size(egui::vec2(1400.0, 900.0));
    h.run();
    new_family_of(&mut h, "Generic");
    for which in [Some(Which::Plan), None] {
        view(&mut h, which);
        for (w, ht) in [(300.0, 50.0), (300.0, 200.0), (420.0, 280.0), (1400.0, 900.0)] {
            h.set_size(egui::vec2(w, ht));
            for _ in 0..3 {
                h.run();
            }
        }
    }
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
