//! Parts in the desktop: Create Parts on the Modify tab, Exclude Parts and Restore Parts, a part's Properties (what it is
//! made of, its kind, the heritage set), a view's parts coloured by one of their values, and a material's weight and heat in
//! the Materials window, driven as a user drives them.

use super::*;
use bimcraft_core::decay::Decay;
use bimcraft_core::graphics::PartsVisibility;
use bimcraft_core::parts::{BuildingPart, PartType};
use egui_kittest::kittest::NodeT;

/// A wall south of the box, of plaster on brick.
pub(super) const HISTORIC: &str = r#"type {"category": "wall", "name": "Historic", "layers": [{"function": "finish", "thickness": 20, "material": "Plaster"}, {"function": "structure", "thickness": 460, "material": "Brick"}]}
wall {"points": [[0,-3000],[4000,-3000]], "type": "Historic", "height": 3000}"#;

pub(super) type Build = fn(&str) -> Harness<'static, BimApp>;

/// The window in both profiles: Rev's ribbon and BIMCraft's Tools palette.
pub(super) const BOTH: [(&str, Build); 2] = [("Rev", app), ("BIMCraft", bimcraft)];

/// The wall south of the box.
pub(super) const WALL: Id = 100006;

/// The wall taken apart: its plaster and its brick.
pub(super) fn surveyed() -> Harness<'static, BimApp> {
    app(&format!("{BOX}\n{HISTORIC}\ncreate_parts {{\"id\": {WALL}}}"))
}

pub(super) fn part_of(h: &Harness<BimApp>, id: Id) -> BuildingPart {
    match h.state().session.model.elements.get(id) {
        Some(Element::Part(p)) => p.clone(),
        _ => panic!("part {id} is gone"),
    }
}

/// Select `ids`, as a click in the view does: the Modify tab comes up. The ribbon fits its panels to the window over a few
/// frames, which a pointer moving to them gives it; a harness asks for none, so it is given them here.
pub(super) fn select(h: &mut Harness<BimApp>, ids: &[Id]) {
    h.state_mut().selection = ids.to_vec();
    h.state_mut().raise_context();
    h.run();
    for _ in 0..4 {
        h.step();
    }
}

/// Tick or untick the check box named `label`, wherever the palette has scrolled it to.
fn tick(h: &mut Harness<BimApp>, label: &str) {
    h.get_by_role_and_label(Role::CheckBox, label).click_accesskit();
    h.run();
}

fn disabled(h: &Harness<BimApp>, role: Role, label: &str) -> bool {
    h.get_by_role_and_label(role, label).accesskit_node().is_disabled()
}

/// Pick `option` from the drop-down named `label`: the last widget of that name, the list being drawn over the palette.
fn choose_from(h: &mut Harness<BimApp>, label: &str, option: &str) {
    h.get_by_role_and_label(Role::ComboBox, label).click_accesskit();
    h.run();
    h.get_all_by(|n| n.label().as_deref() == Some(option)).last().unwrap_or_else(|| panic!("no {option}")).click_accesskit();
    h.run();
    if egui::Popup::is_any_open(&h.ctx) {
        h.get_by_role_and_label(Role::ComboBox, label).click_accesskit();
        h.run();
    }
}

#[test]
fn create_parts_on_the_modify_tab_takes_the_selection_apart_selects_its_parts_and_shows_them() {
    let mut h = app(&format!("{BOX}\n{HISTORIC}"));
    assert!(h.query_by_label("Create Parts").is_none(), "nothing selected, nothing to take apart");
    select(&mut h, &[WALL]);
    let before = h.state().session.history().len();
    from_list(&mut h, "Create", "Create Parts");
    let parts = ids(&h, "part");
    assert_eq!(parts.len(), 2, "its plaster and its brick");
    // As Revit's: the parts selected, and the view showing them, in one undo step.
    assert_eq!(h.state().selection, parts);
    assert_eq!(plan_graphics(&h, 100000).parts_visibility, Some(PartsVisibility::Parts));
    assert_eq!((h.state().session.history().len(), h.state().session.history().last()), (before + 1, Some(&"Create Parts")));
    press(&mut h, "Undo");
    assert!(ids(&h, "part").is_empty());
    assert_eq!(plan_graphics(&h, 100000).parts_visibility, None);
    // A view whose parts' visibility its template sets is left as it is.
    h.state_mut().session.execute("template", &json!({"name": "Survey", "controls": [], "parts_visibility": "original"})).unwrap();
    h.state_mut().session.execute("graphics", &json!({"view": 100000, "template": "Survey"})).unwrap();
    select(&mut h, &[WALL]);
    from_list(&mut h, "Create", "Create Parts");
    assert_eq!(ids(&h, "part").len(), 2);
    assert!(!h.state().status.error, "{}", h.state().status.text);
    // Of what has all its parts already, the core says so.
    select(&mut h, &[WALL]);
    from_list(&mut h, "Create", "Create Parts");
    assert!(h.state().status.error);
    assert_eq!(ids(&h, "part").len(), 2);
}

