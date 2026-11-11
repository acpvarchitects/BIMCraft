//! Parts in views (`graphics/parts.rs`, `render/parts.rs`): Parts Visibility set through the `graphics` command, as a
//! plan, a section, an elevation and the 3D view draw and pick it.

use serde_json::json;

use super::testing::{owned, pixel, run};
use super::{Look, PartsVisibility};
use crate::engine::Session;
use crate::geom::P2;
use crate::model::{Element, Id, Model};
use crate::parts::{BuildingPart, Source};
use crate::render::{self, Drawing, LIGHT_POCHE, POCHE, Rgb, View};
use crate::view3d::Scene;

/// A project with a historic wall along x from the origin, 4 m long and 3 m tall on Level 1, its location line down its
/// middle: 40 mm of lime plaster on its exterior side (north, y from 210 to 250) over 460 mm of rubble masonry (y from
/// -250 to 210), taken apart into those two parts. The session, the wall, and its parts: the plaster's and the masonry's.
pub(crate) fn historic() -> (Session, Id, [Id; 2]) {
    let mut s = Session::new();
    for (name, color) in [("Lime Plaster", "#e8e0d0"), ("Rubble Masonry", "#9a8f80")] {
        run(&mut s, "material", json!({"name": name, "color": color}));
    }
    run(
        &mut s,
        "type",
        json!({"category": "wall", "name": "Historic - 500mm", "layers": [
            {"function": "finish", "material": "Lime Plaster", "thickness": 40},
            {"function": "structure", "material": "Rubble Masonry", "thickness": 460},
        ]}),
    );
    let wall =
        run(&mut s, "wall", json!({"points": [[0, 0], [4000, 0]], "type": "Historic - 500mm", "height": 3000}))["ids"][0].as_u64().unwrap();
    let layers: Vec<_> = s.model.types.wall["Historic - 500mm"].layers.iter().map(|l| l.guid).collect();
    let host = s.model.guid(wall);
    let parts = [0, 1].map(|k| put(&mut s, BuildingPart::new(host, Source::Layer(layers[k]))));
    (s, wall, parts)
}

/// Part `p` put in the project, which must take it: its id.
pub(crate) fn put(s: &mut Session, p: BuildingPart) -> Id {
    let id = s.model_mut().insert(Element::Part(p));
    assert_eq!(s.model.check(), Ok(()));
    id
}

/// Part `id` changed by `change`.
pub(crate) fn change(s: &mut Session, id: Id, change: impl FnOnce(&mut BuildingPart)) {
    let Some(Element::Part(p)) = s.model_mut().elements.get_mut(id) else { panic!("no part {id}") };
    change(p);
    assert_eq!(s.model.check(), Ok(()));
}

fn level(m: &Model) -> Id {
    m.levels()[0].0
}

/// The plan of Level 1 round the wall, 0.3 px to the mm: the plaster 12 px wide.
pub(crate) fn plan(s: &Session) -> Drawing {
    render::draw(&s.model, level(&s.model), View { scale: 0.3, cx: 2000.0, cy: 0.0, width: 1600, height: 600 })
}

/// Level 1's plan showing parts, at 1 : 10, so its pens are thin beside a layer of plaster: the level.
pub(crate) fn showing_parts(s: &mut Session) -> Id {
    let level = level(&s.model);
    run(s, "scale", json!({"view": level, "scale": 10}));
    run(s, "graphics", json!({"view": level, "parts_visibility": "parts"}));
    level
}

fn at(d: &Drawing, x: f64, y: f64) -> (f32, f32) {
    d.view.px(P2(x, y))
}

