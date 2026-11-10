//! FE-13: Revit's keys in the family document. A family tab has a table of its own, Revit's default keys for the family editor's
//! commands, and never the project's: each key does what its ribbon button does in the family, where the button is enabled, and
//! nothing where it is greyed, and no project action runs from a family tab. A key made in Keyboard Shortcuts for one of the
//! family's commands works there too, as does one loaded from Revit's own export of its keyboard shortcuts.
use super::family_views::{editor, select_form, view};
use super::*;
use crate::adaptive_view::Draw;
use crate::dock::Palette;
use bimcraft_core::draw::Shape;
use bimcraft_core::family::DimKind;
use bimcraft_core::family::views::Which;

/// A box from Ref. Level up to `Height`, and a plane of its own facing -x, `Back`.
const FAMILY: &str = r#"family {"name": "Box", "category": "generic", "parameters": [{"name": "Width", "value": 400}, {"name": "Height", "value": 600}], "types": {"T": {}}, "forms": [{"profile": [[0, 0], ["Width", 0], ["Width", 300], [0, 300]], "from": 0, "to": "Height"}], "ref_planes": [{"name": "Back", "start": [-500, 2000, 0], "end": [-500, -2000, 0], "up": [0, 0, 1]}]}"#;

/// The project's four walls, one of them selected, and its family Box open in its document over them, in the floor plan.
fn open() -> Harness<'static, BimApp> {
    let mut h = app(&format!("{BOX}\n{FAMILY}"));
    h.set_size(vec2(1600.0, 1200.0));
    h.run();
    h.state_mut().edit_family("Box");
    h.run();
    let wall = ids(&h, "wall")[0];
    h.state_mut().selection = vec![wall];
    view(&mut h, Some(Which::Plan));
    h
}

fn editor_mut<'a>(h: &'a mut Harness<'_, BimApp>) -> &'a mut crate::editor::FamilyEditor {
    h.state_mut().active_family_mut().expect("a family open")
}

/// The form being sketched.
fn sketch<'a>(h: &'a Harness<'_, BimApp>) -> &'a crate::editor::sketch::FormSketch {
    editor(h).sketch.as_ref().expect("sketching")
}

/// What a key typed on the family's tab must leave of the project: no tool of its own, its four walls with one selected, and the
/// family's tab still the one shown.
fn project_untouched(h: &Harness<BimApp>, after: &str) {
    let app = h.state();
    assert!(app.active_family().is_some(), "{after}: the family's tab is still shown");
    assert_eq!(app.tool, Tool::Modify, "{after}: no project tool");
    assert_eq!(ids(h, "wall").len(), 4, "{after}: the project's walls");
    assert_eq!(app.selection.len(), 1, "{after}: the project's selection");
}

/// The number of forms in the family.
fn forms(h: &Harness<BimApp>) -> usize {
    editor(h).draft_family().unwrap().forms.len()
}

/// Whether a file dialog was begun, taking it.
fn dialog_begun() -> Option<std::sync::mpsc::Sender<Option<Vec<std::path::PathBuf>>>> {
    crate::dialog::BEGUN.with(|b| b.borrow_mut().pop())
}

/// The ribbon's button `label` pressed (a label the Properties may show too).
fn button(h: &mut Harness<BimApp>, label: &str) {
    h.get_by_role_and_label(Role::Button, label).click();
    h.run();
}

/// A rectangle drawn with Rectangle from `a` to `b` on the plan's work plane, while a form is sketched in the floor plan.
fn rectangle(h: &mut Harness<BimApp>, a: P2, b: P2) {
    button(h, "Rectangle");
    for q in [a, b] {
        let ed = editor(h);
        let frame = ed.plane_set().frame(&sketch(h).host).unwrap();
        let at = ed.views.screen_of(frame.point(q, 0.0)).expect("a view shown");
        click_at(h, at);
        h.run();
    }
}

/// A family file to load into Box: a smaller box.
fn nested_file(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("Small.json");
    let family = json!({"name": "Small", "category": "generic", "types": {"S": {}},
        "forms": [{"profile": [[0, 0], [100, 0], [100, 100], [0, 100]], "to": 100}]});
    std::fs::write(&path, family.to_string()).unwrap();
    path
}

