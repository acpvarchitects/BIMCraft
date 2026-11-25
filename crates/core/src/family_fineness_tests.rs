//! GEO-2c-1: the chord follows the zoom. A [`Fineness`] says how near a family's arcs, circles and revolves are drawn to the
//! curve; the default builds as before, bit for bit, and a plan asks for about half a pixel at its scale.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::engine::Session;
use crate::family::{Family, Fineness, Piece, Val};
use crate::geom::P2;
use crate::render::{self, Prim, View};

fn session(forms: Vec<Value>) -> Session {
    let mut s = Session::new();
    s.execute(
        "family",
        &json!({"name": "Round", "category": "generic", "parameters": [{"name": "R", "value": 600, "instance": true}],
                "types": {"T": {}}, "forms": forms}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    s
}

fn family(forms: Vec<Value>) -> (Family, BTreeMap<String, Val>) {
    let s = session(forms);
    let f = s.model.families["Round"].clone();
    let values = f.values("T", &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"));
    (f, values)
}

/// A disc, an arched slab (two arcs and two lines) and a turned knob: every way a family chords a curve.
fn curved() -> Vec<Value> {
    vec![
        json!({"sketch": [{"centre": [0, 0], "radius": "R"}], "from": 0, "to": 100}),
        json!({"sketch": [
            {"start": [1000, 0], "end": [3000, 0], "turn": 180},
            {"start": [3000, 0], "end": [2800, 0]},
            {"start": [2800, 0], "end": [1200, 0], "turn": -180},
            {"start": [1200, 0], "end": [1000, 0]}], "from": 0, "to": 200}),
        json!({"sketch": [
            {"start": [0, 0], "end": [300, 0]}, {"start": [300, 0], "end": [300, 400]},
            {"start": [300, 400], "end": [0, 400]}, {"start": [0, 400], "end": [0, 0]},
            {"start": [0, 0], "end": [0, 100], "axis": true}],
            "host": {"plane": "9e3c0a10-1f0b-4d8c-8a5e-0000000000a2"}, "from": 0, "to": 360}),
    ]
}

fn tris(p: &[Piece]) -> usize {
    p.iter().map(|x| x.tris.len()).sum()
}

fn volume(p: &[Piece]) -> f64 {
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    p.iter().flat_map(|x| x.tris.iter()).map(|[a, b, c]| dot(*a, cross(*b, *c)) / 6.0).sum()
}

#[test]
fn the_default_and_a_coarse_view_build_bit_for_bit_as_before() {
    let (f, v) = family(curved());
    let before = f.pieces(&v).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(f.pieces_fine(&v, Fineness::DEFAULT).unwrap_or_else(|e| panic!("{e}")), before);
    // Far out, the default's 32 chords a turn are already finer than a pixel: nothing changes.
    for scale in [0.001, 0.01, 0.05] {
        assert_eq!(f.pieces_fine(&v, Fineness::for_view(scale)).unwrap_or_else(|e| panic!("{e}")), before, "at {scale} px/mm");
    }
}

#[test]
fn a_finer_view_draws_more_chords_nearer_the_curve() {
    let (f, v) = family(curved());
    let exact = std::f64::consts::PI * 600.0 * 600.0 * 100.0
        + std::f64::consts::PI / 2.0 * (1000.0 * 1000.0 - 800.0 * 800.0) * 200.0
        + std::f64::consts::PI * 300.0 * 300.0 * 400.0;
    let default = (volume(&f.pieces(&v).unwrap_or_else(|e| panic!("{e}"))) - exact).abs();
    let mut last = (0, f64::INFINITY);
    for scale in [0.05, 0.5, 2.0, 8.0] {
        let p = f.pieces_fine(&v, Fineness::for_view(scale)).unwrap_or_else(|e| panic!("{e}"));
        let (n, off) = (tris(&p), (volume(&p) - exact).abs());
        assert!(n >= last.0 && off <= last.1, "at {scale} px/mm: {n} triangles {off} off, after {last:?}");
        last = (n, off);
    }
    // Within 1/16 mm of each curve (some 200 chords a turn for the disc, against 32), the volume is some 40 times nearer.
    assert!(last.1 < default / 30.0 && last.1 < exact * 2e-4, "{last:?}, the default {default} off");
    // Each band builds the same every time: what a cache keys on.
    assert_eq!(f.pieces_fine(&v, Fineness::for_view(1.5)), f.pieces_fine(&v, Fineness::for_view(1.9)));
}

#[test]
fn however_near_a_view_comes_a_chord_count_is_capped() {
    let (f, v) = family(vec![json!({"sketch": [{"centre": [0, 0], "radius": 1e6}], "from": 0, "to": 100})]);
    let p = f.pieces_fine(&v, Fineness::for_view(1e9)).unwrap_or_else(|e| panic!("{e}"));
    let ring = p.iter().map(|x| x.outline.len()).max().unwrap_or(0);
    assert!((32..=Fineness::MOST).contains(&ring), "{ring}");
    for hostile in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
        assert_eq!(Fineness::for_view(hostile), Fineness::DEFAULT);
    }
}

/// A 20 m round column 3 m tall, in plan: the drawn edges' midpoints within a pixel of its circle, over the part of it in
/// view, fitted and at 50 times that.
#[test]
fn a_twenty_metre_arc_in_plan_is_within_a_pixel_fitted_and_at_fifty_times() {
    let r = 20_000.0;
    let mut s = session(vec![json!({"sketch": [{"centre": [0, 0], "radius": r}], "from": 0, "to": 3000})]);
    let c = s.execute("component", &json!({"family": "Round", "at": [0, 0]})).unwrap_or_else(|e| panic!("{e}"));
    let c = c["id"].as_u64().unwrap_or_else(|| panic!("{c}"));
    let fitted = 1000.0 / (2.2 * r);
    for (scale, cx) in [(fitted, 0.0), (fitted * 50.0, r)] {
        let view = View { angle: 0.0, scale, cx, cy: 0.0, width: 1000, height: 800 };
        let d = render::draw(&s.model, 100000, view);
        let model = |(x, y): (f32, f32)| P2(cx + (f64::from(x) - 500.0) / scale, -(f64::from(y) - 400.0) / scale);
        let inside = |(x, y): (f32, f32)| (0.0..=1000.0).contains(&x) && (0.0..=800.0).contains(&y);
        let mut worst: f64 = 0.0;
        let mut seen = 0;
        for (p, _) in d.prims.iter().zip(&d.owners).filter(|(_, o)| o.is_some_and(|o| o == c)) {
            let rings: Vec<Vec<(f32, f32)>> = match p {
                Prim::Fill { rings, .. } => rings.clone(),
                Prim::Line { pts, .. } => vec![pts.clone()],
                _ => continue,
            };
            for ring in rings {
                for w in ring.windows(2).filter(|w| inside(w[0]) && inside(w[1])) {
                    let (a, b) = (model(w[0]), model(w[1]));
                    let mid = P2((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                    worst = worst.max((r - mid.dist(P2(0.0, 0.0))).abs() * scale);
                    seen += 1;
                }
            }
        }
        assert!(seen > 0, "nothing of it in view at {scale} px/mm");
        assert!(worst < 1.0, "a chord {worst} px off the circle at {scale} px/mm");
    }
}

/// Two 20 m columns, one in view and one a kilometre off it: the plan's cache builds the first finer, the second as always.
#[test]
fn a_plan_builds_finer_only_what_it_may_show() {
    let mut s = session(vec![json!({"sketch": [{"centre": [0, 0], "radius": 20000}], "from": 0, "to": 3000})]);
    let near = s.execute("component", &json!({"family": "Round", "at": [0, 0]})).unwrap_or_else(|e| panic!("{e}"));
    let far = s.execute("component", &json!({"family": "Round", "at": [1e6, 0], "params": {"R": 601}})).unwrap_or_else(|e| panic!("{e}"));
    let component = |v: &Value| match s.model.elements.get(v["id"].as_u64().unwrap_or_else(|| panic!("{v}"))) {
        Some(crate::model::Element::Component(c)) => c.clone(),
        _ => panic!("{v}"),
    };
    let (near, far) = (component(&near), component(&far));
    let view = View { angle: 0.0, scale: 1.0, cx: 20_000.0, cy: 0.0, width: 1000, height: 800 };
    let mut cache = crate::component_geometry::Components::for_view(&s.model, &view);
    assert!(tris(&cache.pieces(&near)) > tris(&s.model.component_pieces(&near)), "in view: finer");
    assert_eq!(cache.pieces(&far), s.model.component_pieces(&far), "off it: as always");
    // Its heights, which say whether a plan shows it, are the default's.
    let mut plain = crate::component_geometry::Components::new(&s.model);
    assert_eq!(cache.heights(&near), plain.heights(&near));
}

/// A 20 m disc with two 6 m round voids: within the cutting's limits at the default, past them however little finer.
fn holed() -> Vec<Value> {
    let mut forms = vec![json!({"sketch": [{"centre": [0, 0], "radius": 20000}], "from": 0, "to": 3000})];
    for x in [-10000, 10000] {
        forms.push(json!({"sketch": [{"centre": [x, 0], "radius": 6000}], "from": 1000, "to": 4000, "void": true}));
    }
    forms
}

#[test]
fn a_build_too_fine_to_cut_falls_back_to_the_default() {
    let (f, v) = family(holed());
    let before = f.pieces(&v).unwrap_or_else(|e| panic!("{e}"));
    assert!(Fineness::for_view(8.0).scoped(|| f.pieces(&v)).is_err(), "too much to cut that finely");
    assert_eq!(f.pieces_fine(&v, Fineness::for_view(8.0)).unwrap_or_else(|e| panic!("{e}")), before, "built as always");
    // In a plan at 50 times, the component is drawn as always: never an error, never nothing.
    let mut s = session(holed());
    let c = s.execute("component", &json!({"family": "Round", "at": [0, 0]})).unwrap_or_else(|e| panic!("{e}"));
    let id = c["id"].as_u64().unwrap_or_else(|| panic!("{c}"));
    let Some(crate::model::Element::Component(c)) = s.model.elements.get(id) else { panic!("{c}") };
    let view = View { angle: 0.0, scale: 50.0 * 1000.0 / 44_000.0, cx: 20_000.0, cy: 0.0, width: 1000, height: 800 };
    let mut cache = crate::component_geometry::Components::for_view(&s.model, &view);
    assert_eq!(cache.pieces(c), s.model.component_pieces(c));
    assert!(render::draw(&s.model, 100000, view).owners.contains(&Some(id)), "drawn");
}

/// Under a fineness, every curve a build chords is chorded at it: a nested family's, a void's and a revolve's.
#[test]
fn a_fineness_reaches_every_curve_a_build_chords() {
    let mut s = session(vec![json!({"profile": [[0, 0], [10, 0], [10, 10], [0, 10]], "to": 10})]);
    let disc = json!({"category": "generic", "types": {"T": {}},
        "forms": [{"sketch": [{"centre": [0, 0], "radius": 2000}], "from": 0, "to": 100}]});
    for (k, p) in [
        ("nest", json!({"definition": disc, "name": "Disc"})),
        ("nested_instance", json!({"family": "Disc", "type": "T", "at": [5000, 0]})),
    ] {
        s.execute("family", &json!({"name": "Round", k: p})).unwrap_or_else(|e| panic!("{e}"));
    }
    let f = s.model.families["Round"].clone();
    let v = f.values("T", &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"));
    let nested = |p: Vec<Piece>| p.into_iter().filter(|p| p.tris.len() > 12).map(|p| p.tris.len()).sum::<usize>();
    let (coarse, fine) = (nested(f.pieces(&v).unwrap()), nested(f.pieces_fine(&v, Fineness::for_view(2.0)).unwrap()));
    assert!(coarse > 0 && fine > 2 * coarse, "the nested disc: {coarse} triangles, then {fine}");
    // A round void through a disc: its hole too.
    let (f, v) = family(vec![
        json!({"sketch": [{"centre": [0, 0], "radius": 2000}], "from": 0, "to": 100}),
        json!({"sketch": [{"centre": [0, 0], "radius": 1000}], "from": -10, "to": 110, "void": true}),
    ]);
    let ring = |p: &[Piece]| p[0].tris.iter().flatten().filter(|q| ((q[0] * q[0] + q[1] * q[1]).sqrt() - 1000.0).abs() < 1.0).count();
    let (coarse, fine) = (ring(&f.pieces(&v).unwrap()), ring(&f.pieces_fine(&v, Fineness::for_view(2.0)).unwrap()));
    assert!(coarse > 0 && fine > 2 * coarse, "the hole: {coarse} corners on it, then {fine}");
    // And a revolve: the knob of `curved`, alone.
    let (f, v) = family(curved().split_off(2));
    assert!(tris(&f.pieces_fine(&v, Fineness::for_view(2.0)).unwrap()) > 2 * tris(&f.pieces(&v).unwrap()));
}

/// A zoomed plan drawn again with nothing changed builds nothing finer: what it built finely is kept across drawings, for as
/// long as the families are what they were. An edit of one component builds that one again; an edit of the family, all of it.
#[test]
fn a_zoomed_plan_drawn_again_builds_nothing_finer() {
    let mut s = session(vec![json!({"sketch": [{"centre": [0, 0], "radius": "R"}], "from": 0, "to": 3000})]);
    let ids: Vec<u64> = [0.0, 2000.0, 4000.0]
        .iter()
        .enumerate()
        .map(|(k, x)| {
            let v = s
                .execute("component", &json!({"family": "Round", "at": [x, 0], "params": {"R": 600 + 10 * k}}))
                .unwrap_or_else(|e| panic!("{e}"));
            v["id"].as_u64().unwrap_or_else(|| panic!("{v}"))
        })
        .collect();
    let view = View { angle: 0.0, scale: 0.4, cx: 2000.0, cy: 0.0, width: 2000, height: 800 };
    let built = |s: &Session| {
        let before = crate::component_geometry::fine_builds();
        render::draw(&s.model, 100000, view);
        crate::component_geometry::fine_builds() - before
    };
    assert_eq!(built(&s), 3, "the first drawing builds each finer");
    assert_eq!(built(&s), 0, "drawn again, none");
    s.execute("modify", &json!({"id": ids[1], "params": {"R": 650}})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(built(&s), 1, "the one changed");
    s.execute("family", &json!({"name": "Round", "forms": [{"sketch": [{"centre": [0, 0], "radius": "R"}], "from": 0, "to": 2000}]}))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(built(&s), 3, "the family changed: all of them");
    // A shape too fine to cut is remembered as such: drawn again, it isn't tried again.
    let mut s = session(holed());
    s.execute("component", &json!({"family": "Round", "at": [0, 0]})).unwrap_or_else(|e| panic!("{e}"));
    let view = View { angle: 0.0, scale: 2.0, cx: 20_000.0, cy: 0.0, width: 1000, height: 800 };
    let tried = |s: &Session| {
        let before = crate::component_geometry::fine_builds();
        render::draw(&s.model, 100000, view);
        crate::component_geometry::fine_builds() - before
    };
    assert_eq!((tried(&s), tried(&s)), (1, 0));
}

/// ec's review: a model gone (closed, or a family editor's drag making a family a step at a time) leaves nothing kept: its
/// generation goes at the next keep, so dead shapes neither hold the budget nor push out a live model's. And the kept bytes are
/// counted as they go, never summed again under the lock.
#[test]
fn a_model_gone_keeps_nothing_and_the_bytes_kept_are_counted_as_they_go() {
    let view = View { angle: 0.0, scale: 0.4, cx: 0.0, cy: 0.0, width: 2000, height: 800 };
    let gone = {
        let mut s = session(vec![json!({"sketch": [{"centre": [0, 0], "radius": "R"}], "from": 0, "to": 3000})]);
        s.execute("component", &json!({"family": "Round", "at": [0, 0], "params": {"R": 611}})).unwrap_or_else(|e| panic!("{e}"));
        render::draw(&s.model, 100000, view);
        let id = s.model.families.identity();
        assert!(crate::component_geometry::keeps(&id), "drawn finer, kept");
        id
    };
    let mut s = session(vec![json!({"sketch": [{"centre": [0, 0], "radius": "R"}], "from": 0, "to": 3000})]);
    s.execute("component", &json!({"family": "Round", "at": [0, 0], "params": {"R": 612}})).unwrap_or_else(|e| panic!("{e}"));
    render::draw(&s.model, 100000, view);
    assert!(!crate::component_geometry::keeps(&gone), "the model gone, its shapes are");
    assert!(crate::component_geometry::keeps(&s.model.families.identity()));
    let (counted, summed) = crate::component_geometry::kept_bytes();
    assert_eq!(counted, summed, "the running total is what is kept");
}

/// GEO-2f's facade panel (an L, a knob turned about an axis, a blend and a round through-void), `n` of them, each its own
/// size, in a grid 1.5 m apart: one placed by the command, the rest copies of it put straight in (the command checks the whole
/// model each time). Its side, panels.
fn facade(n: usize) -> (Session, usize) {
    let line = |a: Value, b: Value| json!({"start": a, "end": b});
    let l = vec![
        line(json!([0, 0]), json!(["W", 0])),
        line(json!(["W", 0]), json!(["W", 150])),
        line(json!(["W", 150]), json!([150, 150])),
        line(json!([150, 150]), json!([150, "H"])),
        line(json!([150, "H"]), json!([0, "H"])),
        line(json!([0, "H"]), json!([0, 0])),
    ];
    let knob = vec![
        line(json!(["W / 2 - R", 0]), json!(["W / 2 - R / 2", 0])),
        line(json!(["W / 2 - R / 2", 0]), json!(["W / 2 - R / 2", 80])),
        line(json!(["W / 2 - R / 2", 80]), json!(["W / 2 - R", 80])),
        line(json!(["W / 2 - R", 80]), json!(["W / 2 - R", 0])),
        json!({"start": ["W / 2", 0], "end": ["W / 2", 100], "axis": true}),
    ];
    let square: Vec<Value> = [(-60, -60, 60, -60), (60, -60, 60, 60), (60, 60, -60, 60), (-60, 60, -60, -60)]
        .iter()
        .map(|(a, b, c, d)| line(json!([a, b]), json!([c, d])))
        .collect();
    let mut s = Session::new();
    s.execute(
        "family",
        &json!({"name": "Panel", "category": "generic",
            "parameters": [{"name": "W", "value": 1200, "instance": true}, {"name": "H", "value": 900, "instance": true},
                           {"name": "D", "value": 300, "instance": true}, {"name": "R", "value": 120, "instance": true}],
            "types": {"T": {}}, "forms": [
                {"sketch": l, "from": 0, "to": "D"},
                {"sketch": knob, "host": {"plane": "9e3c0a10-1f0b-4d8c-8a5e-0000000000a2"}, "from": 0, "to": 360},
                {"sketch": square, "top": [{"centre": [0, 0], "radius": 30}], "from": "D", "to": "D + 200"},
                {"sketch": [{"centre": [75, "H / 2"], "radius": "R / 3"}], "from": -10, "to": "D + 10", "void": true}]}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let side = (n as f64).sqrt().ceil() as usize;
    let first = s.execute("component", &json!({"family": "Panel", "at": [0, 0]})).unwrap_or_else(|e| panic!("{e}"));
    let Some(crate::model::Element::Component(first)) =
        s.model.elements.get(first["id"].as_u64().unwrap_or_else(|| panic!("{first}"))).cloned()
    else {
        panic!("{first}")
    };
    let model = std::sync::Arc::make_mut(&mut s.model);
    for i in 0..n {
        let k = i as f64;
        let mut c = first.clone();
        let params = [
            ("W", 1000.0 + (k % 97.0) * 3.1),
            ("H", 800.0 + (k % 89.0) * 2.3),
            ("D", 250.0 + (k % 13.0) * 7.0),
            ("R", 100.0 + (k % 11.0) * 4.0),
        ];
        c.params = params.iter().map(|(n, v)| (n.to_string(), json!(v))).collect();
        c.at = P2((i % side) as f64 * 1500.0, (i / side) as f64 * 1500.0);
        model.insert(crate::model::Element::Component(c));
    }
    (s, side)
}

/// FE-16c-2 (cp's review of #91): the facade seen in an elevation looking north at the panels' fronts, unpainted and with every
/// panel's front painted a material of a surface pattern: the section's picture drawn, then again, on 8 workers. `cargo test
/// --release -p bimcraft-core elevation_paint_bench -- --ignored --nocapture`.
#[test]
#[ignore]
fn elevation_paint_bench() {
    let n: usize = std::env::var("GEO_PANELS").ok().and_then(|v| v.parse().ok()).unwrap_or(10_000);
    render::without_twins(|| {
        crate::component_geometry::with_threads(8, || {
            for painted in [false, true] {
                let (mut s, side) = facade(n);
                if painted {
                    s.execute("fill_pattern", &json!({"name": "Tiles", "target": "model", "lines": [{"angle": 0, "spacing": 200}, {"angle": 90, "spacing": 200}]}))
                        .unwrap_or_else(|e| panic!("{e}"));
                    s.execute("material", &json!({"name": "Tile", "color": [200, 200, 200], "surface": "Tiles"}))
                        .unwrap_or_else(|e| panic!("{e}"));
                    let form = s.model.families["Panel"].forms[0].guid;
                    let ids: Vec<u64> = s.model.elements.iter().filter(|(_, e)| e.category() == "component").map(|(id, _)| *id).collect();
                    // Each panel's front, along x at y = 0: its key, as a click on the first panel's names it.
                    let first = ids
                        .iter()
                        .copied()
                        .find(|id| matches!(s.model.elements.get(*id), Some(crate::model::Element::Component(c)) if c.at == P2(0.0, 0.0)))
                        .unwrap_or_else(|| panic!("the first panel"));
                    let face =
                        s.model.component_face(first, [500.0, 0.0, 100.0], [0.0, -1.0, 0.0]).unwrap_or_else(|| panic!("its front")).face;
                    for some in ids.chunks(5000) {
                        s.execute("paint", &json!({"ids": some, "form": form, "face": face, "material": "Tile"}))
                            .unwrap_or_else(|e| panic!("{e}"));
                    }
                }
                let width = side as f64 * 1500.0;
                let id = s
                    .execute("section", &json!({"start": [-1000, -1000], "end": [width + 1000.0, -1000], "depth": 2000}))
                    .unwrap_or_else(|e| panic!("{e}"))["id"]
                    .as_u64()
                    .unwrap_or_else(|| panic!("a section"));
                let view = View { angle: 0.0, scale: 0.065, cx: width / 2.0 + 1000.0, cy: 500.0, width: 2000, height: 800 };
                let work = render::SectionWork::new(&s.model);
                for what in ["first", "again"] {
                    let t = std::time::Instant::now();
                    let picture = render::section_picture_with(&s.model, &work, id, view, &[]);
                    let elapsed = t.elapsed().as_secs_f64() * 1000.0;
                    // The pattern's ink: the pixels in its grey.
                    let inked = picture.as_ref().map_or(0, |p| p.rgba.chunks(4).filter(|c| c[..3] == [120, 120, 120]).count());
                    eprintln!(
                        "FE-16c-2 elevation, {}, {what}: {elapsed:.1} ms, {inked} px in the pattern's grey{}",
                        if painted { "every front painted" } else { "unpainted" },
                        if picture.is_some() { "" } else { " (no picture)" }
                    );
                }
            }
        })
    });
}

/// The process's peak memory and what it holds now, from `/proc/self/status` (none where there is no such file).
fn memory() -> String {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| status.lines().find(|l| l.starts_with(name)).map_or(String::from("?"), |l| l[name.len()..].trim().to_string());
    format!("peak {}, now {}", field("VmHWM:"), field("VmRSS:"))
}

/// GEO-2c-2a's cost in an elevation: the facade of `GEO_PANELS` panels (10,000 unless it says) seen from the north as
/// `elevation_paint_bench` sees it, drawn as the desktop draws it (its picture and the drawing over it, of the work the desktop
/// keeps for the model): zoomed to fit, first and again; zoomed in on a panel or two at 1.5 px a mm, first, again, panned half a
/// screen, zoomed within the band and into the next. Each with what it built and the process's peak memory. `cargo test
/// --release -p bimcraft-core section_fineness_bench -- --ignored --nocapture`.
#[test]
#[ignore]
fn section_fineness_bench() {
    let n: usize = std::env::var("GEO_PANELS").ok().and_then(|v| v.parse().ok()).unwrap_or(10_000);
    let cg = crate::component_geometry::builds;
    render::without_twins(|| {
        let (mut s, side) = facade(n);
        eprintln!("GEO-2c-2a {n} panels made: {}", memory());
        let width = side as f64 * 1500.0;
        let id = s
            .execute("section", &json!({"start": [-1000, -1000], "end": [width + 1000.0, -1000], "depth": 2000}))
            .unwrap_or_else(|e| panic!("{e}"))["id"]
            .as_u64()
            .unwrap_or_else(|| panic!("a section"));
        let t = std::time::Instant::now();
        let work = render::SectionWork::new_deferred(&s.model);
        eprintln!("GEO-2c-2a elevation, the model's work: {:.1} ms, {}", t.elapsed().as_secs_f64() * 1000.0, memory());
        let fit = render::section_fit_with(&s.model, &work, id, 2000, 800);
        let time = |what: &str, v: View| {
            let (before, t) = (cg(), std::time::Instant::now());
            let (picture, d) = render::section_picture_and_over_with(&s.model, &work, id, v, v, &[]);
            eprintln!(
                "GEO-2c-2a elevation, {what} ({:.3} px/mm): {:.1} ms, {} built, {} primitives{}, {}",
                v.scale,
                t.elapsed().as_secs_f64() * 1000.0,
                cg() - before,
                d.prims.len(),
                if picture.is_some() { "" } else { " (no picture)" },
                memory()
            );
        };
        time("fitted, first", fit);
        time("fitted, again", fit);
        // A panel or two in the middle of the front row, its knob's chords finer than the default's from 1 px a mm.
        let cx = 1000.0 + (side / 2) as f64 * 1500.0 + 600.0;
        let near = |scale: f64, cx: f64| View { angle: 0.0, scale, cx, cy: 250.0, width: 2000, height: 800 };
        time("zoomed in, first", near(1.5, cx));
        time("zoomed in, again", near(1.5, cx));
        time("zoomed in, panned half a screen", near(1.5, cx + 1000.0 / 1.5));
        time("zoomed in within the band", near(1.9, cx));
        time("zoomed into the next band", near(3.0, cx));
        time("zoomed into the next band, again", near(3.0, cx));
        let (bytes, _) = crate::component_geometry::kept_bytes();
        eprintln!(
            "GEO-2c-2a elevation: kept {:.1} MB at the default, {:.1} MB finer; {}",
            bytes[0] as f64 / 1e6,
            bytes[1] as f64 / 1e6,
            memory()
        );
    });
}

/// FE-16c-2: facade `s`'s panels, every `every`th, painted on their top (the end of their first form) a material whose surface
/// is a model pattern of tiles 200 mm square, as one command paints them.
fn painted_facade(s: &mut Session, every: usize) {
    s.execute(
        "fill_pattern",
        &json!({"name": "Tiles", "target": "model", "lines": [{"angle": 0, "spacing": 200}, {"angle": 90, "spacing": 200}]}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    s.execute("material", &json!({"name": "Tile", "color": [200, 200, 200], "surface": "Tiles"})).unwrap_or_else(|e| panic!("{e}"));
    let form = s.model.families["Panel"].forms[0].guid;
    let ids: Vec<u64> = s.model.elements.iter().filter(|(_, e)| e.category() == "component").map(|(id, _)| *id).step_by(every).collect();
    for some in ids.chunks(5000) {
        s.execute("paint", &json!({"ids": some, "form": form, "face": "end", "material": "Tile"})).unwrap_or_else(|e| panic!("{e}"));
    }
}

/// GEO-2c and GEO-2g's cost in a plan: `GEO_PANELS` facade panels (10,000 unless it says), a plan zoomed in on a few hundred
/// drawn first, again, panned by half a screen, and with one panel in view edited; and the plan zoomed to fit them all, first
/// and again. On 8 workers and on as many as the machine has; each plan drawn once (no test twins). `cargo test --release -p
/// bimcraft-core zoomed_plan_bench -- --ignored --nocapture`.
#[test]
#[ignore]
fn zoomed_plan_bench() {
    let n: usize = std::env::var("GEO_PANELS").ok().and_then(|v| v.parse().ok()).unwrap_or(10_000);
    let all = std::thread::available_parallelism().map_or(1, usize::from);
    let cg = crate::component_geometry::builds;
    // Room for both models' default builds (each some 300 MB), so drawing the second again builds nothing the first pushed out.
    let budgets = [768 << 20, 256 << 20];
    render::without_twins(|| {
        crate::component_geometry::with_budgets(budgets, || {
            for threads in [8, all] {
                crate::component_geometry::with_threads(threads, || {
                    let (mut s, side) = facade(n);
                    let middle = (side / 2) as f64 * 1500.0;
                    // Some 20 by 8 panels in a 2000 x 800 screen, at a scale finer than the default draws.
                    let view = |cx: f64| View { angle: 0.0, scale: 0.065, cx, cy: middle, width: 2000, height: 800 };
                    let time = |what: &str, s: &Session, v: View| {
                        let (before, t) = (cg(), std::time::Instant::now());
                        let d = render::draw(&s.model, 100000, v);
                        eprintln!(
                            "GEO-2g {threads} workers, {what}: {:.1} ms, {} built, {} primitives",
                            t.elapsed().as_secs_f64() * 1000.0,
                            cg() - before,
                            d.prims.len()
                        );
                    };
                    time("first zoomed draw", &s, view(middle));
                    time("drawn again", &s, view(middle));
                    time("panned half a screen", &s, view(middle + 1000.0 / 0.065));
                    // One panel in view, edited as a command edits it: a new model, the families as they were.
                    let id = s
                        .model
                        .elements
                        .iter()
                        .find_map(|(id, e)| match e {
                            crate::model::Element::Component(c) if c.at.0 >= middle + 1000.0 / 0.065 && c.at.1 >= middle => Some(*id),
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("a panel in view"));
                    s.execute("modify", &json!({"id": id, "params": {"W": 1234.5}})).unwrap_or_else(|e| panic!("{e}"));
                    time("one panel in view edited", &s, view(middle + 1000.0 / 0.065));
                    // Fitted as another model of the same panels is: what fitting builds isn't this one's.
                    let fit = render::plan_fit(&facade(n).0.model, 100000, 2000, 1200, false);
                    let (fresh, _) = facade(n);
                    time("first draw zoomed to fit", &fresh, fit);
                    time("zoomed to fit drawn again", &fresh, fit);
                    let (shared, own) = crate::component_geometry::kept_finer();
                    eprintln!("GEO-2g {threads} workers: {shared} finer builds kept as their default ones, {own} of their own");
                    let (bytes, _) = crate::component_geometry::kept_bytes();
                    eprintln!(
                        "GEO-2g {threads} workers: kept {:.1} MB at the default, {:.1} MB finer",
                        bytes[0] as f64 / 1e6,
                        bytes[1] as f64 / 1e6
                    );
                    // FE-16c-2: the same zoomed plan at Fine detail with every tenth panel's top painted a material of a
                    // surface pattern, and with every panel's: each painted top's pattern drawn over it.
                    for (what, every) in [("10% painted", 10), ("every panel painted", 1)] {
                        let (mut s, _) = facade(n);
                        painted_facade(&mut s, every);
                        let edit = |s: &mut Session| {
                            let id = s
                                .model
                                .elements
                                .iter()
                                .find_map(|(id, e)| match e {
                                    crate::model::Element::Component(c) if c.at.0 >= middle + 1000.0 / 0.065 && c.at.1 >= middle => {
                                        Some(*id)
                                    }
                                    _ => None,
                                })
                                .unwrap_or_else(|| panic!("a panel in view"));
                            s.execute("modify", &json!({"id": id, "params": {"W": 1234.5}})).unwrap_or_else(|e| panic!("{e}"));
                        };
                        time(&format!("{what}, first zoomed draw"), &s, view(middle));
                        time(&format!("{what}, drawn again"), &s, view(middle));
                        time(&format!("{what}, panned half a screen"), &s, view(middle + 1000.0 / 0.065));
                        edit(&mut s);
                        time(&format!("{what}, one panel in view edited"), &s, view(middle + 1000.0 / 0.065));
                    }
                });
            }
        })
    });
}

/// cp's review of #88: what a default build records of its curves says, for each fineness, whether a build at it would be any
/// different, bit for bit: straight lines (which no fineness changes), a revolve, an arc and a circle, small and large, across
/// finenesses from far out to close in. A family's sketch has no ellipses or splines: [`crate::family::fineness::steps`] is
/// the only curve a fineness changes.
#[test]
fn a_default_build_says_exactly_which_finenesses_would_build_it_otherwise() {
    let line = |a: Value, b: Value| json!({"start": a, "end": b});
    let knob = |r: f64| {
        vec![
            line(json!([r, 0]), json!([2.0 * r, 0])),
            line(json!([2.0 * r, 0]), json!([2.0 * r, 80])),
            line(json!([2.0 * r, 80]), json!([r, 80])),
            line(json!([r, 80]), json!([r, 0])),
            json!({"start": [0, 0], "end": [0, 100], "axis": true}),
        ]
    };
    let cases: Vec<(&str, Vec<Value>)> = vec![
        ("straight lines", vec![json!({"profile": [[0, 0], [4000, 0], [4000, 3000], [0, 3000]], "to": 500})]),
        ("a small revolve", vec![json!({"sketch": knob(60.0), "from": 0, "to": 360})]),
        ("a large part revolve", vec![json!({"sketch": knob(9000.0), "from": 10, "to": 100})]),
        ("a circle", vec![json!({"sketch": [{"centre": [0, 0], "radius": 3000}], "to": 500})]),
        (
            "an arc",
            vec![json!({"sketch": [
                {"start": [0, 0], "end": [20000, 0], "turn": 40}, line(json!([20000, 0]), json!([20000, 3000])),
                line(json!([20000, 3000]), json!([0, 3000])), line(json!([0, 3000]), json!([0, 0]))], "to": 300})],
        ),
    ];
    for (what, forms) in cases {
        let (f, v) = family(forms);
        let (default, needs) = crate::family::fineness::recording(|| f.pieces_fine(&v, Fineness::DEFAULT));
        let default = default.unwrap_or_else(|e| panic!("{what}: {e}"));
        let mut differ = 0;
        for k in -12..=12 {
            let fineness = Fineness::for_view(2f64.powi(k));
            let fine = f.pieces_fine(&v, fineness).unwrap_or_else(|e| panic!("{what}: {e}"));
            assert_eq!(fineness.changes(needs), fine != default, "{what} at {fineness:?}, recorded {needs:?}");
            differ += usize::from(fine != default);
        }
        if what == "straight lines" {
            assert_eq!(differ, 0, "no fineness changes straight lines");
        } else {
            assert!(differ > 0 && differ < 25, "{what}: some finenesses change it, not all ({differ})");
        }
    }
}
