//! FE-24: the reviewer's probes of the Modify tools in {3D}, kept as regression tests. Two had clicks that fell off the
//! pane (so nothing was clicked): their points are moved onto it, the checks are as they were.

use super::family_drag_move::{EDGE, LINE, ed, family, family_with, g, pick, points, status};
use super::family_views::{editor, open, view};
use super::*;
use crate::editor::modify::Tool as M;
use crate::editor::select_box::Picked;
use bimcraft_core::family::Expr;
use bimcraft_core::family::views::Which;

const FRONT_BACK: &str = "9e3c0a10-1f0b-4d8c-8a5e-0000000000a2";

fn in_3d_of(script: &str) -> Harness<'static, BimApp> {
    let (mut h, _) = open(script, "Movers");
    view(&mut h, None);
    for _ in 0..4 {
        h.run();
    }
    h
}

fn click3(h: &mut Harness<BimApp>, p: [f64; 3]) {
    let at = h.state().family_screen3(p).expect("{3D} drawn");
    click_at(h, at);
}

fn tool(h: &Harness<BimApp>) -> Option<M> {
    editor(h).modify_in_hand()
}

fn num(e: &Expr) -> f64 {
    match e {
        Expr::Number(x) => *x,
        _ => f64::NAN,
    }
}

#[test]
fn probe_copy_multiple_in_3d() {
    let mut h = in_3d_of(&family_with(""));
    pick(&mut h, &[Picked::Form(0)]);
    let n = family(&h).forms.len();
    shortcut(&mut h, "CO");
    ed(&mut h).views.modify_options.multiple = true;
    click3(&mut h, [0.0, 0.0, 0.0]);
    click3(&mut h, [0.0, 1000.0, 0.0]);
    for _ in 0..3 {
        h.run();
    }
    eprintln!("after 1st copy: forms {} tool {:?}", family(&h).forms.len(), tool(&h));
    // (At 2000 north the click fell off the pane: 500 north of the first copy is on it.)
    click3(&mut h, [0.0, 500.0, 0.0]);
    for _ in 0..3 {
        h.run();
    }
    eprintln!("after 3rd click: forms {} tool {:?} status {}", family(&h).forms.len(), tool(&h), status(&h));
    assert_eq!(family(&h).forms.len(), n + 2, "Multiple: a second copy from the third click");
}

#[test]
fn probe_rotate_planes_and_lines_only_in_3d() {
    let mut h = in_3d_of(&family_with(""));
    pick(&mut h, &[Picked::Plane(g(EDGE))]);
    shortcut(&mut h, "RO");
    for _ in 0..2 {
        h.run();
    }
    eprintln!("RO tool {:?} status {}", tool(&h), status(&h));
    ed(&mut h).views.modify_options.rotate_copy = false;
    // (East of the plane the clicks fell off the pane: from west of its middle, turned towards the south-west.)
    click3(&mut h, [400.0, 0.0, 0.0]);
    eprintln!("after click: tool {:?} status {}", tool(&h), status(&h));
    click3(&mut h, [400.0, -300.0, 0.0]);
    let p = family(&h).planes.iter().find(|p| p.guid == g(EDGE)).unwrap().clone();
    eprintln!("plane now {:?} {:?}: {}", p.start, p.end, status(&h));
    assert!((p.start[0] - 1000.0).abs() > 1.0 || (p.end[0] - 1000.0).abs() > 1.0, "the plane turned: {}", status(&h));
}

#[test]
fn probe_move_plane_and_line_from_a_plan_in_3d() {
    let mut h = in_3d_of(&family_with(""));
    pick(&mut h, &[Picked::Plane(g(EDGE)), Picked::Line(g(LINE))]);
    shortcut(&mut h, "MV");
    click3(&mut h, [0.0, 0.0, 0.0]);
    click3(&mut h, [300.0, 100.0, 0.0]);
    let f = family(&h);
    let p = f.planes.iter().find(|p| p.guid == g(EDGE)).unwrap();
    let l = f.lines.iter().find(|l| l.guid == g(LINE)).unwrap();
    eprintln!("plane {:?} line {:?} {:?}: {}", p.start, l.start, l.end, status(&h));
    assert!((p.start[0] - 1300.0).abs() < 2.0, "plane x");
    assert!((l.start[0] + 200.0 - 300.0).abs() < 2.0 && (l.start[1] - 1300.0).abs() < 2.0, "line");
}

