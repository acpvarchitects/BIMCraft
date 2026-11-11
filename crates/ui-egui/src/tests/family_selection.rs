//! FE-20: the selection's commands in a family document, as Revit's family editor has them, over the one selection set
//! (FE-1). Filter (FF) keeps the kinds checked; Select Previous (PR) brings back the selection before it was let go or
//! replaced, without what was deleted since; Select by ID (ID) selects what a list of ids and GUIDs names and says what
//! names nothing, and IDs of Selection shows them; Select All Instances takes the nested instances of a selected one's
//! family and type, those shown (SA) or all (SV); Select Elements by Face (BF) says whether a face selects what it is of.
//! A key whose command has nothing to work on does nothing, each key can be given another in Keyboard Shortcuts, and
//! nothing of it reaches the project.
use super::family_box_select::{BOXES, DIM, EDGE, LINE, boxed, click_with_mods, g, plan, selected, set};
use super::family_views::{click_family, editor, view};
use super::*;
use crate::editor::select_box::Picked;
use bimcraft_core::family::Expr;
use bimcraft_core::family::views::{Side, Which};

fn editor_mut<'a>(h: &'a mut Harness<'_, BimApp>) -> &'a mut crate::editor::FamilyEditor {
    h.state_mut().active_family_mut().expect("a family open")
}

/// The project as a file holds it, and what it has selected and open: what the family document's commands must leave be.
fn project(h: &Harness<BimApp>) -> (String, Vec<Id>, bool) {
    let app = h.state();
    let dialogs = app.selection_filter.is_some() || app.select_by_id.is_some() || app.ids_of_selection.is_some();
    (app.session.model.to_json().unwrap(), app.selection.clone(), dialogs)
}

/// Both boxes, Edge, the reference line, the symbolic line and the dimension selected, by boxes as a hand draws them.
fn all_six(h: &mut Harness<BimApp>) {
    boxed(h, [400.0, 700.0, 0.0], [1000.0, 100.0, 0.0], Modifiers::NONE);
    boxed(h, [-400.0, 500.0, 0.0], [-1400.0, 300.0, 0.0], Modifiers::COMMAND);
    boxed(h, [-200.0, -600.0, 0.0], [200.0, -800.0, 0.0], Modifiers::COMMAND);
    boxed(h, [-300.0, 1000.0, 0.0], [300.0, 800.0, 0.0], Modifiers::COMMAND);
    boxed(h, [-1450.0, -1100.0, 0.0], [150.0, -1300.0, 0.0], Modifiers::COMMAND);
    assert_eq!(
        selected(h),
        set(vec![
            Picked::Form(0),
            Picked::Form(1),
            Picked::Plane(g(EDGE)),
            Picked::Line(g(LINE)),
            Picked::Dimension(g(DIM)),
            Picked::Symbol(0)
        ]),
        "both boxes, Edge, the reference line, the dimension and the symbol"
    );
}

/// The family document's Manage tab shown (the window's own bar has a Manage of its own in this profile).
fn manage(h: &mut Harness<BimApp>) {
    editor_mut(h).doc_tab = crate::editor::DocTab::Manage;
    h.run();
}

/// Whether the Filter dialog is open.
fn filtering(h: &Harness<BimApp>) -> bool {
    h.query_by_label("Check All").is_some()
}

