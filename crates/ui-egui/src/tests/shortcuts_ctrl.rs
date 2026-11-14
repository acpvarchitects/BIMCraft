//! Shortcuts need Ctrl (Settings > User Interface). Off, the default: two plain letters are a shortcut, as in Revit, on the
//! project's tab and a family's alike, and Ctrl's own keys (Ctrl+Z, Ctrl+S…) act at once. On: shortcuts are typed with Ctrl
//! held and a Ctrl key of its own waits for a second letter. A real window gets Ctrl+C, Ctrl+X and Ctrl+V as the window's
//! Copy, Cut and Paste, never as keys: with Ctrl held they are the letters C, X and V of a shortcut (CO, MV), and on their
//! own still copy, cut or paste, once. The setting is the user's, kept in `settings.json`.
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
fn with_ctrl_the_windows_copy_cut_and_paste_are_letters_of_a_shortcut_in_the_project() {
    let mut h = app(BOX);
    h.state_mut().shortcuts_need_ctrl = true;
    let wall = ids(&h, "wall")[0];
    h.state_mut().selection = vec![wall];
    h.run();
    // Ctrl+C comes as Copy, then O: Copy's tool.
    frame(&mut h, [held(), Event::Copy]);
    frame(&mut h, [down(Key::O)]);
    frame(&mut h, [up(Key::O), up(Key::C), let_go()]);
    h.run();
    assert_eq!(h.state().tool, Tool::Copy(None), "Ctrl+C, O");
    key(&mut h, Key::Escape);
    // M, then Ctrl+V as Paste (text on the system's clipboard): Move's tool.
    h.state_mut().selection = vec![wall];
    frame(&mut h, [held(), down(Key::M)]);
    frame(&mut h, [Event::Paste("the user's text".into())]);
    frame(&mut h, [up(Key::M), up(Key::V), let_go()]);
    h.run();
    assert_eq!(h.state().tool, Tool::Move(None), "Ctrl+M, V");
    assert!(h.state().typed.is_empty());
}

#[test]
fn with_ctrl_copy_then_o_is_the_familys_copy_and_copies_nothing() {
    let mut h = plan();
    h.state_mut().shortcuts_need_ctrl = true;
    pick_in_family(&mut h, &[Picked::Form(0)]);
    frame(&mut h, [held(), Event::Copy]);
    frame(&mut h, [down(Key::O)]);
    frame(&mut h, [up(Key::O), up(Key::C), let_go()]);
    h.run();
    assert_eq!(family_tool(&h), Some(M::Move { copy: true, from: None }), "CO: Copy {}", status(&h));
    assert!(h.state().clipboard.is_none(), "nothing put on the clipboard");
}

#[test]
fn with_ctrl_m_then_paste_is_the_familys_move_and_pastes_nothing() {
    let mut h = plan();
    h.state_mut().shortcuts_need_ctrl = true;
    pick_in_family(&mut h, &[Picked::Form(0)]);
    // Something on the clipboard, so a paste could happen.
    h.state_mut().family_clipboard(Command::Copy);
    h.run();
    assert!(h.state().clipboard.is_some());
    let forms = family(&h).forms.len();
    // Text on the system's clipboard: V comes as Paste, then its V let go.
    frame(&mut h, [held(), down(Key::M)]);
    frame(&mut h, [Event::Paste("the user's text".into())]);
    frame(&mut h, [up(Key::M), up(Key::V), let_go()]);
    h.run();
    assert_eq!(family_tool(&h), Some(M::Move { copy: false, from: None }), "MV: Move {}", status(&h));
    assert!(!editor(&h).pasting(), "no paste follows the pointer");
    assert_eq!(family(&h).forms.len(), forms);
    key(&mut h, Key::Escape);
    key(&mut h, Key::Escape);
    pick_in_family(&mut h, &[Picked::Form(0)]);
    // Nothing on the system's clipboard: V comes as nothing but its V let go, and is V all the same.
    frame(&mut h, [held(), down(Key::M)]);
    frame(&mut h, [up(Key::M)]);
    frame(&mut h, [up(Key::V), let_go()]);
    h.run();
    assert_eq!(family_tool(&h), Some(M::Move { copy: false, from: None }), "MV: Move {}", status(&h));
    assert!(!editor(&h).pasting(), "no paste follows the pointer");
}

#[test]
fn with_ctrl_the_windows_copy_cut_and_paste_on_their_own_still_do_their_job_once() {
    let mut h = plan();
    h.state_mut().shortcuts_need_ctrl = true;
    pick_in_family(&mut h, &[Picked::Form(0)]);
    let forms = family(&h).forms.len();
    // Ctrl+C: Copy, waiting for a second letter while Ctrl is held, done when it is let go.
    frame(&mut h, [held(), Event::Copy]);
    assert!(h.state().clipboard.is_none(), "C waits for a second letter");
    frame(&mut h, [up(Key::C), let_go()]);
    h.run();
    assert!(h.state().clipboard.is_some(), "copied: {}", status(&h));
    assert!(status(&h).contains("1 element"), "{}", status(&h));
    // Ctrl+V with text on the system's clipboard: one paste, its V let go no second.
    frame(&mut h, [held(), Event::Paste("the user's text".into())]);
    frame(&mut h, [up(Key::V), let_go()]);
    h.run();
    assert!(editor(&h).pasting(), "{}", status(&h));
    let at = editor(&h).views.screen_of([1300.0, 1100.0, 0.0]).expect("a view shown");
    click_at(&mut h, at);
    h.run();
    assert_eq!(family(&h).forms.len(), forms + 1, "one copy: {}", status(&h));
    assert!(!editor(&h).pasting(), "and no second paste");
    // Ctrl+X on the copy: Cut, one step, once Ctrl is let go; with Ctrl still held past the wait, as well.
    let steps = editor(&h).steps();
    frame(&mut h, [held(), Event::Cut]);
    h.run_steps(3);
    assert_eq!(family(&h).forms.len(), forms, "cut after the wait: {}", status(&h));
    frame(&mut h, [up(Key::X), let_go()]);
    h.run();
    assert_eq!(editor(&h).steps(), steps + 1, "one step");
    assert_eq!(family(&h).forms.len(), forms);
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
