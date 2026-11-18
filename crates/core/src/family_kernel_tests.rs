//! GEO-1 step 2: the forms are built through `family::kernel`'s `MeshKernel` exactly as they were before it: every piece
//! of every family here, for each of its types, hashed bit for bit (its triangles, caps, outline, holes, heights, roundness,
//! material and Visibility Settings) and held to the hash the code made before the kernel was put between it and the forms.
//! The families: the sample house's, the starter fixtures, and forms of every kind (extrusions with arcs and holes, a
//! revolve, blends, sweeps along a bent path, an arc and a circle, swept blends, voids and solids cutting them, a join, a
//! void through `cut_by` and one kept off by `uncut_by`, flexed by their types), and what a door cuts in its wall.

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
        // What it cuts in its host, where it is hosted.
        match f.host_voids(&values) {
            Err(e) => h.bytes(e.as_bytes()),
            Ok(voids) => {
                h.n(voids.len());
                for v in &voids {
                    h.n(v.len());
                    v.iter().flatten().flatten().for_each(|x| h.f(*x));
                }
            }
        }
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

/// A form's GUID, the `n`th of these.
fn guid(n: u32) -> String {
    format!("9e01c000-0000-4000-8000-{n:012}")
}

/// A square `w` (a formula) across, its corner at (300, 300).
fn square_of(w: &str) -> Vec<Value> {
    let (a, b) = (json!(300), json!(format!("300 + {w}")));
    let at = |x: &Value, y: &Value| json!([x, y]);
    vec![
        json!({"start": at(&a, &a), "end": at(&b, &a)}),
        json!({"start": at(&b, &a), "end": at(&b, &b)}),
        json!({"start": at(&b, &b), "end": at(&a, &b)}),
        json!({"start": at(&a, &b), "end": at(&a, &a)}),
    ]
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
        // FE-16's cuts beyond the list's order, sized by W so the two types flex them: a solid joined to another, a void
        // listed before the solid it cuts (`cut_by`), and a void listed after one it doesn't (`uncut_by`).
        (
            "a join",
            json!([
                {"guid": guid(1), "sketch": rect(0.0, 0.0, 1000.0, 1000.0), "from": 0, "to": 100, "joined_by": [guid(2)]},
                {"guid": guid(2), "sketch": square_of("W"), "from": 50, "to": 150}
            ]),
        ),
        (
            "a void cutting through cut_by",
            json!([
                {"guid": guid(3), "sketch": square_of("W"), "from": -10, "to": 110, "void": true},
                {"guid": guid(4), "sketch": rect(0.0, 0.0, 1000.0, 1000.0), "from": 0, "to": 100, "cut_by": [guid(3)]}
            ]),
        ),
        (
            "a void kept off by uncut_by",
            json!([
                {"guid": guid(5), "sketch": rect(0.0, 0.0, 1000.0, 1000.0), "from": 0, "to": 100, "uncut_by": [guid(6)]},
                {"guid": guid(7), "sketch": rect(0.0, 0.0, 1000.0, 1000.0), "from": 200, "to": 300},
                {"guid": guid(6), "sketch": square_of("W"), "from": -10, "to": 310, "void": true}
            ]),
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
    // A door: its leaf, and the opening it cuts in its wall, sized by Width.
    s.execute(
        "family",
        &json!({"name": "a door's wall void", "category": "door",
        "parameters": [{"name": "Width", "value": 900}, {"name": "Height", "value": 2100}],
        "types": {"A": {}, "B": {"Width": 800}},
        "forms": [
            {"sketch": rect(0.0, -20.0, 900.0, 20.0), "from": 0, "to": 2100},
            {"sketch": [
                {"start": [0, -200], "end": ["Width", -200]}, {"start": ["Width", -200], "end": ["Width", 200]},
                {"start": ["Width", 200], "end": [0, 200]}, {"start": [0, 200], "end": [0, -200]}
            ], "from": 0, "to": "Height", "void": true, "cuts": "host"}
        ]}),
    )
    .unwrap_or_else(|e| panic!("door: {e}"));
    let mut families: Vec<(String, Family)> = s.model.families.iter().map(|(n, f)| (n.clone(), f.clone())).collect();
    for (name, def) in crate::fixtures::library(crate::language::Language::English) {
        if let Ok(f) = serde_json::from_value::<Family>(def) {
            families.push((format!("fixture {name}"), f));
        }
    }
    families.sort_by(|a, b| a.0.cmp(&b.0));
    // Each hand-made kind builds, both its types, into pieces: an error hashes too, so one that didn't would prove nothing.
    for (name, _) in kinds() {
        let f = &s.model.families[name];
        for t in f.types.keys() {
            let values = f.values(t, &BTreeMap::new()).unwrap_or_else(|e| panic!("{name} {t}: {e}"));
            let pieces = f.pieces(&values).unwrap_or_else(|e| panic!("{name} {t}: {e}"));
            assert!(!pieces.is_empty() && pieces.iter().all(|p| !p.tris.is_empty()), "{name} {t} builds pieces");
        }
    }
    let door = &s.model.families["a door's wall void"];
    for t in door.types.keys() {
        let values = door.values(t, &BTreeMap::new()).unwrap();
        assert_eq!(door.host_voids(&values).unwrap().len(), 1, "the door cuts its wall, type {t}");
    }
    let (mut forms, mut built, mut errors) = (0, 0, 0);
    for (name, f) in &families {
        forms += f.forms.len();
        for t in f.types.keys() {
            match f.values(t, &BTreeMap::new()).map(|v| f.pieces(&v)) {
                Ok(Ok(p)) => built += p.len(),
                _ => errors += 1,
            }
        }
        hash_family(&mut h, name, f);
    }
    assert!(families.len() > 20 && forms > 40, "{} families, {forms} forms: the set is what it should be", families.len());
    // Most of it builds: the hash is of pieces, the few types that can't build (a fixture's on its own) hashing their errors.
    assert!(built > 100 && errors * 10 < built, "{built} pieces built, {errors} types refused");
    h.0
}

/// What `built` came to before `MeshKernel`, on followups f1c1a64a (this file run there), on Linux x86-64.
const BEFORE: u64 = 0x1398_ea93_dcfb_0c31;

/// Bit for bit holds where the hash was recorded: each platform's maths library may round a sine or a cosine an ulp apart
/// (Windows' and macOS' do), which the hash sees though no geometry changes. CI's Linux job runs it.
#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "the hash was recorded with Linux x86-64's maths library, whose sines and cosines others may round an ulp apart"
)]
fn every_form_builds_bit_for_bit_as_before_the_kernel() {
    // GEO-2e's drill builds a through-void as a profile hole on purpose; `family_cutting_tests` holds it to the cut.
    let now = crate::family::without_drilling(built);
    assert_eq!(now, BEFORE, "the forms built now hash to {now:#018x}");
}
