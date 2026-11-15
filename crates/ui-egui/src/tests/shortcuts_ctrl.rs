//! Shortcuts need Ctrl (Settings > User Interface). Off, the default: two plain letters are a shortcut, as in Revit, on the
//! project's tab and a family's alike, and Ctrl's own keys (Ctrl+Z, Ctrl+S…) act at once. On: shortcuts are typed with Ctrl
//! held and a Ctrl key of its own waits for a second letter. Either way Ctrl+C, Ctrl+X and Ctrl+V (which a real window gets
//! as its Copy, Cut and Paste) are the clipboard's, at once, and never letters of a shortcut. The setting is the user's, kept
//! in `settings.json`.
use super::family_drag_move::{family, pick as pick_in_family, plan, status};
use super::family_views::editor;
use super::*;
use crate::editor::clipboard::Command;
use crate::editor::modify::Tool as M;
use crate::editor::select_box::Picked;

/// One frame of `events`, as a window hands them over.
fn frame(h: &mut Harness<BimApp>, events: impl IntoIterator<Item = Event>) {
    h.input_mut().events.extend(events);
    h.step();
}

fn down(key: Key) -> Event {
    Event::Key { key, pressed: true, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None }
}

fn up(key: Key) -> Event {
    Event::Key { key, pressed: false, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None }
}

fn held() -> Event {
    Event::ModifiersChanged(Modifiers::COMMAND)
}

fn let_go() -> Event {
    Event::ModifiersChanged(Modifiers::NONE)
}

/// Plain letters, as a keyboard types them with no modifier.
fn plain(h: &mut Harness<BimApp>, letters: &str) {
    h.event(Event::Text(letters.into()));
    h.run();
}

fn family_tool(h: &Harness<BimApp>) -> Option<M> {
    editor(h).modify_in_hand()
}

#[test]
fn by_default_plain_letters_are_shortcuts_in_the_project_and_in_a_family() {
    let mut h = app(SAMPLE);
    assert!(!h.state().shortcuts_need_ctrl, "off at first");
    plain(&mut h, "WA");
    assert!(matches!(h.state().tool, Tool::Wall(_)), "{:?}", h.state().tool);
    plain(&mut h, "md");
    assert_eq!(h.state().tool, Tool::Modify);

    // A family's tab: its own keys, as plain letters.
    let mut h = plan();
    plain(&mut h, "RP");
    assert!(editor(&h).views.drawing.is_some(), "RP: Reference Plane");
    plain(&mut h, "MD");
    assert!(editor(&h).views.drawing.is_none(), "MD");
    plain(&mut h, "al");
    assert!(editor(&h).views.aligning.is_some(), "AL: Align");
}

#[test]
fn with_plain_letters_ctrl_z_undoes_at_once() {
    let mut h = app("");
    for y in [0, 3000] {
        h.state_mut().session.execute("wall", &json!({"points": [[0, y], [4000, y]]})).unwrap();
    }
    h.run();
    // Ctrl held, Z, and Ctrl still held: no wait.
    frame(&mut h, [held(), down(Key::Z)]);
    assert_eq!(ids(&h, "wall").len(), 1, "undone in the frame Z came");
    frame(&mut h, [up(Key::Z), let_go()]);
    h.run();
    assert_eq!(ids(&h, "wall").len(), 1, "and once");
}

#[test]
fn with_shortcuts_needing_ctrl_plain_letters_do_nothing_and_ctrl_held_letters_are_shortcuts() {
    let mut h = app(SAMPLE);
    h.state_mut().shortcuts_need_ctrl = true;
    plain(&mut h, "WA");
    assert_eq!(h.state().tool, Tool::Modify, "plain letters are no shortcut");
    frame(&mut h, [held(), down(Key::W)]);
    frame(&mut h, [down(Key::A)]);
    frame(&mut h, [up(Key::W), up(Key::A), let_go()]);
    h.run();
    assert!(matches!(h.state().tool, Tool::Wall(_)), "Ctrl+W, A: {:?}", h.state().tool);
}

