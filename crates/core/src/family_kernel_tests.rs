//! GEO-1 step 2: the forms are built through `family::kernel`'s `MeshKernel` exactly as they were before it: every piece
//! of every family here, for each of its types, hashed bit for bit (its triangles, caps, outline, holes, heights, roundness,
//! material and Visibility Settings) and held to the hash the code made before the kernel was put between it and the forms.
//! The families: the sample house's, the starter fixtures, and forms of every kind (extrusions with arcs and holes, a
//! revolve, blends, sweeps along a bent path, an arc and a circle, swept blends, and voids and solids cutting them).

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::engine::Session;
use crate::family::Family;

/// FNV-1a, 64 bits: a hash that stays the same from one build, platform and Rust release to the next.
struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for x in b {
            self.0 ^= u64::from(*x);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn f(&mut self, x: f64) {
        self.bytes(&x.to_bits().to_le_bytes());
    }
    fn n(&mut self, x: usize) {
        self.bytes(&(x as u64).to_le_bytes());
    }
}

/// Every piece of family `f` for each of its types, hashed into `h`; a type it can't build, its error.
fn hash_family(h: &mut Fnv, name: &str, f: &Family) {
    h.bytes(name.as_bytes());
    let types: Vec<&String> = f.types.keys().collect();
    for t in types {
        h.bytes(t.as_bytes());
        let Ok(values) = f.values(t, &BTreeMap::new()) else {
            h.bytes(b"no values");
            continue;
        };
        match f.pieces(&values) {
            Err(e) => h.bytes(e.as_bytes()),
            Ok(pieces) => {
                h.n(pieces.len());
                for p in &pieces {
                    h.n(p.tris.len());
                    for t in &p.tris {
                        for q in t {
                            q.iter().for_each(|x| h.f(*x));
                        }
                    }
                    h.n(p.caps);
                    h.n(p.outline.len());
                    p.outline.iter().for_each(|q| {
                        h.f(q.0);
                        h.f(q.1);
                    });
                    h.n(p.holes.len());
                    for hole in &p.holes {
                        h.n(hole.len());
                        hole.iter().for_each(|q| {
                            h.f(q.0);
                            h.f(q.1);
                        });
                    }
                    h.f(p.bottom);
                    h.f(p.top);
                    h.n(usize::from(p.round));
                    h.bytes(p.material.as_bytes());
                    h.bytes(format!("{:?}", p.vis).as_bytes());
                }
            }
        }
    }
}

fn line(a: [f64; 2], b: [f64; 2]) -> Value {
    json!({"start": a, "end": b})
}

fn arc(a: [f64; 2], b: [f64; 2], turn: f64) -> Value {
    json!({"start": a, "end": b, "turn": turn})
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Value> {
    vec![line([x0, y0], [x1, y0]), line([x1, y0], [x1, y1]), line([x1, y1], [x0, y1]), line([x0, y1], [x0, y0])]
}

/// Forms of every kind, each a family of its own with two types.
fn kinds() -> Vec<(&'static str, Value)> {
    let mut rounded = rect(-500.0, -300.0, 300.0, 300.0);
    rounded.truncate(3);
    rounded.push(arc([-500.0, 300.0], [-500.0, -300.0], 180.0));
    let mut holed = rounded;
    holed.push(json!({"centre": [0, 0], "radius": 100}));
    let mut revolve = rect(200.0, 0.0, 500.0, 800.0);
    revolve.push(json!({"start": [0, 0], "end": [0, 100], "axis": true}));
    let bent = vec![line([0.0, 0.0], [2000.0, 0.0]), line([2000.0, 0.0], [2000.0, 1000.0])];
    vec![
        ("extrusion with an arc and a hole", json!([{"sketch": holed, "from": 0, "to": "L"}])),
        ("a circle", json!([{"sketch": [{"centre": [0, 0], "radius": "W"}], "from": 0, "to": 900}])),
        ("revolve, part turn", json!([{"sketch": revolve, "from": 0, "to": 270}])),
        (
            "blend square to circle, twisted",
            json!([{"sketch": rect(-500.0, -500.0, 500.0, 500.0), "top": [{"centre": [0, 0], "radius": 250}], "from": 0, "to": 1000, "twist": 1}]),
        ),
        ("sweep along a bent path", json!([{"sketch": bent, "sweep": {"profile": rect(-100.0, 0.0, 100.0, 200.0)}}])),
        (
            "sweep along an arc",
            json!([{"sketch": [arc([1000.0, 0.0], [0.0, 1000.0], 90.0)], "sweep": {"profile": rect(-50.0, -50.0, 50.0, 50.0)}}]),
        ),
        (
            "sweep round a circle",
            json!([{"sketch": [{"centre": [0, 0], "radius": 1000}], "sweep": {"profile": rect(-50.0, -50.0, 50.0, 50.0)}}]),
        ),
        (
            "swept blend along an arc",
            json!([{"sketch": [arc([1000.0, 0.0], [0.0, 1000.0], 90.0)], "sweep": {"profile": rect(-100.0, 0.0, 100.0, 200.0), "end": [{"centre": [0, 100], "radius": 50}]}}]),
        ),
        (
            "voids and a solid cutting",
            json!([
                {"sketch": rect(-600.0, -600.0, 600.0, 600.0), "from": 0, "to": 600},
                {"sketch": rect(-200.0, -200.0, 200.0, 200.0), "top": [{"centre": [0, 0], "radius": 100}], "from": -10, "to": 700, "void": true},
                {"sketch": [line([-700.0, 0.0], [700.0, 0.0])], "sweep": {"profile": rect(-50.0, 400.0, 50.0, 500.0)}, "void": true},
                {"sketch": [{"centre": [0, 0], "radius": 900}], "from": 0, "to": 300}
            ]),
        ),
    ]
}

/// The hash of every family here, built as the code builds them.
fn built() -> u64 {
    let mut h = Fnv::new();
    let mut s = Session::new();
    s.script(crate::sample::SAMPLE).unwrap();
    for (name, forms) in kinds() {
        s.execute(
            "family",
            &json!({"name": name, "category": "generic",
                    "parameters": [{"name": "W", "value": 400}, {"name": "L", "value": 2000}],
                    "types": {"A": {}, "B": {"W": 250, "L": 3000}}, "forms": forms}),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    let mut families: Vec<(String, Family)> = s.model.families.iter().map(|(n, f)| (n.clone(), f.clone())).collect();
    for (name, def) in crate::fixtures::library(crate::language::Language::English) {
        if let Ok(f) = serde_json::from_value::<Family>(def) {
            families.push((format!("fixture {name}"), f));
        }
    }
    families.sort_by(|a, b| a.0.cmp(&b.0));
    let mut kinds_seen = 0;
    for (name, f) in &families {
        kinds_seen += f.forms.len();
        hash_family(&mut h, name, f);
    }
    assert!(families.len() > 20 && kinds_seen > 40, "{} families, {kinds_seen} forms: the set is what it should be", families.len());
    h.0
}

/// What `built` came to before `MeshKernel`, on followups f1c1a64a.
const BEFORE: u64 = 0x013c_6ea1_1c31_54d0;

#[test]
fn every_form_builds_bit_for_bit_as_before_the_kernel() {
    let now = built();
    assert_eq!(now, BEFORE, "the forms built now hash to {now:#018x}");
}