#[test]
fn filter_counts_the_selection_by_kind_and_keeps_the_kinds_checked() {
    let mut h = plan();
    let before = project(&h);
    all_six(&mut h);
    // The Select panel's Filter, there while something is selected, as the project's.
    press(&mut h, "Filter");
    assert!(filtering(&h), "Filter opens its dialog");
    for kind in ["Extrusions", "Reference Planes", "Reference Lines", "Dimensions", "Symbols"] {
        assert!(h.query_by_role_and_label(Role::CheckBox, kind).is_some(), "{kind} listed");
    }
    assert!(h.query_by_role_and_label(Role::CheckBox, "Connectors").is_none(), "only the kinds selected");
    h.get_by_label("Total selected items: 6");
    // Each with its count: the two extrusions cleared leave four, Edge three.
    pick(&mut h, Role::CheckBox, "Extrusions");
    h.get_by_label("Total selected items: 4");
    pick(&mut h, Role::CheckBox, "Reference Planes");
    h.get_by_label("Total selected items: 3");
    press(&mut h, "OK");
    assert!(!filtering(&h));
    assert_eq!(
        selected(&h),
        set(vec![Picked::Line(g(LINE)), Picked::Dimension(g(DIM)), Picked::Symbol(0)]),
        "OK keeps what is checked selected"
    );
    // FF opens it too; Check None and Cancel leave the selection as it was.
    shortcut(&mut h, "FF");
    assert!(filtering(&h), "FF");
    h.get_by_label("Total selected items: 3");
    press(&mut h, "Check None");
    h.get_by_label("Total selected items: 0");
    press(&mut h, "Cancel");
    assert!(!filtering(&h));
    assert_eq!(selected(&h).len(), 3, "Cancel keeps the selection");
    // Check None, Check All, one cleared: OK keeps the others.
    shortcut(&mut h, "FF");
    press(&mut h, "Check None");
    press(&mut h, "Check All");
    h.get_by_label("Total selected items: 3");
    pick(&mut h, Role::CheckBox, "Symbols");
    press(&mut h, "OK");
    assert_eq!(selected(&h), set(vec![Picked::Line(g(LINE)), Picked::Dimension(g(DIM))]));
    // The status bar's funnel counts the family's selection and filters it, never the project's.
    assert!(painted_text(&h).contains(&":2".to_string()), "the status bar counts the family's selection");
    press(&mut h, "Filter Selection");
    assert!(filtering(&h), "the status bar's funnel");
    h.get_by_label("Total selected items: 2");
    press(&mut h, "Check None");
    press(&mut h, "OK");
    assert!(selected(&h).is_empty(), "nothing checked, nothing kept");
    assert_eq!(project(&h), before, "nothing of it reaches the project");
}

#[test]
fn ff_does_nothing_with_nothing_selected_and_none_of_the_keys_work_while_a_form_is_sketched() {
    let mut h = plan();
    let before = project(&h);
    assert!(selected(&h).is_empty());
    assert_eq!(h.query_all_by_label("Filter").count(), 0, "no Filter button with nothing selected");
    shortcut(&mut h, "FF");
    assert!(!filtering(&h), "FF with nothing selected");
    // The funnel, with nothing selected, opens nothing either.
    press(&mut h, "Filter Selection");
    assert!(!filtering(&h));
    // PR with no selection before this one, SA and SV with no nested instance selected: nothing.
    click_family(&mut h, [700.0, 400.0, 0.0]);
    assert_eq!(selected(&h), vec![Picked::Form(1)]);
    for keys in ["PR", "SA", "SV"] {
        shortcut(&mut h, keys);
        assert_eq!(selected(&h), vec![Picked::Form(1)], "{keys}");
    }
    // While a form is sketched, the selection's commands wait: its contextual tab has none of them.
    key(&mut h, Key::Escape);
    h.get_by_role_and_label(Role::Button, "Extrusion").click();
    h.run();
    assert!(editor(&h).sketching());
    for keys in ["FF", "ID", "PR", "SA", "SV"] {
        shortcut(&mut h, keys);
        assert!(!filtering(&h) && h.query_by_label("Add to the selection").is_none(), "{keys} while sketching");
        assert!(editor(&h).sketching(), "{keys}");
    }
    assert_eq!(project(&h), before, "nothing of it reaches the project");
}