#[test]
fn in_a_family_copy_cut_and_paste_act_at_once_in_both_modes_and_start_no_shortcut() {
    for ctrl in [false, true] {
        let mut h = plan();
        h.state_mut().shortcuts_need_ctrl = ctrl;
        pick_in_family(&mut h, &[Picked::Form(0)]);
        let forms = family(&h).forms.len();
        // Ctrl+C: copied in the frame it came, Ctrl still held, no letter waiting.
        frame(&mut h, [held(), Event::Copy]);
        assert!(h.state().clipboard.is_some(), "ctrl {ctrl}: copied at once: {}", status(&h));
        assert!(status(&h).contains("1 element"), "{}", status(&h));
        assert!(h.state().typed.is_empty(), "ctrl {ctrl}: no shortcut begun: {:?}", h.state().typed);
        // O next, with Ctrl still held (or as a plain letter): no CO, as C was no letter.
        if ctrl {
            frame(&mut h, [down(Key::O)]);
            frame(&mut h, [up(Key::O), up(Key::C), let_go()]);
        } else {
            frame(&mut h, [up(Key::C), let_go()]);
            plain(&mut h, "O");
        }
        h.run();
        assert_eq!(family_tool(&h), None, "ctrl {ctrl}: C and O are no Copy tool");
        // Ctrl+V with text on the system's clipboard: one paste at once, its V let go no second.
        frame(&mut h, [held(), Event::Paste("the user's text".into())]);
        assert!(editor(&h).pasting(), "ctrl {ctrl}: pasting at once: {}", status(&h));
        frame(&mut h, [up(Key::V), let_go()]);
        h.run();
        let at = editor(&h).views.screen_of([1300.0, 1100.0, 0.0]).expect("a view shown");
        click_at(&mut h, at);
        h.run();
        assert_eq!(family(&h).forms.len(), forms + 1, "ctrl {ctrl}: one copy: {}", status(&h));
        assert!(!editor(&h).pasting(), "and no second paste");
        // Ctrl+X on the copy: cut in the frame it came, one step.
        let steps = editor(&h).steps();
        frame(&mut h, [held(), Event::Cut]);
        assert_eq!(family(&h).forms.len(), forms, "ctrl {ctrl}: cut at once: {}", status(&h));
        assert!(!h.state().typed.contains('X'), "ctrl {ctrl}: no letter X: {:?}", h.state().typed);
        frame(&mut h, [up(Key::X), let_go()]);
        h.run();
        assert_eq!(editor(&h).steps(), steps + 1, "one step");
        assert_eq!(family(&h).forms.len(), forms);
    }
}

#[test]
fn ctrl_v_heard_only_as_its_key_let_go_pastes_once_in_both_modes() {
    for ctrl in [false, true] {
        let mut h = plan();
        h.state_mut().shortcuts_need_ctrl = ctrl;
        pick_in_family(&mut h, &[Picked::Form(0)]);
        h.state_mut().family_clipboard(Command::Copy);
        h.run();
        let forms = family(&h).forms.len();
        // Nothing on the system's clipboard: Ctrl+V's press comes as nothing, its V let go as a key.
        frame(&mut h, [held()]);
        frame(&mut h, [up(Key::V)]);
        assert!(editor(&h).pasting(), "ctrl {ctrl}: pasting: {}", status(&h));
        assert!(h.state().typed.is_empty(), "no letter V waiting");
        frame(&mut h, [let_go()]);
        h.run();
        let at = editor(&h).views.screen_of([1300.0, 1100.0, 0.0]).expect("a view shown");
        click_at(&mut h, at);
        h.run();
        assert_eq!(family(&h).forms.len(), forms + 1, "ctrl {ctrl}: one copy: {}", status(&h));
        assert!(!editor(&h).pasting(), "and no second paste");
    }
}