#[test]
fn exclude_parts_and_restore_parts_on_the_modify_tab_each_take_the_parts_selected() {
    let mut h = surveyed();
    let [plaster, brick] = ids(&h, "part")[..] else { panic!("two parts") };
    select(&mut h, &[plaster]);
    assert!(h.query_by_label("Create Parts").is_none(), "a part is taken apart no further");
    assert!(disabled(&h, Role::Button, "Restore Parts"), "nothing excluded to restore");
    from_list(&mut h, "Part", "Exclude Parts");
    assert!(part_of(&h, plaster).excluded && !part_of(&h, brick).excluded);
    assert!(disabled(&h, Role::Button, "Exclude Parts"));
    // Both selected, one of them excluded: each button takes them all.
    select(&mut h, &[plaster, brick]);
    from_list(&mut h, "Part", "Exclude Parts");
    assert!(part_of(&h, brick).excluded);
    from_list(&mut h, "Part", "Restore Parts");
    assert!(!part_of(&h, plaster).excluded && !part_of(&h, brick).excluded);
}

#[test]
fn a_parts_properties_show_what_it_is_of_and_set_what_it_is_made_of_its_kind_and_the_heritage_set() {
    let mut h = surveyed();
    let [plaster, brick] = ids(&h, "part")[..] else { panic!("two parts") };
    select(&mut h, &[plaster]);
    // What it is a part of and what it measures, read only: its host's type, its layer and its material by its layer.
    h.get_by_label("Historic");
    h.get_by_label("1 (Finish)");
    assert_eq!(combo_value(&h, "Material"), "Plaster (By Original)");
    assert_eq!(combo_value(&h, "Kind"), "Not Defined");
    // Its own material, then its layer's again.
    choose_from(&mut h, "Material", "Concrete");
    assert_eq!(part_of(&h, plaster).material.as_deref(), Some("Concrete"));
    choose_from(&mut h, "Material", "By Original");
    assert_eq!(part_of(&h, plaster).material, None);
    // A kind of the project's own says what it is first.
    type_role(&mut h, Role::TextInput, "Object Type", "USM");
    choose_from(&mut h, "Kind", "User Defined");
    let p = part_of(&h, plaster);
    assert_eq!((p.predefined_type, p.object_type.as_str()), (PartType::UserDefined, "USM"));
    // The heritage set.
    type_role(&mut h, Role::TextInput, "SU_Code", "USM 12");
    type_role(&mut h, Role::TextInput, "Historical_Period", "XVI century");
    choose_from(&mut h, "Decay_Pathology", "Crust");
    choose_from(&mut h, "Decay_Severity", "4");
    let p = part_of(&h, plaster);
    assert_eq!((p.heritage.su_code.as_str(), p.heritage.historical_period.as_str()), ("USM 12", "XVI century"));
    assert_eq!((p.heritage.decay_pathology.clone(), p.heritage.decay_severity), (Some(Decay::Crust), Some(4)));
    // A decay in the project's own words, then none.
    type_role(&mut h, Role::TextInput, "Other Decay", "Salt bloom");
    assert_eq!(part_of(&h, plaster).heritage.decay_pathology, Some(Decay::Custom("Salt bloom".into())));
    assert_eq!(combo_value(&h, "Decay_Pathology"), "Salt bloom");
    choose_from(&mut h, "Decay_Pathology", "<None>");
    assert_eq!(part_of(&h, plaster).heritage.decay_pathology, None);
    // Excluded from its own row too.
    tick(&mut h, "Excluded");
    assert!(part_of(&h, plaster).excluded);
    // Two selected: what they share is shown, and a value typed goes to both.
    select(&mut h, &[plaster, brick]);
    assert_eq!(field_text(&h, "SU_Code"), "");
    type_role(&mut h, Role::TextInput, "Intervention_Type", "Consolidation");
    assert_eq!(part_of(&h, plaster).heritage.intervention_type, "Consolidation");
    assert_eq!(part_of(&h, brick).heritage.intervention_type, "Consolidation");
    // What the core refuses is said, and nothing changes.
    select(&mut h, &[brick]);
    choose_from(&mut h, "Kind", "User Defined");
    assert!(h.state().status.error, "a part of the project's own kind says what it is");
    assert_eq!(part_of(&h, brick).predefined_type, PartType::NotDefined);
}

