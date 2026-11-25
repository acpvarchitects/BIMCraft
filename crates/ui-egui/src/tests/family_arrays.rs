//! T17: arrays use the family document's native tools and history.
use super::family_drag_move::{ed, family, near, pick, plan, points, status};
use super::family_views::editor;
use super::*;
use crate::editor::select_box::Picked;
use bimcraft_core::family::arrays::MoveTo;
fn click_family(h: &mut Harness<BimApp>, p: [f64; 2]) {
    let at = editor(h).views.screen_of([p[0], p[1], 0.]).unwrap();
    click_at(h, at);
}
#[test]
fn family_array_two_clicks_last_member_count_changes_seed_edits_and_undo_redo() {
    let mut h = plan();
    h.set_size(vec2(1800., 1000.));
    pick(&mut h, &[Picked::Form(0)]);
    let original = family(&h);
    press(&mut h, "Array");
    ed(&mut h).array_options.number = "3".into();
    ed(&mut h).array_options.move_to = MoveTo::Last;
    h.run();
    assert!(h.query_by_label("Group and Associate").is_some());
    assert!(h.query_by_label("Constrain").is_some());
    click_family(&mut h, [300., 300.]);
    click_family(&mut h, [900., 300.]);
    assert_eq!(family(&h).forms.len(), original.forms.len() + 2, "{}", status(&h));
    assert_eq!(family(&h).arrays.len(), 1);
    let f = family(&h);
    let group = f.arrays[0].guid;
    let last = f.forms.iter().position(|f| f.array == Some((group, 2))).unwrap();
    assert!(near(points(&h, last)[0][0], 700.));
    assert_eq!(editor(&h).steps(), 1);
    h.state_mut().family_undo();
    h.run();
    assert!(family(&h).arrays.is_empty());
    assert_eq!(family(&h).forms.len(), original.forms.len());
    h.state_mut().family_redo();
    h.run();
    assert_eq!(family(&h).forms.len(), original.forms.len() + 2);
    ed(&mut h).arrays[0].pattern.number = bimcraft_core::family::Expr::Number(4.);
    h.run();
    assert_eq!(family(&h).forms.len(), original.forms.len() + 3);
    ed(&mut h).array_options.seed = Some(group);
    pick(&mut h, &[Picked::Form(0)]);
    ed(&mut h).apply_edits(vec![bimcraft_core::family::constraints::Edit::End(0, bimcraft_core::family::Extent::End, 550.)]);
    h.run();
    assert!(family(&h).forms.iter().filter(|f| f.array.is_some()).all(|f| f.to == bimcraft_core::family::Expr::Number(550.)));
}
#[test]
fn family_array_without_association_makes_plain_copies_and_array_has_no_default_key() {
    let mut h = plan();
    h.set_size(vec2(1800., 1000.));
    pick(&mut h, &[Picked::Form(0)]);
    let n = family(&h).forms.len();
    press(&mut h, "Array");
    ed(&mut h).array_options.associate = false;
    ed(&mut h).array_options.number = "3".into();
    h.run();
    click_family(&mut h, [300., 300.]);
    click_family(&mut h, [800., 300.]);
    let f = family(&h);
    assert_eq!(f.forms.len(), n + 2, "{}", status(&h));
    assert!(f.arrays.is_empty());
    assert!(f.forms.iter().all(|f| f.array.is_none()));
    assert!(crate::editor::keys::FAMILY_KEYLESS.contains(&"Array"));
    assert!(!crate::editor::keys::FAMILY_SHORTCUTS.iter().any(|(_, a)| *a == "Array"));
}
#[test]
fn family_array_options_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_T17_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    for profile in [crate::style::Profile::Rev, crate::style::Profile::BimCraft] {
        for (name, dark, light) in [("light", false, false), ("dark", true, false), ("dark-ui-light-canvas", true, true)] {
            let mut h = plan();
            h.set_size(vec2(1800., 1000.));
            h.state_mut().profile = profile;
            h.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
            h.state_mut().light_canvas = light;
            pick(&mut h, &[Picked::Form(0)]);
            h.state_mut().family_action("Array");
            h.run();
            h.render().unwrap().save(std::path::Path::new(&folder).join(format!("array-{profile:?}-{name}.png"))).unwrap();
        }
    }
}