#[test]
fn probe_pinned_refused_in_3d() {
    let mut h = in_3d_of(&family_with(""));
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "PN");
    let before = points(&h, 0);
    for keys in ["MV", "RO"] {
        shortcut(&mut h, keys);
        for _ in 0..2 {
            h.run();
        }
        eprintln!("{keys}: tool {:?} status {}", tool(&h), status(&h));
        if keys == "MV" {
            click3(&mut h, [0.0, 0.0, 0.0]);
            click3(&mut h, [300.0, 0.0, 0.0]);
        } else {
            click3(&mut h, [700.0, 300.0, 0.0]);
            eprintln!("RO 1st: {:?} {}", tool(&h), status(&h));
            click3(&mut h, [300.0, 700.0, 0.0]);
        }
        eprintln!("{keys} after: status {}", status(&h));
        assert_eq!(points(&h, 0), before, "pinned stays");
        assert!(status(&h).to_lowercase().contains("pinned"), "{}", status(&h));
        key(&mut h, Key::Escape);
        key(&mut h, Key::Escape);
    }
}

#[test]
fn probe_solid_and_void_moved_together_in_3d() {
    let script = family_with("").replace(
        r#""to": 300}], "dimensions""#,
        r#""to": 300}, {"void": true, "sketch": [{"start": [200, 200], "end": [400, 200]}, {"start": [400, 200], "end": [400, 400]}, {"start": [400, 400], "end": [200, 400]}, {"start": [200, 400], "end": [200, 200]}], "from": -10, "to": 310}], "dimensions""#,
    );
    let mut h = in_3d_of(&script);
    assert_eq!(family(&h).forms.len(), 3);
    pick(&mut h, &[Picked::Form(0), Picked::Form(2)]);
    for _ in 0..3 {
        h.run();
    }
    let (a, v) = (points(&h, 0)[0], points(&h, 2)[0]);
    shortcut(&mut h, "MV");
    click3(&mut h, [0.0, 0.0, 0.0]);
    click3(&mut h, [300.0, 0.0, 0.0]);
    let (a2, v2) = (points(&h, 0)[0], points(&h, 2)[0]);
    eprintln!("solid {a:?}->{a2:?} void {v:?}->{v2:?}: {}", status(&h));
    assert!((a2[0] - a[0] - 300.0).abs() < 2.0, "solid moved");
    assert!((v2[0] - v[0] - 300.0).abs() < 2.0, "the void selected with it moved too");
}

#[test]
fn probe_edge_on_and_parallel_views_refuse_the_click() {
    let mut h = in_3d_of(&family_with(""));
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "MV");
    let rect = ed(&mut h).orbit.rect;
    // Level, parallel, seen exactly edge on.
    for (elev, lens) in [(0.0, None), (0.0, Some(35.0)), (10.0, Some(35.0)), (0.001, None)] {
        {
            let e = ed(&mut h);
            let c = e.orbit.camera.expect("a camera");
            e.orbit.camera = Some(bimcraft_core::view3d::Camera { elevation: elev, lens, ..c });
            e.orbit.fitted = None;
        }
        h.run();
        let at = rect.center() - egui::vec2(0.0, rect.height() * 0.4);
        click_at(&mut h, at);
        eprintln!("elev {elev} lens {lens:?}: tool {:?} status {}", tool(&h), status(&h));
        key(&mut h, Key::Escape);
        shortcut(&mut h, "MV");
    }
}

#[test]
fn probe_grazing_click_lands_far_away() {
    let mut h = in_3d_of(&family_with(""));
    pick(&mut h, &[Picked::Form(0)]);
    let before = points(&h, 0)[0];
    shortcut(&mut h, "MV");
    click3(&mut h, [0.0, 0.0, 0.0]);
    {
        let e = ed(&mut h);
        let c = e.orbit.camera.expect("a camera");
        e.orbit.camera = Some(bimcraft_core::view3d::Camera { elevation: 0.01, lens: None, ..c });
        e.orbit.fitted = None;
    }
    h.run();
    let rect = ed(&mut h).orbit.rect;
    click_at(&mut h, rect.center() - egui::vec2(0.0, rect.height() * 0.3));
    let after = points(&h, 0)[0];
    eprintln!("grazing: {before:?} -> {after:?}: tool {:?} status {}", tool(&h), status(&h));
}

#[test]
fn probe_rotate_and_mirror_on_an_upright_plane() {
    let mut h = in_3d_of(&family_with(""));
    ed(&mut h).views.set_work_plane(bimcraft_core::family::PlaneRef::Plane(g(FRONT_BACK)));
    pick(&mut h, &[Picked::Form(0)]);
    let before = serde_json::to_value(&family(&h).forms[0]).unwrap();
    shortcut(&mut h, "RO");
    for _ in 0..2 {
        h.run();
    }
    eprintln!("RO upright: tool {:?}", tool(&h));
    ed(&mut h).views.modify_options.rotate_copy = false;
    // Centre is (300, 150) on the plane: x 300, z 150. East of it, then above it.
    eprintln!("rect {:?} at {:?}", editor(&h).orbit.rect, h.state().family_screen3([700.0, 0.0, 150.0]));
    click3(&mut h, [700.0, 0.0, 150.0]);
    eprintln!("1st: {:?}", tool(&h));
    click3(&mut h, [300.0, 0.0, 550.0]);
    let after = serde_json::to_value(&family(&h).forms[0]).unwrap();
    eprintln!("rotate upright: status {} tool {:?}\nbefore {before}\nafter {after}", status(&h), tool(&h));
    let mesh = bimcraft_core::family::views::form_corners(&family(&h), &editor(&h).values(), 0);
    let (lo, hi) = mesh.iter().fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), p| {
        (std::array::from_fn(|k| lo[k].min(p[k])), std::array::from_fn(|k| hi[k].max(p[k])))
    });
    eprintln!("rotated box {lo:?} {hi:?}");
}