#[test]
fn a_views_properties_colour_its_parts_by_one_of_their_values_and_draw_its_key() {
    let mut h =
        app(&format!("{BOX}\n{HISTORIC}\ncreate_parts {{\"id\": {WALL}}}\ngraphics {{\"view\": 100000, \"parts_visibility\": \"parts\"}}"));
    let theme = |h: &Harness<BimApp>| plan_graphics(h, 100000).theme;
    assert_eq!(combo_value(&h, "Color Parts By"), "<None>");
    assert!(h.query_by_label("Parts Legend").is_none(), "no key without colours");
    choose_from(&mut h, "Color Parts By", "Decay_Pathology");
    assert_eq!(theme(&h).map(|t| (t.by, t.legend)), Some(("Decay_Pathology".into(), false)));
    tick(&mut h, "Parts Legend");
    assert_eq!(theme(&h).map(|t| t.legend), Some(true));
    // Another value keeps the key.
    choose_from(&mut h, "Color Parts By", "Material");
    assert_eq!(theme(&h).map(|t| (t.by, t.legend)), Some(("Material".into(), true)));
    choose_from(&mut h, "Color Parts By", "<None>");
    assert_eq!(theme(&h), None);
    // A template that sets the view's parts sets their colours too: the rows are greyed out.
    h.state_mut().session.execute("template", &json!({"name": "Survey", "controls": [], "parts_visibility": "parts"})).unwrap();
    h.state_mut().session.execute("graphics", &json!({"view": 100000, "template": "Survey"})).unwrap();
    h.run();
    assert!(disabled(&h, Role::ComboBox, "Color Parts By"));
}

#[test]
fn the_materials_window_says_a_materials_density_conductivity_and_specific_heat() {
    let mut h = app(BOX);
    press(&mut h, "Manage");
    press(&mut h, "Materials");
    press(&mut h, "New");
    let physics = |h: &Harness<BimApp>| h.state().session.model.materials["Material 1"].physics();
    assert_eq!(field_text(&h, "Density (kg/m³)"), "", "not said until it is");
    type_role(&mut h, Role::TextInput, "Density (kg/m³)", "1800");
    type_role(&mut h, Role::TextInput, "Thermal Conductivity (W/(m·K))", "0.72");
    type_role(&mut h, Role::TextInput, "Specific Heat (J/(kg·K))", "840");
    assert_eq!(physics(&h), [Some(1800.0), Some(0.72), Some(840.0)]);
    // Cleared, it is not said; what isn't a number is said so, and nothing changes.
    type_role(&mut h, Role::TextInput, "Density (kg/m³)", "");
    assert_eq!(physics(&h), [None, Some(0.72), Some(840.0)]);
    type_role(&mut h, Role::TextInput, "Specific Heat (J/(kg·K))", "a lot");
    assert!(h.state().status.error);
    assert_eq!(physics(&h), [None, Some(0.72), Some(840.0)]);
    // Another material shows its own.
    press(&mut h, "New");
    assert_eq!(field_text(&h, "Thermal Conductivity (W/(m·K))"), "");
}

/// What the parts' tools look like, for a person to look at: `BIMCRAFT_PARTS_DUMP=<folder>` writes, in both profiles, light and
/// dark, the Modify tab with a wall selected, its parts selected with their Properties, the view's Properties colouring them,
/// and the Materials window with a material's weight and heat.
#[test]
fn parts_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_PARTS_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    let surveyed = format!(
        "{BOX}\n{HISTORIC}\ncreate_parts {{\"id\": {WALL}}}\npart {{\"id\": 100007, \"su_code\": \"USM 12\", \"decay_pathology\": \"crust\", \"decay_severity\": 3, \"intervention_type\": \"Consolidation\"}}\ngraphics {{\"view\": 100000, \"parts_visibility\": \"parts\", \"theme\": {{\"by\": \"Decay_Pathology\", \"legend\": true}}}}\nmaterial {{\"name\": \"Brick\", \"density\": 1800, \"conductivity\": 0.72, \"specific_heat\": 840}}"
    );
    for (profile, build) in BOTH {
        for (dark, look) in [(false, "light"), (true, "dark")] {
            let shot = |h: &mut Harness<BimApp>, name: &str| {
                if dark {
                    h.ctx.set_theme(egui::Theme::Dark);
                }
                h.run();
                h.render().unwrap().save(std::path::Path::new(&folder).join(format!("parts-{profile}-{look}-{name}.png"))).unwrap();
            };
            let mut h = build(&format!("{BOX}\n{HISTORIC}"));
            select(&mut h, &[WALL]);
            shot(&mut h, "create");
            let mut h = build(&surveyed);
            select(&mut h, &[100007]);
            shot(&mut h, "part");
            h.get_by_label("Intervention_Type").scroll_to_me();
            shot(&mut h, "part-heritage");
            select(&mut h, &[]);
            h.get_by_label("Parts Legend").scroll_to_me();
            shot(&mut h, "view");
            let mut h = build(&surveyed);
            h.state_mut().materials_dialog = Some(crate::properties::MaterialsDialog::new(&h.state().session.model));
            shot(&mut h, "materials");
        }
    }
}
