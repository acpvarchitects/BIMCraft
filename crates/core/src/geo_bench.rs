//! GEO-2f: the mesh path's baseline, what a kernel is measured against (`docs/brep-plan.md`). A facade of panels, every one
//! different (its instance parameters), each an L extrusion, a revolve and a blend, with and without one round through-void,
//! built on all cores and on one: `cargo test --release -p bimcraft-core geo_bench -- --ignored --nocapture`, `GEO_PANELS`
//! panels (1000 unless it says), the times extrapolated to 10,000.

use std::collections::BTreeMap;
use std::time::Instant;

use rayon::prelude::*;
use serde_json::{Value, json};

use crate::engine::Session;
use crate::family::Family;

fn line(a: [Value; 2], b: [Value; 2]) -> Value {
    json!({"start": a, "end": b})
}

/// The panel: `void`, with its round hole through the L.
fn panel(void: bool) -> Family {
    let n = |x: f64| json!(x);
    let l = vec![
        line([n(0.0), n(0.0)], [json!("W"), n(0.0)]),
        line([json!("W"), n(0.0)], [json!("W"), n(150.0)]),
        line([json!("W"), n(150.0)], [n(150.0), n(150.0)]),
        line([n(150.0), n(150.0)], [n(150.0), json!("H")]),
        line([n(150.0), json!("H")], [n(0.0), json!("H")]),
        line([n(0.0), json!("H")], [n(0.0), n(0.0)]),
    ];
    let knob = vec![
        line([json!("W / 2 - R"), n(0.0)], [json!("W / 2 - R / 2"), n(0.0)]),
        line([json!("W / 2 - R / 2"), n(0.0)], [json!("W / 2 - R / 2"), n(80.0)]),
        line([json!("W / 2 - R / 2"), n(80.0)], [json!("W / 2 - R"), n(80.0)]),
        line([json!("W / 2 - R"), n(80.0)], [json!("W / 2 - R"), n(0.0)]),
        json!({"start": [json!("W / 2"), 0], "end": [json!("W / 2"), 100], "axis": true}),
    ];
    let square = |h: f64| {
        vec![
            line([n(-h), n(-h)], [n(h), n(-h)]),
            line([n(h), n(-h)], [n(h), n(h)]),
            line([n(h), n(h)], [n(-h), n(h)]),
            line([n(-h), n(h)], [n(-h), n(-h)]),
        ]
    };
    let mut forms = vec![
        json!({"sketch": l, "from": 0, "to": "D"}),
        json!({"sketch": knob, "host": {"plane": "9e3c0a10-1f0b-4d8c-8a5e-0000000000a2"}, "from": 0, "to": 360}),
        json!({"sketch": square(60.0), "top": [{"centre": [0, 0], "radius": 30}], "from": "D", "to": "D + 200"}),
    ];
    if void {
        forms.push(json!({"sketch": [{"centre": [75, "H / 2"], "radius": "R / 3"}], "from": -10, "to": "D + 10", "void": true}));
    }
    let mut s = Session::new();
    s.execute(
        "family",
        &json!({"name": "Panel", "category": "generic",
            "parameters": [{"name": "W", "value": 1200, "instance": true}, {"name": "H", "value": 900, "instance": true},
                           {"name": "D", "value": 300, "instance": true}, {"name": "R", "value": 120, "instance": true}],
            "types": {"T": {}}, "forms": forms}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    s.model.families["Panel"].clone()
}

/// Panel `i`'s own values: no two alike.
fn values(f: &Family, i: usize) -> BTreeMap<String, crate::family::Val> {
    let k = i as f64;
    let instance: BTreeMap<String, Value> = [
        ("W", json!(1000.0 + (k % 97.0) * 3.1)),
        ("H", json!(800.0 + (k % 89.0) * 2.3)),
        ("D", json!(250.0 + (k % 13.0) * 7.0)),
        ("R", json!(100.0 + (k % 11.0) * 4.0)),
    ]
    .into_iter()
    .map(|(n, v)| (n.to_string(), v))
    .collect();
    f.values("T", &instance).unwrap_or_else(|e| panic!("{e}"))
}

/// Builds `n` panels of `f` on all cores (`parallel`) or one: seconds, and triangles made.
fn build(f: &Family, n: usize, parallel: bool) -> (f64, usize) {
    let t = Instant::now();
    let count = |i: usize| f.pieces(&values(f, i)).map(|p| p.iter().map(|x| x.tris.len()).sum::<usize>()).unwrap_or(0);
    let tris = if parallel { (0..n).into_par_iter().map(count).sum() } else { (0..n).map(count).sum() };
    (t.elapsed().as_secs_f64(), tris)
}

#[test]
#[ignore]
fn geo_bench() {
    let n: usize = std::env::var("GEO_PANELS").ok().and_then(|v| v.parse().ok()).unwrap_or(1000);
    let scale = 10_000.0 / n as f64;
    eprintln!("GEO-2f mesh baseline, {n} unique panels, {} threads; times for 10,000", rayon::current_num_threads());
    for void in [false, true] {
        let f = panel(void);
        let (all, tris) = build(&f, n, true);
        let (one, _) = build(&f, n.min(200), false);
        let ten = {
            let t = Instant::now();
            (0..10).for_each(|i| drop(f.pieces(&values(&f, n + i))));
            t.elapsed().as_secs_f64()
        };
        eprintln!(
            "GEO-2f {}: all cores {:.2} s, one core {:.1} ms a panel, 10 changed {:.1} ms, {} triangles a panel",
            if void { "with a void" } else { "no void    " },
            all * scale,
            one / n.min(200) as f64 * 1e3,
            ten * 1e3,
            tris / n.max(1)
        );
    }
    // Cut probes: a through-hole cut by the mesh csg, against the same hole drawn into the profile.
    let probe = |void: bool| {
        let f = panel(void);
        let v = values(&f, 0);
        let t = Instant::now();
        for _ in 0..50 {
            drop(f.pieces(&v));
        }
        t.elapsed().as_secs_f64() / 50.0 * 1e3
    };
    eprintln!("GEO-2f probe: a panel {:.2} ms without its void, {:.2} ms with it", probe(false), probe(true));
}