#[test]
fn each_key_does_the_family_documents_command_and_never_the_projects() {
    let mut h = open();
    project_untouched(&h, "opened");
    // DI and DD the Aligned Dimension, AN the Angular; MD puts the tool down again, as Modify does.
    for (keys, kind) in [("DI", DimKind::Aligned), ("DD", DimKind::Aligned), ("AN", DimKind::Angular)] {
        shortcut(&mut h, keys);
        assert!(editor(&h).views.dimensioning.is_some(), "{keys}: the dimension tool");
        assert_eq!(editor(&h).views.dim_kind, kind, "{keys}");
        project_untouched(&h, keys);
        shortcut(&mut h, "MD");
        assert!(editor(&h).views.dimensioning.is_none(), "MD after {keys}");
    }
    for keys in ["AL", "AA"] {
        shortcut(&mut h, keys);
        assert!(editor(&h).views.aligning.is_some(), "{keys}: Align");
        project_untouched(&h, keys);
        shortcut(&mut h, "MD");
        assert!(editor(&h).views.aligning.is_none(), "MD after {keys}");
    }
    shortcut(&mut h, "RP");
    assert!(editor(&h).views.drawing.is_some(), "RP: Reference Plane");
    project_untouched(&h, "RP");
    shortcut(&mut h, "MD");
    assert!(editor(&h).views.drawing.is_none() && editor(&h).modeller.tool == Draw::Modify, "MD after RP");
    // TX: the family's own Text (Annotate > Detail), made and selected; DE takes it away, as the Delete key does.
    shortcut(&mut h, "TX");
    assert_eq!(editor(&h).selected_symbol(), Some(0), "TX: the family's Text");
    assert!(h.query_by_label("1. Text").is_some(), "the browser lists it");
    project_untouched(&h, "TX");
    shortcut(&mut h, "DE");
    assert!(editor(&h).draft_family().unwrap().symbols.is_empty(), "DE: the Text deleted");
    project_untouched(&h, "DE");
    select_form(&mut h, 1);
    shortcut(&mut h, "DE");
    assert_eq!(forms(&h), 0, "DE: the form deleted");
    project_untouched(&h, "DE on a form");
    // WP and WW: Work Plane's Set, the Work Plane dialog.
    for keys in ["WP", "WW"] {
        shortcut(&mut h, keys);
        assert!(editor(&h).views.dialog.is_some(), "{keys}: the Work Plane dialog");
        project_untouched(&h, keys);
        editor_mut(&mut h).views.dialog = None;
        h.run();
    }
    // PP, VP and Ctrl+1: the Properties palette hidden, and shown again.
    let shown = |h: &Harness<BimApp>| h.state().docking.shown(Palette::Properties);
    let was = shown(&h);
    shortcut(&mut h, "PP");
    assert_eq!(shown(&h), !was, "PP");
    shortcut(&mut h, "VP");
    assert_eq!(shown(&h), was, "VP");
    for _ in 0..2 {
        h.key_press_modifiers(Modifiers::COMMAND, Key::Num1);
        h.run();
        assert_ne!(shown(&h), was, "Ctrl+1");
        h.key_press_modifiers(Modifiers::COMMAND, Key::Num1);
        h.run();
        assert_eq!(shown(&h), was, "Ctrl+1 again");
    }
    project_untouched(&h, "PP");
    // KS: Keyboard Shortcuts, over the family.
    shortcut(&mut h, "KS");
    assert!(h.state().show_shortcuts, "KS");
    key(&mut h, Key::Escape);
    assert!(!h.state().show_shortcuts);
    project_untouched(&h, "KS");
    // CM is greyed until a family is loaded; FA is Load Family, its file dialog; then CM places the family loaded.
    shortcut(&mut h, "CM");
    assert!(editor(&h).placing.is_none(), "CM with no family loaded");
    let dir = tempfile::tempdir().unwrap();
    shortcut(&mut h, "FA");
    dialog_begun().expect("FA: Load Family's file dialog").send(Some(vec![nested_file(dir.path())])).unwrap();
    h.run();
    assert_eq!(editor(&h).nested.len(), 1, "FA: loaded into the family");
    project_untouched(&h, "FA");
    shortcut(&mut h, "CM");
    assert!(editor(&h).placing.is_some(), "CM: Component");
    project_untouched(&h, "CM");
    key(&mut h, Key::Escape);
    // 3D: the family's {3D}, not the project's.
    shortcut(&mut h, "3D");
    assert_eq!(editor(&h).views.shown, None, "3D: the family's {{3D}}");
    assert_ne!(h.state().sheet, Sheet::View3D);
    project_untouched(&h, "3D");
    // ZF, ZE, ZX and ZA: the view control bar's Zoom to Fit.
    let camera =
        bimcraft_core::view3d::Camera { azimuth: 1.0, elevation: 0.5, scale: 1.0, target: [0.0; 3], width: 400, height: 300, lens: None };
    for keys in ["ZF", "ZE", "ZX", "ZA"] {
        editor_mut(&mut h).modeller.camera = Some(camera);
        shortcut(&mut h, keys);
        assert!(editor(&h).modeller.camera.is_none(), "{keys}: Zoom to Fit");
        project_untouched(&h, keys);
    }
}