#[test]
fn pr_brings_back_the_selection_before_this_one_without_what_was_deleted_since() {
    let mut h = plan();
    let before = project(&h);
    let three = set(vec![Picked::Form(0), Picked::Form(1), Picked::Plane(g(EDGE))]);
    boxed(&mut h, [400.0, 700.0, 0.0], [1000.0, 100.0, 0.0], Modifiers::NONE);
    boxed(&mut h, [-400.0, 500.0, 0.0], [-1400.0, 300.0, 0.0], Modifiers::COMMAND);
    assert_eq!(selected(&h), three);
    // Let go, it comes back.
    key(&mut h, Key::Escape);
    assert!(selected(&h).is_empty());
    shortcut(&mut h, "PR");
    assert_eq!(selected(&h), three, "PR after Escape");
    // Replaced by a click, it comes back, and what it replaced is the previous one: the two change places.
    click_family(&mut h, [-150.0, 900.0, 0.0]);
    let one = vec![Picked::Line(g(LINE))];
    assert_eq!(selected(&h), one, "the reference line alone");
    shortcut(&mut h, "PR");
    assert_eq!(selected(&h), three, "PR after another was selected");
    shortcut(&mut h, "PR");
    assert_eq!(selected(&h), one, "and back");
    shortcut(&mut h, "PR");
    assert_eq!(selected(&h), three);
    // Ctrl and Shift change the selection without making it another: PR still brings back the one before.
    click_with_mods(&mut h, [-700.0, 400.0, 0.0], Modifiers::SHIFT);
    assert_eq!(selected(&h), set(vec![Picked::Form(1), Picked::Plane(g(EDGE))]));
    shortcut(&mut h, "PR");
    assert_eq!(selected(&h), one, "the one before, not the three less one");
    shortcut(&mut h, "PR");
    assert_eq!(selected(&h), set(vec![Picked::Form(1), Picked::Plane(g(EDGE))]));
    // Deleted since: A, the first form, goes; PR brings back the others, B now the first form.
    click_with_mods(&mut h, [-700.0, 400.0, 0.0], Modifiers::COMMAND);
    assert_eq!(selected(&h), three);
    key(&mut h, Key::Escape);
    click_family(&mut h, [-700.0, 400.0, 0.0]);
    assert_eq!(selected(&h), vec![Picked::Form(0)], "A alone");
    key(&mut h, Key::Delete);
    let f = editor(&h).draft_family().unwrap();
    assert_eq!(f.forms.len(), 1, "A deleted");
    assert!(selected(&h).is_empty());
    shortcut(&mut h, "PR");
    assert_eq!(selected(&h), set(vec![Picked::Form(0), Picked::Plane(g(EDGE))]), "B, now the first form, and Edge: A is skipped");
    assert!((editor(&h).draft_family().unwrap().forms[0].corners().is_some()), "B is the form left");
    // Let go and both deleted from the browser's side of things: nothing is left to bring back, and PR does nothing.
    key(&mut h, Key::Escape);
    editor_mut(&mut h).remove_form(0);
    editor_mut(&mut h).delete_plane(g(EDGE)).unwrap();
    h.run();
    shortcut(&mut h, "PR");
    assert!(selected(&h).is_empty(), "nothing left to bring back");
    // The reference line selected and let go is the one PR brings back now.
    click_family(&mut h, [-150.0, 900.0, 0.0]);
    key(&mut h, Key::Escape);
    shortcut(&mut h, "PR");
    assert_eq!(selected(&h), one);
    assert_eq!(project(&h), before, "nothing of it reaches the project");
}

