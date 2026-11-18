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