#[test]
fn finish_cancel_pick_lines_and_trim_work_while_a_form_is_sketched_and_do_nothing_otherwise() {
    let mut h = open();
    for keys in ["FG", "CG", "PL", "TR"] {
        shortcut(&mut h, keys);
        assert!(!editor(&h).sketching(), "{keys}");
        assert_eq!(forms(&h), 1, "{keys}");
        project_untouched(&h, keys);
    }
    button(&mut h, "Extrusion");
    assert!(editor(&h).sketching());
    shortcut(&mut h, "PL");
    assert_eq!(sketch(&h).tool, Some(Shape::PickLines), "PL: Pick Lines");
    shortcut(&mut h, "MD");
    assert_eq!(sketch(&h).tool, None, "MD: the sketch's Modify");
    shortcut(&mut h, "TR");
    assert!(sketch(&h).trim.is_some(), "TR: Trim/Extend to Corner");
    // What the sketch's ribbon doesn't offer does nothing: dimensions, Text, Load Family, Component.
    for keys in ["DI", "DD", "AN", "TX", "FA", "CM"] {
        shortcut(&mut h, keys);
        let ed = editor(&h);
        assert!(ed.sketching() && ed.views.dimensioning.is_none() && ed.selected_symbol().is_none(), "{keys} while sketching");
        assert!(dialog_begun().is_none(), "{keys}: no file dialog");
        project_untouched(&h, keys);
    }
    rectangle(&mut h, P2(-1000.0, -800.0), P2(-600.0, -500.0));
    assert_eq!(sketch(&h).sketch.lines.len(), 4);
    // DE: the lines picked, as the Delete key does.
    editor_mut(&mut h).sketch.as_mut().unwrap().sketch.picked.insert(0);
    shortcut(&mut h, "DE");
    assert_eq!(sketch(&h).sketch.lines.len(), 3, "DE: the line picked");
    shortcut(&mut h, "CG");
    assert!(!editor(&h).sketching(), "CG: Cancel Edit Mode");
    assert_eq!(forms(&h), 1);
    button(&mut h, "Extrusion");
    rectangle(&mut h, P2(-1000.0, -800.0), P2(-600.0, -500.0));
    shortcut(&mut h, "FG");
    assert!(!editor(&h).sketching(), "FG: Finish Edit Mode");
    assert_eq!(forms(&h), 2);
    project_untouched(&h, "FG");
}