#[test]
fn id_selects_what_a_list_of_ids_and_guids_names_and_says_what_names_nothing() {
    let mut h = plan();
    let before = project(&h);
    let family = editor(&h).draft_family().unwrap();
    let (a, b) = (family.forms[0].id, family.forms[1].guid);
    assert!(a >= 100_000 && !b.is_nil(), "a form of a family the project has holds an id and a GUID: {a} {b}");
    let label = "IDs, GUIDs or IFC GlobalIds, as many as you like:";
    // Manage > Inquiry > Select by ID, as the project's.
    manage(&mut h);
    press(&mut h, "Select by ID");
    type_into(&mut h, label, &format!("{a}, {EDGE} 123"));
    assert!(h.query_by_label("Finds 2; nothing has these: 123").is_some(), "what it finds, and what names nothing, as typed");
    press(&mut h, "OK");
    assert_eq!(selected(&h), set(vec![Picked::Form(0), Picked::Plane(g(EDGE))]));
    assert!(!h.state().status.error && h.state().status.text.contains("123"), "{}", h.state().status.text);
    // ID opens it too. A list that names nothing says so, and OK waits; Cancel leaves the selection be.
    shortcut(&mut h, "ID");
    type_into(&mut h, label, "nowhere 99");
    assert!(h.query_by_label("Finds 0; nothing has these: nowhere, 99").is_some());
    h.get_by_label("OK").click();
    h.run();
    assert!(h.query_by_label("Add to the selection").is_some(), "OK waits while nothing is found");
    press(&mut h, "Cancel");
    assert_eq!(selected(&h).len(), 2);
    // Added to the selection: B by its GUID without its dashes, the dimension by its own.
    shortcut(&mut h, "ID");
    type_into(&mut h, label, &format!("{} {DIM}", b.to_string().replace('-', "")));
    assert!(h.query_by_label("Finds 2.").is_some());
    pick(&mut h, Role::CheckBox, "Add to the selection");
    press(&mut h, "OK");
    assert_eq!(selected(&h), set(vec![Picked::Form(0), Picked::Form(1), Picked::Plane(g(EDGE)), Picked::Dimension(g(DIM))]));
    // IDs of Selection, beside it: each one's id and GUID, to read and copy.
    manage(&mut h);
    press(&mut h, "IDs of Selection");
    for text in [a.to_string(), b.to_string(), EDGE.to_string(), "4 selected.".to_string(), "1. Extrusion".to_string()] {
        assert!(h.query_all_by_label(&text).count() > 0, "{text}");
    }
    press(&mut h, "Copy GUIDs");
    let copied = editor(&h).selecting.copied.clone().unwrap();
    assert!(copied.lines().count() == 4 && copied.contains(EDGE) && copied.contains(&b.to_string()), "{copied}");
    press(&mut h, "Copy IDs");
    assert!(editor(&h).selecting.copied.as_deref().is_some_and(|c| c.lines().any(|l| l == a.to_string())));
    press(&mut h, "Close");
    assert!(h.query_by_label("Copy GUIDs").is_none());
    // A plane drawn since has no id yet: a dash, and a note of it; it is found by its GUID.
    shortcut(&mut h, "RP");
    click_family(&mut h, [-1100.0, -1000.0, 0.0]);
    click_family(&mut h, [-1100.0, 1000.0, 0.0]);
    key(&mut h, Key::Escape);
    let drawn = editor(&h).planes.last().unwrap().guid;
    assert_eq!(selected(&h), vec![Picked::Plane(drawn)]);
    manage(&mut h);
    press(&mut h, "IDs of Selection");
    assert!(h.query_all_by_label("\u{2014}").count() > 0 && h.query_all_by_label(&drawn.to_string()).count() > 0, "no id yet, its GUID");
    press(&mut h, "Close");
    key(&mut h, Key::Escape);
    shortcut(&mut h, "ID");
    type_into(&mut h, label, &drawn.to_string());
    press(&mut h, "OK");
    assert_eq!(selected(&h), vec![Picked::Plane(drawn)]);
    assert_eq!(project(&h), before, "nothing of it reaches the project");
}

/// A small box of two types, S 100 wide and L 300, to nest.
fn small_file(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("Small.json");
    let family = json!({"name": "Small", "category": "generic", "parameters": [{"name": "W", "value": 100}],
        "types": {"S": {}, "L": {"W": 300}},
        "forms": [{"profile": [[0, 0], ["W", 0], ["W", 100], [0, 100]], "to": 100}]});
    std::fs::write(&path, family.to_string()).unwrap();
    path
}

/// An instance of Small's type `type_name` placed by Component at `at` of the floor plan.
fn place(h: &mut Harness<BimApp>, type_name: &str, at: [f64; 3]) {
    shortcut(h, "CM");
    assert!(editor(h).placing.is_some(), "Component");
    editor_mut(h).placing = Some(("Small".into(), type_name.into()));
    let pos = editor(h).views.screen_of(at).expect("the floor plan shows");
    click_at(h, pos);
    h.run();
}