#[test]
fn in_the_project_copy_does_nothing_and_starts_no_shortcut() {
    for ctrl in [false, true] {
        let mut h = app(BOX);
        h.state_mut().shortcuts_need_ctrl = ctrl;
        let walls = ids(&h, "wall");
        h.state_mut().selection = vec![walls[0]];
        h.run();
        frame(&mut h, [held(), Event::Copy]);
        assert!(h.state().typed.is_empty(), "ctrl {ctrl}: no letter C: {:?}", h.state().typed);
        if ctrl {
            frame(&mut h, [down(Key::O)]);
            frame(&mut h, [up(Key::O), up(Key::C), let_go()]);
        } else {
            frame(&mut h, [up(Key::C), let_go()]);
            plain(&mut h, "O");
        }
        h.run();
        assert_eq!(h.state().tool, Tool::Modify, "ctrl {ctrl}: no Copy tool");
        assert_eq!(ids(&h, "wall"), walls, "nothing copied or cut");
        assert_eq!(h.state().selection, vec![walls[0]]);
        frame(&mut h, [held(), Event::Cut]);
        frame(&mut h, [held(), Event::Paste("the user's text".into())]);
        frame(&mut h, [up(Key::V), let_go()]);
        h.run();
        assert_eq!(ids(&h, "wall"), walls, "ctrl {ctrl}: Cut and Paste do nothing either");
        assert!(!h.state().typed.contains(&['C', 'X', 'V'][..]), "{:?}", h.state().typed);
    }
}

#[test]
fn with_plain_letters_co_and_mv_are_copy_and_move_in_the_project_and_a_family() {
    let mut h = app(BOX);
    let wall = ids(&h, "wall")[0];
    h.state_mut().selection = vec![wall];
    h.run();
    plain(&mut h, "CO");
    assert_eq!(h.state().tool, Tool::Copy(None), "CO");
    key(&mut h, Key::Escape);
    h.state_mut().selection = vec![wall];
    h.run();
    plain(&mut h, "MV");
    assert_eq!(h.state().tool, Tool::Move(None), "MV");

    let mut h = plan();
    pick_in_family(&mut h, &[Picked::Form(0)]);
    plain(&mut h, "CO");
    assert_eq!(family_tool(&h), Some(M::Move { copy: true, from: None }), "CO: Copy {}", status(&h));
    assert!(h.state().clipboard.is_none(), "nothing put on the clipboard");
    key(&mut h, Key::Escape);
    key(&mut h, Key::Escape);
    pick_in_family(&mut h, &[Picked::Form(0)]);
    plain(&mut h, "MV");
    assert_eq!(family_tool(&h), Some(M::Move { copy: false, from: None }), "MV: Move {}", status(&h));
}

#[test]
fn in_a_text_field_ctrl_c_copies_its_text_in_both_modes() {
    for ctrl in [false, true] {
        let mut h = app(SAMPLE);
        h.state_mut().shortcuts_need_ctrl = ctrl;
        h.state_mut().find = Some("wall".into());
        h.run();
        frame(&mut h, [held(), down(Key::A)]);
        frame(&mut h, [up(Key::A), Event::Copy]);
        let copied: Vec<_> = h
            .output()
            .platform_output
            .commands
            .iter()
            .filter_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(copied, vec!["wall".to_string()], "ctrl {ctrl}");
        frame(&mut h, [up(Key::C), let_go()]);
        h.run();
        assert!(h.state().typed.is_empty(), "ctrl {ctrl}: {:?}", h.state().typed);
        assert_eq!(h.state().tool, Tool::Modify);
    }
}

