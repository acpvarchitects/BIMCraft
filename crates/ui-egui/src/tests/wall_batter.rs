//! A battered wall (HBIM shaped walls, option A) shows its thinnest, thickest and average thickness in Properties; a plumb one
//! doesn't, its thickness being its type's throughout.

use super::*;

#[test]
fn a_battered_walls_properties_show_its_thickness_up_its_height() {
    let mut h = app(BOX);
    let walls = ids(&h, "wall");
    let (battered, plumb) = (walls[0], walls[1]);
    let done = h.state_mut().run("batter", json!({"id": battered, "face": "exterior", "top": -100}));
    assert!(done.is_some());
    h.state_mut().selection = vec![battered];
    h.run();
    for label in ["Thinnest", "Thickest", "Average Thickness"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    let t = h.state().session.model.thickness(match h.state().session.model.elements.get(battered) {
        Some(Element::Wall(w)) => w,
        _ => panic!("a wall"),
    });
    assert_eq!(field(&h, battered, "thickness_min"), json!(t - 100.0));
    if let Ok(folder) = std::env::var("BIMCRAFT_HBIM_DUMP") {
        std::fs::create_dir_all(&folder).unwrap();
        h.set_size(vec2(1600., 1000.));
        h.run();
        h.get_by_label("Average Thickness").scroll_to_me();
        h.run();
        h.render().unwrap().save(std::path::Path::new(&folder).join("iw1-wall-properties.png")).unwrap();
    }
    h.state_mut().selection = vec![plumb];
    h.run();
    assert!(h.query_by_label("Thinnest").is_none());
}