#[test]
fn sa_and_sv_select_the_nested_instances_of_the_selected_ones_family_and_type() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = plan();
    let before = project(&h);
    let doc = h.state().active_doc().expect("a family document");
    h.state_mut().nest_family_file(doc, &small_file(dir.path()));
    h.run();
    assert_eq!(editor(&h).nested.len(), 1, "Small is loaded into the family");
    // Three of type S and one of L in a row in front of the boxes; the second S switched off.
    for (type_name, x) in [("S", 200.0), ("S", 600.0), ("S", 1000.0), ("L", 1400.0)] {
        place(&mut h, type_name, [x, -400.0, 0.0]);
    }
    assert_eq!(editor(&h).nested_instances.len(), 4);
    editor_mut(&mut h).nested_instances[1].visible = Some(Expr::Number(0.0));
    h.run();
    key(&mut h, Key::Escape);
    // One selected: SA takes those of its type the view shows, SV all of them.
    click_family(&mut h, [250.0, -350.0, 0.0]);
    assert_eq!(selected(&h), vec![Picked::Nested(0)]);
    shortcut(&mut h, "SA");
    assert_eq!(selected(&h), vec![Picked::Nested(0), Picked::Nested(2)], "SA: visible in view");
    shortcut(&mut h, "SV");
    assert_eq!(selected(&h), vec![Picked::Nested(0), Picked::Nested(1), Picked::Nested(2)], "SV: in entire project");
    // Another type is another: L alone.
    click_family(&mut h, [1500.0, -350.0, 0.0]);
    assert_eq!(selected(&h), vec![Picked::Nested(3)]);
    shortcut(&mut h, "SV");
    assert_eq!(selected(&h), vec![Picked::Nested(3)]);
    // With a form among them, the instances of the nested one's type are the selection, as the project's SA.
    click_with_mods(&mut h, [700.0, 400.0, 0.0], Modifiers::COMMAND);
    click_with_mods(&mut h, [1050.0, -350.0, 0.0], Modifiers::COMMAND);
    assert_eq!(selected(&h), set(vec![Picked::Form(1), Picked::Nested(2), Picked::Nested(3)]));
    shortcut(&mut h, "SA");
    assert_eq!(selected(&h), vec![Picked::Nested(0), Picked::Nested(2), Picked::Nested(3)], "of each type selected");
    // PR brings back what SA replaced.
    shortcut(&mut h, "PR");
    assert_eq!(selected(&h), set(vec![Picked::Form(1), Picked::Nested(2), Picked::Nested(3)]));
    // A form alone has no type: nothing.
    click_family(&mut h, [700.0, 400.0, 0.0]);
    shortcut(&mut h, "SA");
    shortcut(&mut h, "SV");
    assert_eq!(selected(&h), vec![Picked::Form(1)]);
    // The Project Browser's menu on a nested family's type asks the same of the type.
    editor_mut(&mut h).select_instances_of("Small", "S", true);
    assert_eq!(selected(&h), vec![Picked::Nested(0), Picked::Nested(2)]);
    editor_mut(&mut h).select_instances_of("Small", "S", false);
    assert_eq!(selected(&h), vec![Picked::Nested(0), Picked::Nested(1), Picked::Nested(2)]);
    // In {3D} and an elevation too: the keys are the document's, not a view's.
    for which in [None, Some(Which::Elevation(Side::Front))] {
        view(&mut h, which);
        editor_mut(&mut h).select_nested(Some(3));
        h.run();
        shortcut(&mut h, "SA");
        assert_eq!(selected(&h), vec![Picked::Nested(3)], "{which:?}");
        editor_mut(&mut h).select_nested(Some(2));
        h.run();
        shortcut(&mut h, "SV");
        assert_eq!(selected(&h).len(), 3, "{which:?}");
    }
    assert_eq!(project(&h), before, "nothing of it reaches the project");
}