#[test]
fn a_view_draws_the_originals_unless_it_says_and_names_its_parts_visibility() {
    let (mut s, wall, [plaster, masonry]) = historic();
    let level = level(&s.model);
    // A view starts at Show Original, and holds nothing for it: its file is as it was.
    let r = run(&mut s, "graphics", json!({"view": level}));
    assert_eq!(r["parts_visibility"], "original");
    let d = plan(&s);
    assert!(!owned(&d, wall).is_empty() && owned(&d, plaster).is_empty() && owned(&d, masonry).is_empty());
    assert_eq!(d.pick(at(&d, 2000.0, 0.0), 2.0), Some(wall));
    assert!(!s.model.to_json().unwrap().contains("parts_visibility"));
    // Each of Revit's three, by its name or by what Properties call it.
    for (given, name) in [("parts", "parts"), ("Show Both", "both"), ("show_original", "original"), ("BOTH", "both")] {
        let r = run(&mut s, "graphics", json!({"view": level, "parts_visibility": given}));
        assert_eq!(r["parts_visibility"], name, "{given}");
    }
    run(&mut s, "graphics", json!({"view": level, "parts_visibility": "parts"}));
    assert!(s.model.to_json().unwrap().contains(r#""parts_visibility": "parts""#));
    // It is saved with the project and read back, and is one step to undo.
    let back = Model::from_json(&s.model.to_json().unwrap()).unwrap();
    assert_eq!(super::graphics_of(&back, level).unwrap().parts_visibility, Some(PartsVisibility::Parts));
    run(&mut s, "undo", json!({}));
    assert_eq!(super::graphics_of(&s.model, level).unwrap().parts_visibility, Some(PartsVisibility::Both));
    // Show Original and null are the view as it starts: nothing held.
    run(&mut s, "graphics", json!({"view": level, "parts_visibility": "original"}));
    assert!(super::graphics_of(&s.model, level).unwrap().is_default());
    run(&mut s, "graphics", json!({"view": level, "parts_visibility": "both"}));
    run(&mut s, "graphics", json!({"view": level, "parts_visibility": null}));
    assert!(super::graphics_of(&s.model, level).unwrap().is_default());
    // What isn't one of them is refused, saying which there are.
    for bad in [json!("some"), json!(2), json!(["parts"])] {
        let before = s.model.clone();
        let e = s.execute("graphics", &json!({"view": level, "parts_visibility": bad})).unwrap_err();
        assert!(e.contains("original, parts or both"), "{e}");
        assert_eq!(*s.model, *before);
    }
}

#[test]
fn a_plan_draws_an_element_as_its_parts_where_its_view_shows_parts() {
    let (mut s, wall, [plaster, masonry]) = historic();
    let level = showing_parts(&mut s);
    let d = plan(&s);
    // The wall is its parts: each the cut of its solid, picked by its own id.
    assert!(owned(&d, wall).is_empty(), "the wall gives way to its parts");
    assert!(!owned(&d, plaster).is_empty() && !owned(&d, masonry).is_empty());
    assert_eq!(d.pick(at(&d, 2000.0, 0.0), 1.0), Some(masonry));
    assert_eq!(d.pick(at(&d, 2000.0, 230.0), 1.0), Some(plaster));
    assert_eq!(d.pick(at(&d, 2000.0, 600.0), 1.0), None);
    // As the wall's layers are drawn: its structure in poché, its finish lighter, a line between the two.
    assert_eq!(pixel(&d, P2(2000.0, 0.0)), POCHE);
    assert_eq!(pixel(&d, P2(2000.0, 230.0)), LIGHT_POCHE);
    let dark = |c: Rgb| c.0 < 100 && c.1 < 100 && c.2 < 100;
    assert!(dark(pixel(&d, P2(2000.0, 210.0))), "the line between the parts: {:?}", pixel(&d, P2(2000.0, 210.0)));
    assert!(dark(pixel(&d, P2(2000.0, 250.0))) && dark(pixel(&d, P2(2000.0, -250.0))), "their outlines");
    // Coarse, each is poché alone.
    run(&mut s, "detail", json!({"view": level, "detail": "coarse"}));
    let coarse = plan(&s);
    assert_eq!(pixel(&coarse, P2(2000.0, 230.0)), POCHE);
    run(&mut s, "detail", json!({"view": level, "detail": "fine"}));
    // Show Both draws the wall and its parts, the parts over it.
    run(&mut s, "graphics", json!({"view": level, "parts_visibility": "both"}));
    let d = plan(&s);
    assert!(!owned(&d, wall).is_empty() && !owned(&d, plaster).is_empty() && !owned(&d, masonry).is_empty());
    assert_eq!(d.pick_all(at(&d, 2000.0, 0.0), 1.0), [masonry, wall]);
    // A wall with no parts is drawn as ever, whatever the view shows.
    let plain = run(&mut s, "wall", json!({"points": [[0, 3000], [4000, 3000]]}))["ids"][0].as_u64().unwrap();
    run(&mut s, "graphics", json!({"view": level, "parts_visibility": "parts"}));
    let d = plan(&s);
    assert!(!owned(&d, plain).is_empty() && owned(&d, wall).is_empty());
    // A door through the wall leaves its gap in the parts.
    run(&mut s, "door", json!({"wall": wall, "offset": 1000}));
    let d = plan(&s);
    assert_eq!(pixel(&d, P2(1000.0, 100.0)), render::PAPER);
    assert_eq!(pixel(&d, P2(2500.0, 0.0)), POCHE);
    // A view that hides walls hides their parts with them.
    run(&mut s, "graphics", json!({"view": level, "hide": ["walls"]}));
    let d = plan(&s);
    assert!(owned(&d, plaster).is_empty() && owned(&d, masonry).is_empty() && owned(&d, wall).is_empty());
}

#[test]
fn a_part_below_the_cut_plane_is_seen_and_one_over_it_is_not() {
    let (mut s, wall, [plaster, masonry]) = historic();
    showing_parts(&mut s);
    // A wall 900 mm tall stands under the cut plane: its parts are seen from above, outlined and not filled.
    run(&mut s, "modify", json!({"id": wall, "height": 900}));
    let d = plan(&s);
    assert!(!owned(&d, plaster).is_empty() && !owned(&d, masonry).is_empty());
    assert_eq!(pixel(&d, P2(2000.0, 0.0)), render::PAPER);
    assert!(d.parts.iter().zip(&d.owners).filter(|(_, o)| **o == Some(masonry)).all(|(p, _)| *p == render::Part::Projection));
    assert_eq!(d.pick(at(&d, 2000.0, 250.0), 2.0), Some(plaster));
    // Raised over the cut plane, a floor plan shows none of it.
    run(&mut s, "modify", json!({"id": wall, "base_offset": 2000}));
    let d = plan(&s);
    assert!(owned(&d, plaster).is_empty() && owned(&d, masonry).is_empty());
}

#[test]
fn a_part_excluded_in_conflict_or_divided_is_not_drawn_and_a_conflict_leaves_its_host_whole() {
    let (mut s, wall, [plaster, masonry]) = historic();
    let level = showing_parts(&mut s);
    let shown = |s: &Session| {
        let look = Look::of(&s.model, level);
        [wall, plaster, masonry].map(|id| look.shows(id))
    };
    assert_eq!(shown(&s), [false, true, true]);
    // Excluded, a part is left out, and the rest of its host's parts still stand for it.
    change(&mut s, plaster, |p| p.excluded = true);
    assert_eq!(shown(&s), [false, false, true]);
    let d = plan(&s);
    assert!(owned(&d, plaster).is_empty() && !owned(&d, masonry).is_empty());
    assert_eq!(pixel(&d, P2(2000.0, 230.0)), render::PAPER, "nothing where the excluded part was");
    // With no part left to draw, the wall is drawn whole.
    change(&mut s, masonry, |p| p.excluded = true);
    assert_eq!(shown(&s), [true, false, false]);
    change(&mut s, plaster, |p| p.excluded = false);
    change(&mut s, masonry, |p| p.excluded = false);
    // Divided, a part gives way to the parts it was divided into.
    let (host, parent, source) = {
        let Some(Element::Part(p)) = s.model.elements.get(plaster) else { panic!() };
        (p.host, s.model.guid(plaster), p.source)
    };
    let child = put(&mut s, BuildingPart { parent: Some(parent), ..BuildingPart::new(host, source) });
    let look = Look::of(&s.model, level);
    assert!(!look.shows(plaster) && look.shows(child) && look.shows(masonry) && !look.shows(wall));
    let d = plan(&s);
    assert!(owned(&d, plaster).is_empty() && !owned(&d, child).is_empty());
    assert!(!Scene::new(&s.model).render_in(&Scene::new(&s.model).fit(60.0, 30.0, 320, 240), &[], &look).shows().contains(&plaster));
    // Its host of another type, a part is in conflict: it isn't drawn, and the wall is whole again, so nothing goes missing.
    run(&mut s, "type", json!({"category": "wall", "name": "Plain - 300mm", "thickness": 300}));
    run(&mut s, "modify", json!({"id": wall, "type": "Plain - 300mm"}));
    assert!(s.model.part_state(masonry).unwrap().conflict.is_some());
    let look = Look::of(&s.model, level);
    assert!(look.shows(wall) && !look.shows(plaster) && !look.shows(masonry) && !look.shows(child));
    let d = plan(&s);
    assert!(!owned(&d, wall).is_empty() && owned(&d, masonry).is_empty());
}

#[test]
fn the_3d_view_draws_parts_in_their_materials_and_picks_them() {
    let (mut s, wall, [plaster, masonry]) = historic();
    let scene = Scene::new(&s.model);
    // From the north-east and above: the plaster's face, the wall's east end and its top.
    let cam = scene.fit(60.0, 30.0, 480, 360);
    let whole = scene.render_in(&cam, &[], &Look::of_3d(&s.model));
    let of_wall = |img: &crate::view3d::Image| [wall, plaster, masonry].map(|id| img.shows().contains(&id));
    assert_eq!(of_wall(&whole), [true, false, false], "Show Original: the wall, and no part");
    run(&mut s, "graphics", json!({"view": "{3D}", "parts_visibility": "parts"}));
    let look = Look::of_3d(&s.model);
    let parts = scene.render_in(&cam, &[], &look);
    assert_eq!(of_wall(&parts), [false, true, true], "Show Parts: its parts, and no more of it");
    // Each where it is, and in its material's colour as the wall's layers are: the picture is the wall's, but for the
    // lines between its parts.
    let (w, h) = (480usize, 360usize);
    let picked = |img: &crate::view3d::Image, id: Id| (0..w * h).filter(|i| img.pick((i % w) as f64, (i / w) as f64) == Some(id)).count();
    assert!(picked(&parts, plaster) > 2000 && picked(&parts, masonry) > 500, "{} {}", picked(&parts, plaster), picked(&parts, masonry));
    let differ = whole.rgba.chunks(4).zip(parts.rgba.chunks(4)).filter(|(a, b)| a != b).count();
    assert!(differ < (w * h) / 50, "{differ} pixels differ: the same surfaces, in the same materials");
    // A part's own material is what it is drawn in.
    run(&mut s, "material", json!({"name": "Cement Render", "color": "#3050c0"}));
    change(&mut s, plaster, |p| p.material = Some("Cement Render".into()));
    let scene = Scene::new(&s.model);
    let own = scene.render_in(&cam, &[], &Look::of_3d(&s.model));
    let blue = own.rgba.chunks(4).filter(|c| c[2] > c[0].saturating_add(40) && c[2] > c[1].saturating_add(40)).count();
    assert!(blue > 2000, "{blue}");
    // Selected, a part is tinted as any element is.
    let tinted = scene.render_in(&cam, &[(plaster, Rgb(37, 99, 235))], &Look::of_3d(&s.model));
    assert_ne!(tinted.rgba, own.rgba);
    // Show Both draws the wall too.
    run(&mut s, "graphics", json!({"view": "{3D}", "parts_visibility": "both"}));
    let both = scene.render_in(&cam, &[], &Look::of_3d(&s.model));
    assert!(both.shows().contains(&wall));
}

#[test]
fn a_section_cuts_parts_and_an_elevation_sees_them() {
    let (mut s, wall, [plaster, masonry]) = historic();
    // Across the wall, looking east; and an elevation of its north face, the plaster's.
    let section = run(&mut s, "section", json!({"start": [2000, 1500], "end": [2000, -1500]}))["id"].as_u64().unwrap();
    run(&mut s, "scale", json!({"view": section, "scale": 10}));
    let view = View { scale: 0.5, cx: 1500.0, cy: 1500.0, width: 800, height: 600 };
    let d = render::section(&s.model, section, view);
    assert!(!owned(&d, wall).is_empty() && owned(&d, plaster).is_empty());
    run(&mut s, "graphics", json!({"view": section, "parts_visibility": "parts"}));
    let d = render::section(&s.model, section, view);
    assert!(owned(&d, wall).is_empty() && !owned(&d, plaster).is_empty() && !owned(&d, masonry).is_empty());
    // Along the line from its start at y = 1500: the plaster from 1250 to 1290, the masonry on to 1750.
    assert_eq!(d.pick(d.view.px(P2(1270.0, 1500.0)), 1.0), Some(plaster));
    assert_eq!(d.pick(d.view.px(P2(1500.0, 1500.0)), 1.0), Some(masonry));
    assert_eq!(pixel(&d, P2(1500.0, 1500.0)), POCHE);
    assert_eq!(pixel(&d, P2(1270.0, 1500.0)), LIGHT_POCHE);
    let dark = |c: Rgb| c.0 < 100 && c.1 < 100 && c.2 < 100;
    assert!(dark(pixel(&d, P2(1290.0, 1500.0))), "the line between the parts: {:?}", pixel(&d, P2(1290.0, 1500.0)));
    // The elevation's picture is of the parts, the plaster in front.
    let elevation = run(&mut s, "elevation", json!({"facing": "north"}))["id"].as_u64().unwrap();
    let view = render::section_fit(&s.model, elevation, 800, 600);
    let whole = render::section_picture(&s.model, elevation, view, &[]).unwrap();
    assert!(whole.shows().contains(&wall) && !whole.shows().contains(&plaster));
    run(&mut s, "graphics", json!({"view": elevation, "parts_visibility": "parts"}));
    let seen = render::section_picture(&s.model, elevation, view, &[]).unwrap();
    assert!(seen.shows().contains(&plaster) && !seen.shows().contains(&wall), "{:?}", seen.shows());
    assert_eq!(seen.pick(400.0, 300.0), Some(plaster));
}

#[test]
fn a_view_template_that_holds_a_parts_visibility_sets_it_for_its_views() {
    let (mut s, wall, [plaster, _]) = historic();
    let level = level(&s.model);
    // A template says none unless it is given one, and leaves it to each view.
    let t = run(&mut s, "template", json!({"name": "Survey", "kind": "plan", "controls": ["detail"]}));
    assert_eq!(t["parts_visibility"], json!(null));
    run(&mut s, "graphics", json!({"view": level, "template": "Survey", "parts_visibility": "parts"}));
    assert!(!Look::of(&s.model, level).shows(wall));
    // Given one, its views show what it says, and can't say otherwise.
    run(&mut s, "template", json!({"name": "Survey", "parts_visibility": "original"}));
    assert!(Look::of(&s.model, level).shows(wall) && !Look::of(&s.model, level).shows(plaster));
    assert_eq!(run(&mut s, "graphics", json!({"view": level}))["parts_visibility"], "original");
    let e = s.execute("graphics", &json!({"view": level, "parts_visibility": "both"})).unwrap_err();
    assert!(e.contains("parts visibility follows view template `Survey`"), "{e}");
    let e = s.execute("modify", &json!({"id": level, "graphics": {"template": "Survey", "parts_visibility": "both"}})).unwrap_err();
    assert!(e.contains("parts visibility follows view template `Survey`"), "{e}");
    run(&mut s, "template", json!({"name": "Survey", "parts_visibility": "parts"}));
    assert!(!Look::of(&s.model, level).shows(wall) && Look::of(&s.model, level).shows(plaster));
    // Unfollowing and changing in one call is fine; and a template made from a view that shows parts takes it.
    run(&mut s, "graphics", json!({"view": level, "template": null, "parts_visibility": "both"}));
    let t = run(&mut s, "template", json!({"name": "Both", "from": level}));
    assert_eq!(t["parts_visibility"], "both");
    // With null it leaves it to each view again.
    let t = run(&mut s, "template", json!({"name": "Both", "parts_visibility": null}));
    assert_eq!(t["parts_visibility"], json!(null));
}