#[test]
fn keys_whose_command_is_greyed_do_nothing() {
    let mut h = open();
    // {3D}: no plan or elevation to draw or dimension in, no family loaded to place, nothing to move and no sketch.
    view(&mut h, None);
    for keys in ["DI", "DD", "AN", "AL", "AA", "RP", "CM", "MV", "CO", "RO", "PL", "FG", "CG", "TR"] {
        shortcut(&mut h, keys);
        let ed = editor(&h);
        assert!(ed.views.dimensioning.is_none() && ed.views.aligning.is_none() && ed.views.drawing.is_none(), "{keys} in {{3D}}");
        assert!(ed.placing.is_none() && ed.modeller.tool == Draw::Modify && !ed.sketching(), "{keys} in {{3D}}");
        project_untouched(&h, keys);
    }
    // An adaptive family: no Text, Work Plane dialog, Component or dimensions; Move, Copy and Rotate once something is selected.
    let (mut h, points, _) = super::family_temporary_hide::adaptive();
    for keys in ["TX", "WP", "WW", "CM", "DI", "AN", "AL", "RP", "MV", "CO", "RO"] {
        shortcut(&mut h, keys);
        let ed = editor(&h);
        assert!(ed.selected_symbol().is_none() && ed.views.dialog.is_none() && ed.placing.is_none(), "{keys} adaptive");
        assert!(ed.views.dimensioning.is_none() && ed.views.aligning.is_none() && ed.views.drawing.is_none(), "{keys} adaptive");
        assert_eq!(ed.modeller.tool, Draw::Modify, "{keys}: nothing selected to move");
        assert_eq!(h.state().tool, Tool::Modify, "{keys}: no project tool");
    }
    editor_mut(&mut h).modeller.selection = vec![points[0].clone()];
    h.run();
    shortcut(&mut h, "MV");
    assert_eq!(editor(&h).modeller.tool, Draw::Move { from: None, copy: false }, "MV: Move");
    shortcut(&mut h, "MD");
    assert_eq!(editor(&h).modeller.tool, Draw::Modify, "MD");
    shortcut(&mut h, "CO");
    assert_eq!(editor(&h).modeller.tool, Draw::Move { from: None, copy: true }, "CO: Copy");
    shortcut(&mut h, "RO");
    assert!(matches!(editor(&h).modeller.tool, Draw::Rotate { start: None, .. }), "RO: Rotate");
    shortcut(&mut h, "MD");
    let nodes = |h: &Harness<BimApp>| editor(h).adaptive.as_ref().map_or(0, |d| d.nodes.len());
    let before = nodes(&h);
    shortcut(&mut h, "DE");
    assert!(nodes(&h) < before, "DE: the point selected deleted");
    assert_eq!(h.state().tool, Tool::Modify);
}

#[test]
fn the_projects_keys_are_unchanged_on_its_tab_with_a_family_open() {
    let mut h = open();
    press(&mut h, "Level 1");
    assert!(h.state().active_family().is_none(), "the project's tab");
    // The keys the family alone has do nothing in the project.
    for keys in ["FG", "CG", "PL", "DD", "AA", "AN", "WP", "WW", "FA", "ZX"] {
        h.state_mut().typed.clear();
        shortcut(&mut h, keys);
        assert_eq!(h.state().tool, Tool::Modify, "{keys}");
        assert!(h.state().active_family().is_none() && dialog_begun().is_none(), "{keys}");
        assert_eq!(h.state().selection.len(), 1, "{keys}");
    }
    // Find, which runs what it finds on the project, doesn't offer the family's own commands there.
    for name in crate::shortcuts::FAMILY_ONLY {
        assert!(h.state().find_matches(name).iter().all(|(label, ..)| label != name), "Find lists {name} on the project");
    }
    assert!(h.state().find_matches("Reference Plane").iter().any(|(label, ..)| label == "Reference Plane"), "the project's own");
    // Its own work as they did: TX the project's Text, MD Modify, AL Align.
    shortcut(&mut h, "TX");
    assert_eq!(h.state().tool, Tool::Text, "TX");
    shortcut(&mut h, "MD");
    assert_eq!(h.state().tool, Tool::Modify, "MD");
    shortcut(&mut h, "AL");
    assert!(matches!(h.state().tool, Tool::Align(None)), "AL");
    // Nothing was made in the family meanwhile.
    press(&mut h, "Box - Floor Plan: Ref. Level");
    assert!(editor(&h).selected_symbol().is_none() && editor(&h).views.aligning.is_none());
}

#[test]
fn a_key_made_in_keyboard_shortcuts_for_a_family_command_works_in_the_family() {
    let mut h = open();
    // Made on the family's tab: KS opens the dialog over it, and its list offers the family's commands.
    for (keys, action) in [("QF", "Finish Edit Mode"), ("QA", "Align"), ("QP", "Pick Lines")] {
        super::shortcuts_kept::add_in_the_dialog(&mut h, keys, action);
        key(&mut h, Key::Escape);
        assert!(!h.state().show_shortcuts);
        assert_eq!(crate::shortcuts::resolve_action(action), Some(action), "{action} is an action a key can be given");
    }
    shortcut(&mut h, "QA");
    assert!(editor(&h).views.aligning.is_some(), "QA: Align");
    shortcut(&mut h, "MD");
    button(&mut h, "Extrusion");
    shortcut(&mut h, "QP");
    assert_eq!(sketch(&h).tool, Some(Shape::PickLines), "QP: Pick Lines");
    rectangle(&mut h, P2(-1000.0, -800.0), P2(-600.0, -500.0));
    shortcut(&mut h, "QF");
    assert!(!editor(&h).sketching(), "QF: Finish Edit Mode");
    assert_eq!(forms(&h), 2);
    project_untouched(&h, "QF");
    // On the project's tab the family's commands do nothing.
    press(&mut h, "Level 1");
    shortcut(&mut h, "QF");
    shortcut(&mut h, "QP");
    assert_eq!(h.state().tool, Tool::Modify);
    assert_eq!(h.state().selection.len(), 1);
}