#[test]
fn each_of_the_selections_keys_can_be_given_another_in_keyboard_shortcuts() {
    use crate::editor::keys::FAMILY_SHORTCUTS;
    let mut h = plan();
    let before = project(&h);
    for (keys, action) in [
        ("FF", "Filter Selection"),
        ("PR", "Select Previous"),
        ("ID", "Select by ID"),
        ("SA", "Select All Instances"),
        ("SV", "Select All Instances in Entire Project"),
        ("BF", "Select Elements by Face"),
    ] {
        assert!(FAMILY_SHORTCUTS.contains(&(keys, action)), "{keys} is {action} in the family's table");
        assert_eq!(crate::shortcuts::resolve_action(action), Some(action), "{action} is an action a key can be given");
    }
    // Made in the dialog over the family's tab: QF filters, QB brings the selection back, QI selects by ID.
    for (keys, action) in [("QF", "Filter Selection"), ("QB", "Select Previous"), ("QI", "Select by ID")] {
        super::shortcuts_kept::add_in_the_dialog(&mut h, keys, action);
        key(&mut h, Key::Escape);
        assert!(!h.state().show_shortcuts);
    }
    boxed(&mut h, [400.0, 700.0, 0.0], [1000.0, 100.0, 0.0], Modifiers::NONE);
    assert_eq!(selected(&h), vec![Picked::Form(1)]);
    shortcut(&mut h, "QF");
    assert!(filtering(&h), "QF: Filter");
    press(&mut h, "Cancel");
    key(&mut h, Key::Escape);
    assert!(selected(&h).is_empty());
    shortcut(&mut h, "QB");
    assert_eq!(selected(&h), vec![Picked::Form(1)], "QB: Select Previous");
    shortcut(&mut h, "QI");
    assert!(h.query_by_label("Add to the selection").is_some(), "QI: Select by ID");
    press(&mut h, "Cancel");
    // On the project's tab the family's commands do nothing, by their own keys or the ones made.
    press(&mut h, "Level 1");
    assert!(h.state().active_family().is_none(), "the project's tab");
    for keys in ["FF", "PR", "ID", "SV", "QF", "QB", "QI"] {
        h.state_mut().typed.clear();
        shortcut(&mut h, keys);
        assert_eq!(project(&h), before, "{keys} on the project's tab");
        assert_eq!(h.state().tool, Tool::Modify, "{keys}");
    }
    for name in ["Filter Selection", "Select Previous", "Select by ID", "Select All Instances in Entire Project"] {
        assert!(h.state().find_matches(name).iter().all(|(label, ..)| label != name), "Find lists {name} on the project");
    }
}

#[test]
fn the_selections_commands_are_not_an_adaptive_familys() {
    let (mut h, points, _) = super::family_temporary_hide::adaptive();
    editor_mut(&mut h).modeller.selection = vec![points[0].clone()];
    h.run();
    assert!(painted_text(&h).contains(&":1".to_string()), "the status bar counts the point selected");
    for keys in ["FF", "PR", "ID", "SA", "SV"] {
        shortcut(&mut h, keys);
        assert!(!filtering(&h) && h.query_by_label("Add to the selection").is_none(), "{keys} in an adaptive family");
        assert_eq!(editor(&h).modeller.selection.len(), 1, "{keys}");
        assert_eq!(h.state().tool, Tool::Modify, "{keys}: no project tool");
    }
    assert_eq!(h.query_all_by_label("Filter").count(), 0);
}