#[test]
fn shortcuts_need_ctrl_is_a_setting_kept_in_the_settings_file_and_off_in_an_older_one() {
    for (profile, build) in [("Rev", app as fn(&str) -> Harness<'static, BimApp>), ("BIMCraft", bimcraft)] {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        // A file written before the setting was: off.
        std::fs::write(&file, r#"{"theme": "light", "shortcuts": {"ZW": "Door"}}"#).unwrap();
        let mut h = build("");
        h.state_mut().use_settings(file.clone());
        h.run();
        assert!(!h.state().shortcuts_need_ctrl, "{profile}: an older file has plain letters");
        // File > Settings > User Interface's check box.
        h.state_mut().settings_page = Some(crate::settings::Page::Interface);
        h.run();
        h.get_by_role_and_label(Role::CheckBox, "Shortcuts need Ctrl").scroll_to_me();
        h.run();
        pick(&mut h, Role::CheckBox, "Shortcuts need Ctrl");
        assert!(h.state().shortcuts_need_ctrl, "{profile}");
        h.state_mut().settings_page = None;
        h.run();
        let kept: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(kept["shortcuts_need_ctrl"], json!(true), "{profile}: written");
        assert_eq!(kept["shortcuts"], json!({"ZW": "Door"}), "{profile}: the rest kept");
        // The next window has it.
        let mut again = build("");
        again.state_mut().use_settings(file.clone());
        again.run();
        assert!(again.state().shortcuts_need_ctrl, "{profile}: read back");
        plain(&mut again, "WA");
        assert_eq!(again.state().tool, Tool::Modify, "{profile}: plain letters do nothing");
    }
}

/// File > Settings > User Interface with the check box, in Light and Dark and both profiles, saved where
/// `BIMCRAFT_KEYS_CTRL_DUMP` says (only then).
#[test]
fn shortcuts_need_ctrl_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_KEYS_CTRL_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    for (profile, build) in [("rev", app as fn(&str) -> Harness<'static, BimApp>), ("bimcraft", bimcraft)] {
        for (look, theme) in [("light", egui::ThemePreference::Light), ("dark", egui::ThemePreference::Dark)] {
            let mut h = build(SAMPLE);
            h.set_size(vec2(1400.0, 900.0));
            h.ctx.set_theme(theme);
            h.state_mut().shortcuts_need_ctrl = look == "dark";
            h.state_mut().settings_page = Some(crate::settings::Page::Interface);
            h.run();
            h.get_by_role_and_label(Role::CheckBox, "Shortcuts need Ctrl").scroll_to_me();
            h.run();
            h.run();
            h.render().unwrap().save(std::path::Path::new(&folder).join(format!("settings-{profile}-{look}.png"))).unwrap();
        }
    }
}

const CLIP_HINT: &str = "Ctrl+C, Ctrl+X and Ctrl+V stay copy, cut and paste: rebind a shortcut that has C, X or V.";

#[test]
fn keyboard_shortcuts_says_with_ctrl_that_c_x_and_v_stay_the_clipboards() {
    for ctrl in [false, true] {
        let mut h = app(SAMPLE);
        h.state_mut().shortcuts_need_ctrl = ctrl;
        h.state_mut().show_shortcuts = true;
        h.run();
        assert_eq!(h.query_by_label(CLIP_HINT).is_some(), ctrl, "ctrl {ctrl}");
    }
}

/// Keyboard Shortcuts with Shortcuts need Ctrl on, its hint at the top, in the three looks, saved where
/// `BIMCRAFT_KEYS_CTRL_DUMP` says (only then).
#[test]
fn keyboard_shortcuts_clipboard_hint_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_KEYS_CTRL_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    for (name, dark, light_canvas) in [("light", false, false), ("dark", true, false), ("dark-ui-light-canvas", true, true)] {
        let mut h = app(SAMPLE);
        h.set_size(vec2(1400.0, 900.0));
        h.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
        h.state_mut().light_canvas = light_canvas;
        h.state_mut().shortcuts_need_ctrl = true;
        h.state_mut().show_shortcuts = true;
        h.run();
        h.run();
        h.render().unwrap().save(std::path::Path::new(&folder).join(format!("keyboard-shortcuts-ctrl-{name}.png"))).unwrap();
    }
}