/// Revit's export of its keyboard shortcuts, as Revit writes `KeyboardShortcuts.xml`: every command, most of them without a key,
/// with the tabs and panels each is on; then some with keys of their own.
fn revit_export() -> String {
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n<Shortcuts>\n");
    for i in 0..900 {
        x += &format!(
            "  <ShortcutItem CommandName=\"Command &quot;{i}&quot;\" CommandId=\"ID_COMMAND_{i}\" Paths=\"Architecture&gt;Build; Massing &amp; Site&gt;Panel {i}\" />\n"
        );
    }
    for item in [
        r#"<ShortcutItem CommandName="Set" CommandId="ID_SETUP_WORKPLANE" Shortcuts="QW" Paths="Architecture&gt;Work Plane; Create&gt;Work Plane" />"#,
        r#"<ShortcutItem CommandName="Set" CommandId="ID_OTHER_SET" Shortcuts="QS" Paths="Manage&gt;Phasing" />"#,
        r#"<ShortcutItem CommandName="Finish Edit Mode" CommandId="ID_FINISH_SKETCH" Shortcuts="QF" Paths="Modify | Create Extrusion&gt;Mode" />"#,
        r#"<ShortcutItem CommandName="Mirror - Pick Axis" CommandId="ID_MIRROR_PICK_AXIS" Shortcuts="MM#QM" Paths="Modify&gt;Modify" />"#,
        r#"<ShortcutItem CommandName="Default 3D View" CommandId="ID_VIEW_DEFAULT_3DVIEW" Shortcuts="3D#Q3" Paths="Quick Access Toolbar; View&gt;Create&gt;3D View" />"#,
        r#"<ShortcutItem CommandName="Place a Component" CommandId="ID_OBJECTS_FAMSYM" Shortcuts="CM#QC" Paths="Architecture&gt;Build&gt;Component" />"#,
        r#"<ShortcutItem CommandName="Properties" CommandId="ID_TOGGLE_PROPERTIES_PALETTE" Shortcuts="PP#Ctrl+1#VP" Paths="View&gt;Windows&gt;User Interface" />"#,
    ] {
        x += &format!("  {item}\n");
    }
    x + "</Shortcuts>\n"
}

#[test]
fn a_revit_export_bigger_than_64_kib_binds_the_family_commands_it_names() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("KeyboardShortcuts.xml");
    std::fs::write(&file, revit_export()).unwrap();
    assert!(std::fs::metadata(&file).unwrap().len() > 64 << 10, "as big as Revit's");
    let mut h = open();
    h.state_mut().load_shortcuts_from_file(&file);
    assert_eq!(h.state().shortcuts_dialog.error, None);
    let custom = h.state().custom_shortcuts.clone();
    for (keys, action) in
        [("QW", "Work Plane"), ("QF", "Finish Edit Mode"), ("QM", "Mirror Pick Axis"), ("Q3", "3D View"), ("QC", "Component")]
    {
        assert_eq!(custom.get(keys).map(String::as_str), Some(action), "{keys}");
    }
    assert!(!custom.contains_key("QS"), "Set on another panel is not Work Plane's");
    // Its keys work in the family: QW the Work Plane dialog, Q3 the family's {3D}.
    shortcut(&mut h, "QW");
    assert!(editor(&h).views.dialog.is_some(), "QW: Work Plane");
    editor_mut(&mut h).views.dialog = None;
    h.run();
    shortcut(&mut h, "Q3");
    assert_eq!(editor(&h).views.shown, None, "Q3: the family's {{3D}}");
    project_untouched(&h, "Q3");
}