/// BF, Revit's Select Elements by Face: on, a face selects what it is of, by a click and by a crossing box drawn inside it;
/// off, only an edge of its outline does. A box is drawn from inside the face of what isn't selected, either way.
#[test]
fn select_elements_by_face_decides_whether_a_face_selects_by_a_click_and_by_a_crossing_box() {
    let mut h = plan();
    let before = project(&h);
    assert!(h.state().select_by_face, "on to begin with: a click has always taken a form by its face");
    // On: a click inside B selects it, and so does one on its edge.
    click_family(&mut h, [700.0, 400.0, 0.0]);
    assert_eq!(selected(&h), vec![Picked::Form(1)], "a click on its face");
    key(&mut h, Key::Escape);
    click_family(&mut h, [900.0, 400.0, 0.0]);
    assert_eq!(selected(&h), vec![Picked::Form(1)], "a click on its edge");
    key(&mut h, Key::Escape);
    // A crossing box drawn wholly inside A's face takes A; a window there holds none of it.
    boxed(&mut h, [-650.0, 450.0, 0.0], [-750.0, 350.0, 0.0], Modifiers::NONE);
    assert_eq!(selected(&h), vec![Picked::Form(0)], "a crossing inside its face takes it");
    boxed(&mut h, [650.0, 350.0, 0.0], [750.0, 450.0, 0.0], Modifiers::NONE);
    assert!(selected(&h).is_empty(), "a window inside a face holds nothing whole");
    // Ctrl adds what a crossing inside a face takes.
    boxed(&mut h, [-650.0, 450.0, 0.0], [-750.0, 350.0, 0.0], Modifiers::NONE);
    boxed(&mut h, [750.0, 450.0, 0.0], [650.0, 350.0, 0.0], Modifiers::COMMAND);
    assert_eq!(selected(&h), vec![Picked::Form(0), Picked::Form(1)], "Ctrl adds");
    // From the face of one selected no box is drawn: its drag is its own.
    boxed(&mut h, [750.0, 450.0, 0.0], [650.0, 350.0, 0.0], Modifiers::NONE);
    assert_eq!(selected(&h), vec![Picked::Form(0), Picked::Form(1)], "a drag from a selected form's face boxes nothing");
    // BF turns it off: a face selects nothing, by a click or by a crossing box; an edge still does.
    shortcut(&mut h, "BF");
    assert!(!h.state().select_by_face, "BF");
    key(&mut h, Key::Escape);
    click_family(&mut h, [700.0, 400.0, 0.0]);
    assert!(selected(&h).is_empty(), "off, a click on a face selects nothing");
    click_family(&mut h, [900.0, 400.0, 0.0]);
    assert_eq!(selected(&h), vec![Picked::Form(1)], "off, a click on an edge selects");
    boxed(&mut h, [-650.0, 450.0, 0.0], [-750.0, 350.0, 0.0], Modifiers::NONE);
    assert!(selected(&h).is_empty(), "off, a crossing inside a face takes nothing, and lets the selection go");
    boxed(&mut h, [-400.0, 500.0, 0.0], [-600.0, 300.0, 0.0], Modifiers::NONE);
    assert_eq!(selected(&h), vec![Picked::Form(0)], "off, a crossing over its edge takes it");
    // Off, the face of one selected is nothing a drag belongs to: a box is drawn from it.
    boxed(&mut h, [-650.0, 450.0, 0.0], [-750.0, 350.0, 0.0], Modifiers::NONE);
    assert!(selected(&h).is_empty(), "off, a drag from inside a selected form's face is a box");
    // The status bar's toggle turns it on again.
    press(&mut h, "Select Elements by Face");
    assert!(h.state().select_by_face, "the status bar's toggle");
    click_family(&mut h, [700.0, 400.0, 0.0]);
    assert_eq!(selected(&h), vec![Picked::Form(1)]);
    // An elevation picks as a plan does.
    view(&mut h, Some(Which::Elevation(Side::Front)));
    key(&mut h, Key::Escape);
    boxed(&mut h, [-650.0, 0.0, 350.0], [-750.0, 0.0, 250.0], Modifiers::NONE);
    assert_eq!(selected(&h), vec![Picked::Form(0)], "a crossing inside A's face in the Front elevation");
    shortcut(&mut h, "BF");
    click_family(&mut h, [700.0, 0.0, 300.0]);
    assert!(selected(&h).is_empty(), "off, in an elevation too");
    click_family(&mut h, [700.0, 0.0, 600.0]);
    assert_eq!(selected(&h), vec![Picked::Form(1)], "its top edge");
    assert_eq!(project(&h), before, "nothing of it reaches the project");
}