#[test]
fn probe_nested_instance_moved_in_3d() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, _) = open(&family_with(""), "Movers");
    view(&mut h, Some(Which::Plan));
    let path = dir.path().join("Small.json");
    std::fs::write(
        &path,
        serde_json::json!({"name": "Small", "category": "generic", "types": {"Short": {}},
        "forms": [{"profile": [[0, 0], [100, 0], [100, 100], [0, 100]], "to": 100}]})
        .to_string(),
    )
    .unwrap();
    let doc = h.state().active_doc().unwrap();
    h.state_mut().nest_family_file(doc, &path);
    h.run();
    shortcut(&mut h, "CM");
    ed(&mut h).placing = Some(("Small".into(), "Short".into()));
    let pos = editor(&h).views.screen_of([200.0, -400.0, 0.0]).unwrap();
    click_at(&mut h, pos);
    h.run();
    key(&mut h, Key::Escape);
    assert_eq!(editor(&h).nested_instances.len(), 1);
    view(&mut h, None);
    for _ in 0..4 {
        h.run();
    }
    pick(&mut h, &[Picked::Nested(0)]);
    let at = family(&h).nested_instances[0].at.clone();
    shortcut(&mut h, "MV");
    click3(&mut h, [0.0, 0.0, 0.0]);
    click3(&mut h, [300.0, 0.0, 0.0]);
    let now = family(&h).nested_instances[0].at.clone();
    eprintln!("nested {:?} -> {:?}: {}", at.iter().map(num).collect::<Vec<_>>(), now.iter().map(num).collect::<Vec<_>>(), status(&h));
    assert!((num(&now[0]) - num(&at[0]) - 300.0).abs() < 2.0);
}

#[test]
fn probe_primary_drag_with_a_tool_orbits_and_places_nothing() {
    let mut h = in_3d_of(&family_with(""));
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "MV");
    let cam = editor(&h).orbit.camera;
    let rect = editor(&h).orbit.rect;
    let (a, b) = (rect.center(), rect.center() + egui::vec2(200.0, 60.0));
    // A slow drag: many frames.
    h.hover_at(a);
    h.run();
    h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run();
    for t in 1..=30 {
        h.hover_at(a + (b - a) * (t as f32 / 30.0));
        h.run();
    }
    h.event(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run();
    let moved = editor(&h).orbit.camera != cam;
    eprintln!("slow drag: camera moved {moved}, tool {:?}", tool(&h));
    assert!(moved, "the drag turned the view");
    assert_eq!(tool(&h), Some(M::Move { copy: false, from: None }), "and placed no point");
    // A quick flick: a click where it went down, the view as it was (as selecting).
    let cam = editor(&h).orbit.camera;
    flick(&mut h, a, b, Modifiers::NONE);
    h.run();
    eprintln!("flick: camera moved {}, tool {:?}", editor(&h).orbit.camera != cam, tool(&h));
}

#[test]
fn probe_upright_screenshots() {
    let Ok(dir) = std::env::var("BIMCRAFT_FE24_DUMP") else { return };
    for (name, keys, first, then) in
        [("move-upright", "MV", [0.0, 0.0, 0.0], [200.0, 0.0, 500.0]), ("mirror-upright", "DM", [0.0, 0.0, 0.0], [0.0, 0.0, 500.0])]
    {
        let mut h = app(&family_with(""));
        h.set_size(vec2(1400.0, 900.0));
        h.state_mut().edit_family("Movers");
        h.ctx.set_theme(egui::ThemePreference::Light);
        for _ in 0..4 {
            h.run();
        }
        view(&mut h, None);
        for _ in 0..4 {
            h.run();
        }
        ed(&mut h).views.set_work_plane(bimcraft_core::family::PlaneRef::Plane(g(FRONT_BACK)));
        pick(&mut h, &[Picked::Form(0)]);
        shortcut(&mut h, keys);
        for _ in 0..2 {
            h.run();
        }
        click3(&mut h, first);
        let hover = h.state().family_screen3(then).expect("{3D} drawn");
        h.hover_at(hover);
        for _ in 0..3 {
            h.run();
        }
        h.render().unwrap().save(std::path::Path::new(&dir).join(format!("fe24-{name}-light.png"))).unwrap();
    }
}
