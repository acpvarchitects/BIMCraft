//! FE-15, part 2: a model line in a family's views as a reference, as a reference line is. Aligned Dimension picks it seen
//! edge on (in plan, a line drawn on Ref. Level), and Align moves it onto a plane, its padlock offered; a pinned one stays.
use super::family_views::{click_family, editor, open, view};
use super::*;
use bimcraft_core::family::views::Which;
use bimcraft_core::family::{DimKind, DimRef};
use bimcraft_core::ids::Guid;

const LINE: &str = "6a7b8c9d-0e1f-4a2b-9c3d-4e5f60718293";
const EDGE: &str = "7b8c9d0e-1f2a-4b3c-8d4e-5f6071829304";
const CFB: &str = "9e3c0a10-1f0b-4d8c-8a5e-0000000000a2";

/// A box, a plane Edge along x at y -500, and a model line along x at y -300 on Ref. Level, pinned or not.
fn family(pinned: bool) -> String {
    format!(
        r#"family {{"name": "Lined", "category": "generic", "types": {{"T": {{}}}}, "forms": [{{"profile": [[500, 300], [1000, 300], [1000, 600], [500, 600]], "to": 2000}}], "ref_planes": [{{"guid": "{EDGE}", "name": "Edge", "start": [-1000, -500, 0], "end": [1000, -500, 0], "up": [0, 0, 1]}}], "model_lines": [{{"guid": "{LINE}", "start": [-200, -300, 0], "end": [400, -300, 0], "pinned": {pinned}}}]}}"#
    )
}

fn g(s: &str) -> Guid {
    serde_json::from_value(json!(s)).unwrap()
}

fn ed_mut<'a>(h: &'a mut Harness<'_, BimApp>) -> &'a mut crate::editor::FamilyEditor {
    h.state_mut().active_family_mut().unwrap()
}

fn in_plan(pinned: bool) -> Harness<'static, BimApp> {
    let (mut h, _) = open(&family(pinned), "Lined");
    view(&mut h, Some(Which::Plan));
    h
}

#[test]
fn aligned_dimension_picks_a_model_line_seen_edge_on() {
    let mut h = in_plan(false);
    ed_mut(&mut h).radial_tool(DimKind::Aligned);
    h.run();
    // Center (Front/Back)'s trace, then the model line, then where the dimension goes.
    click_family(&mut h, [-700.0, 0.0, 0.0]);
    click_family(&mut h, [100.0, -300.0, 0.0]);
    assert_eq!(editor(&h).views.dimensioning.as_ref().map(Vec::len), Some(2), "{}", h.state().status.text);
    click_family(&mut h, [700.0, -150.0, 0.0]);
    let d = editor(&h).dimensions.last().cloned().unwrap();
    assert_eq!(d.refs, vec![DimRef::Plane(g(CFB)), DimRef::ModelLine(g(LINE))]);
    let f = editor(&h).draft_family().unwrap();
    let m = f.measured(&editor(&h).values()).unwrap();
    assert!((m.last().unwrap()[0] - 300.0).abs() < 1e-6, "it measures 300: {m:?}");
}

#[test]
fn align_moves_a_model_line_onto_a_plane_and_offers_its_padlock() {
    let mut h = in_plan(false);
    shortcut(&mut h, "AL");
    click_family(&mut h, [-800.0, -500.0, 0.0]);
    click_family(&mut h, [100.0, -300.0, 0.0]);
    let l = editor(&h).model_lines[0].clone();
    assert!((l.start[1] + 500.0).abs() < 1e-6 && (l.end[1] + 500.0).abs() < 1e-6, "on Edge: {l:?} {}", h.state().status.text);
    assert!((l.start[0] + 200.0).abs() < 1e-6 && (l.end[0] - 400.0).abs() < 1e-6, "slid across, not along");
    assert!(editor(&h).views.align_lock.is_some(), "the padlock waits");
    assert!(editor(&h).unsaved(), "a step of the history");
}

#[test]
fn align_leaves_a_pinned_model_line_where_it_is() {
    let mut h = in_plan(true);
    shortcut(&mut h, "AL");
    click_family(&mut h, [-800.0, -500.0, 0.0]);
    click_family(&mut h, [100.0, -300.0, 0.0]);
    let l = editor(&h).model_lines[0].clone();
    assert!((l.start[1] + 300.0).abs() < 1e-6, "pinned, it stays: {l:?}");
    let said = format!("{:?} {}", editor(&h).views.status, h.state().status.text);
    assert!(said.contains("pinned"), "and says why: {said}");
}

/// Aligned Dimension to a model line and Align onto Edge in the three looks, when `BIMCRAFT_FE15B_DUMP` names a folder.
#[test]
fn model_line_reference_screenshots() {
    let Ok(dir) = std::env::var("BIMCRAFT_FE15B_DUMP") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, dark, light_canvas) in [("light", false, false), ("dark", true, false), ("dark-ui-light-canvas", true, true)] {
        let mut h = app(&family(false));
        h.set_size(egui::vec2(1400.0, 900.0));
        h.state_mut().edit_family("Lined");
        h.run();
        h.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
        h.state_mut().light_canvas = light_canvas;
        view(&mut h, Some(Which::Plan));
        ed_mut(&mut h).radial_tool(DimKind::Aligned);
        h.run();
        click_family(&mut h, [-700.0, 0.0, 0.0]);
        click_family(&mut h, [100.0, -300.0, 0.0]);
        click_family(&mut h, [700.0, -150.0, 0.0]);
        key(&mut h, Key::Escape);
        shortcut(&mut h, "AL");
        click_family(&mut h, [-800.0, -500.0, 0.0]);
        click_family(&mut h, [100.0, -300.0, 0.0]);
        for _ in 0..4 {
            h.run();
        }
        h.render().unwrap().save(dir.join(format!("fe15b-dimension-align-{name}.png"))).unwrap();
    }
}