#[test]
fn select_elements_by_face_is_greyed_where_it_changes_nothing_and_is_kept_with_the_settings() {
    let dir = tempfile::tempdir().unwrap();
    let settings = dir.path().join("settings.json");
    std::fs::write(&settings, "{}").unwrap();
    let mut h = plan();
    h.state_mut().use_settings(settings.clone());
    h.run();
    assert!(h.state().select_by_face, "on, in a settings file that says nothing of it");
    // {3D} takes a form by its face either way, as the project's 3D view does: there the toggle is greyed and BF does nothing.
    view(&mut h, None);
    shortcut(&mut h, "BF");
    h.get_by_label("Select Elements by Face").click();
    h.run();
    assert!(h.state().select_by_face, "greyed in {{3D}}");
    // In a plan it is the window's, kept in the settings file and not in the family or the project.
    view(&mut h, Some(Which::Plan));
    shortcut(&mut h, "BF");
    assert!(!h.state().select_by_face);
    let kept: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(kept["select_by_face"], false, "{kept}");
    let mut again = app("");
    again.state_mut().use_settings(settings);
    assert!(!again.state().select_by_face, "the next session begins as this one ended");
    // While a form is sketched the view picks for the sketch: greyed.
    h.get_by_role_and_label(Role::Button, "Extrusion").click();
    h.run();
    assert!(editor(&h).sketching());
    shortcut(&mut h, "BF");
    assert!(!h.state().select_by_face, "greyed while sketching");
    // An adaptive family selects its own way: no toggle, and BF does nothing.
    let (mut h, ..) = super::family_temporary_hide::adaptive();
    let was = h.state().select_by_face;
    assert_eq!(h.query_all_by_label("Select Elements by Face").count(), 0, "no toggle in an adaptive family");
    shortcut(&mut h, "BF");
    assert_eq!(h.state().select_by_face, was);
    // On the project's tab neither: its views pick as they always have.
    let mut h = app(BOX);
    assert_eq!(h.query_all_by_label("Select Elements by Face").count(), 0, "no toggle on a project's tab");
    shortcut(&mut h, "BF");
    assert!(h.state().select_by_face);
}

/// Looking at the result: with `BIMCRAFT_FAMILY_SELECTION_DUMP` naming a folder, the family's plan with several selected
/// (the Select panel's Filter, the status bar's count, funnel and Select Elements by Face), Filter, Select by ID and IDs of
/// Selection, each in the three looks.
#[test]
fn the_selections_dialogs_and_status_bar_in_the_three_looks() {
    let Ok(folder) = std::env::var("BIMCRAFT_FAMILY_SELECTION_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    for (name, dark, light_canvas) in [("light", false, false), ("dark", true, false), ("dark-ui-light-canvas", true, true)] {
        let mut h = app(BOXES);
        h.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
        h.state_mut().light_canvas = light_canvas;
        h.set_size(vec2(1600., 1000.));
        h.state_mut().edit_family("Boxes");
        h.run();
        view(&mut h, Some(Which::Plan));
        let shot = |h: &mut Harness<BimApp>, what: &str| {
            h.run();
            h.render().unwrap().save(std::path::Path::new(&folder).join(format!("selection-{what}-{name}.png"))).unwrap();
        };
        all_six(&mut h);
        shot(&mut h, "several");
        shortcut(&mut h, "FF");
        shot(&mut h, "filter");
        press(&mut h, "Cancel");
        shortcut(&mut h, "BF");
        shortcut(&mut h, "ID");
        type_into(&mut h, "IDs, GUIDs or IFC GlobalIds, as many as you like:", &format!("{EDGE} 123"));
        shot(&mut h, "by-id-and-face-off");
        press(&mut h, "Cancel");
        manage(&mut h);
        press(&mut h, "IDs of Selection");
        shot(&mut h, "ids");
    }
}
