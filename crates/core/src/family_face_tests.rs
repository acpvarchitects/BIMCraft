//! GEO-2b: every triangle of a built piece knows the face of its form it is, keyed by the sketch ([`FormFace`]): caps, the
//! side each profile line sweeps (a sweep's along each path line), a blend's top-only sides, a pre-sketch profile's edges, and
//! the faces a void cuts in; the same keys across a flex and a line moved; and the JSON a key is written as.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::family::{Family, FormFace, Piece};
use crate::geom::P2;
use crate::ids::Guid;
use crate::view3d::{P3, cross, dot, sub};

fn guid(n: u32) -> Guid {
    Guid::parse(&gstr(n)).unwrap()
}

fn gstr(n: u32) -> String {
    format!("9e01c000-0000-4000-8000-{n:012}")
}

fn line(n: u32, a: [f64; 2], b: [f64; 2]) -> Value {
    json!({"guid": gstr(n), "start": a, "end": b})
}

/// A rectangle's four lines, GUIDs `n` to `n + 3`: bottom, right, top, left.
fn rect(n: u32, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Value> {
    vec![line(n, [x0, y0], [x1, y0]), line(n + 1, [x1, y0], [x1, y1]), line(n + 2, [x1, y1], [x0, y1]), line(n + 3, [x0, y1], [x0, y0])]
}

fn family(forms: Value) -> Family {
    serde_json::from_value(json!({
        "category": "generic",
        "parameters": [{"name": "W", "value": 400}, {"name": "L", "value": 2000}],
        "types": {"A": {}, "B": {"W": 250, "L": 3000}},
        "forms": forms
    }))
    .unwrap()
}

fn pieces_of(f: &Family, t: &str) -> Vec<Piece> {
    let values = f.values(t, &BTreeMap::new()).unwrap();
    f.pieces(&values).unwrap()
}

fn one(f: &Family) -> Piece {
    let mut p = pieces_of(f, "A");
    assert_eq!(p.len(), 1);
    p.remove(0)
}

fn normal(t: &[P3; 3]) -> P3 {
    let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    let len = dot(n, n).sqrt();
    n.map(|x| x / len)
}

/// Each triangle with its key.
fn tagged(p: &Piece) -> Vec<(&[P3; 3], &FormFace)> {
    assert_eq!(p.faces.len(), p.tris.len(), "a tag per triangle");
    p.tris.iter().zip(&p.faces).map(|(t, k)| (t, &p.keys[*k as usize])).collect()
}

fn keys(p: &Piece) -> BTreeSet<String> {
    let used: BTreeSet<u32> = p.faces.iter().copied().collect();
    used.iter().map(|k| serde_json::to_string(&p.keys[*k as usize]).unwrap()).collect()
}

fn side(n: u32) -> FormFace {
    FormFace::side(guid(n))
}

#[test]
fn an_extrusions_caps_and_each_lines_side_are_tagged() {
    let f = family(json!([{"sketch": rect(1, 0.0, 0.0, 1000.0, 500.0), "from": 0, "to": 300}]));
    let p = one(&f);
    let want: BTreeSet<FormFace> = [FormFace::Start, FormFace::End, side(1), side(2), side(3), side(4)].into();
    let got: BTreeSet<FormFace> = tagged(&p).into_iter().map(|(_, k)| k.clone()).collect();
    assert_eq!(got, want);
    let ends = [((0.0, 0.0), (1000.0, 0.0)), ((1000.0, 0.0), (1000.0, 500.0)), ((1000.0, 500.0), (0.0, 500.0)), ((0.0, 500.0), (0.0, 0.0))];
    for (t, k) in tagged(&p) {
        let n = normal(t);
        match k {
            FormFace::Start => assert!(n[2] < -1.0 + 1e-9 && t.iter().all(|q| q[2] == 0.0), "{t:?}"),
            FormFace::End => assert!(n[2] > 1.0 - 1e-9 && t.iter().all(|q| q[2] == 300.0), "{t:?}"),
            FormFace::Side { line, along: None } => {
                let i = (1..=4).position(|m| guid(m) == *line).unwrap();
                let ((ax, ay), (bx, by)) = ends[i];
                let d = [bx - ax, by - ay, 0.0];
                assert!(dot(n, d).abs() < 1e-9 && n[2].abs() < 1e-9, "a side's normal is square to its line: {n:?}");
                for q in t {
                    assert!(crate::geom::dist_to_segment(P2(q[0], q[1]), P2(ax, ay), P2(bx, by)) < 1e-9, "{q:?} on line {i}");
                }
            }
            k => panic!("{k:?}"),
        }
    }
}

#[test]
fn an_arcs_chords_and_a_circles_share_one_side() {
    let mut rounded = rect(1, -500.0, -300.0, 300.0, 300.0);
    rounded.truncate(3);
    rounded.push(json!({"guid": gstr(4), "start": [-500, 300], "end": [-500, -300], "turn": 180}));
    let p = one(&family(json!([{"sketch": rounded, "from": 0, "to": 100}])));
    let arc: Vec<_> = tagged(&p).into_iter().filter(|(_, k)| **k == side(4)).collect();
    assert!(arc.len() > 20, "every chord of the arc is its line's: {}", arc.len());
    for (t, _) in &arc {
        // On the arc: its corners a radius from its centre.
        assert!(t.iter().all(|q| ((q[0] + 500.0).hypot(q[1]) - 300.0).abs() < 1e-6), "{t:?}");
    }
    assert_eq!(keys(&p).len(), 6);
    let c = one(&family(json!([{"sketch": [{"guid": gstr(9), "centre": [0, 0], "radius": "W"}], "from": 0, "to": 900}])));
    let want: BTreeSet<String> = [FormFace::Start, FormFace::End, side(9)].iter().map(|k| serde_json::to_string(k).unwrap()).collect();
    assert_eq!(keys(&c), want);
}

#[test]
fn a_revolves_sides_are_its_lines_and_a_part_turn_has_ends() {
    let mut sketch = rect(1, 200.0, 0.0, 500.0, 800.0);
    sketch.push(json!({"guid": gstr(5), "start": [0, 0], "end": [0, 100], "axis": true}));
    for (to, ends) in [(270, true), (360, false)] {
        let p = one(&family(json!([{"sketch": sketch.clone(), "from": 0, "to": to}])));
        let got: BTreeSet<FormFace> = tagged(&p).into_iter().map(|(_, k)| k.clone()).collect();
        let mut want: BTreeSet<FormFace> = (1..=4).map(side).collect();
        if ends {
            want.extend([FormFace::Start, FormFace::End]);
        }
        assert_eq!(got, want, "turned to {to}");
        // About the sketch's y (the world's y): a line at y = c keeps its y, one at x = r its distance from the axis.
        for (t, k) in tagged(&p) {
            let check = |q: &P3| match k {
                k if *k == side(1) => q[1].abs() < 1e-6,
                k if *k == side(3) => (q[1] - 800.0).abs() < 1e-6,
                k if *k == side(2) => (q[0].hypot(q[2]) - 500.0).abs() < 1e-6,
                k if *k == side(4) => (q[0].hypot(q[2]) - 200.0).abs() < 1e-6,
                _ => true,
            };
            assert!(t.iter().all(check), "{k:?}: {t:?}");
        }
    }
}

#[test]
fn a_blends_sides_are_its_base_lines_or_its_tops() {
    let f = family(json!([{"sketch": rect(1, -500.0, -500.0, 500.0, 500.0),
        "top": [{"guid": gstr(9), "centre": [0, 0], "radius": 250}], "from": 0, "to": 1000, "twist": 1}]));
    let p = one(&f);
    let got: BTreeSet<FormFace> = tagged(&p).into_iter().map(|(_, k)| k.clone()).collect();
    let mut want: BTreeSet<FormFace> = (1..=4).map(side).collect();
    want.extend([FormFace::Start, FormFace::End, FormFace::TopSide(guid(9))]);
    assert_eq!(got, want);
    for (t, k) in tagged(&p) {
        match k {
            FormFace::Start => assert!(t.iter().all(|q| q[2] == 0.0)),
            FormFace::End => assert!(t.iter().all(|q| (q[2] - 1000.0).abs() < 1e-9)),
            // A base line's side has two corners on that line.
            k if *k == side(1) => assert_eq!(t.iter().filter(|q| q[2] == 0.0 && (q[1] + 500.0).abs() < 1e-9).count(), 2, "{t:?}"),
            // A top-only side: a base corner and a chord of the circle.
            FormFace::TopSide(_) => assert_eq!(t.iter().filter(|q| (q[2] - 1000.0).abs() < 1e-9).count(), 2, "{t:?}"),
            _ => {}
        }
    }
}

#[test]
fn a_sweeps_sides_are_each_profile_line_along_each_path_line() {
    let path = vec![line(11, [0.0, 0.0], [2000.0, 0.0]), line(12, [2000.0, 0.0], [2000.0, 1000.0])];
    let p = one(&family(json!([{"sketch": path, "sweep": {"profile": rect(1, -100.0, 0.0, 100.0, 200.0)}}])));
    let got: BTreeSet<FormFace> = tagged(&p).into_iter().map(|(_, k)| k.clone()).collect();
    let mut want: BTreeSet<FormFace> = [FormFace::Start, FormFace::End].into();
    for l in 1..=4 {
        for path in [11, 12] {
            want.insert(side(l).along(Some(guid(path))));
        }
    }
    assert_eq!(got, want);
    for (t, k) in tagged(&p) {
        match k {
            FormFace::Side { along: Some(a), .. } => {
                let (x0, y0, x1, y1) = if *a == guid(11) { (0.0, 0.0, 2000.0, 0.0) } else { (2000.0, 0.0, 2000.0, 1000.0) };
                // Within the profile's half width of its line, or the mitre's corner at the bend.
                for q in t {
                    let d = crate::geom::dist_to_segment(P2(q[0], q[1]), P2(x0, y0), P2(x1, y1));
                    assert!(d <= 100.0 * 2f64.sqrt() + 1e-6, "{k:?} {q:?}");
                }
            }
            FormFace::Start => assert!(t.iter().all(|q| q[0].abs() < 1e-9)),
            FormFace::End => assert!(t.iter().all(|q| (q[1] - 1000.0).abs() < 1e-9)),
            k => panic!("{k:?}"),
        }
    }
}

#[test]
fn a_swept_blends_sides_go_along_its_path() {
    let p = one(&family(json!([{"sketch": [{"guid": gstr(11), "start": [1000, 0], "end": [0, 1000], "turn": 90}],
        "sweep": {"profile": rect(1, -100.0, 0.0, 100.0, 200.0), "end": [{"guid": gstr(9), "centre": [0, 100], "radius": 50}]}}])));
    let got: BTreeSet<FormFace> = tagged(&p).into_iter().map(|(_, k)| k.clone()).collect();
    let mut allowed: BTreeSet<FormFace> = (1..=4).map(|l| side(l).along(Some(guid(11)))).collect();
    allowed.extend([FormFace::Start, FormFace::End, FormFace::TopSide(guid(9))]);
    assert!(got.is_subset(&allowed), "{got:?}");
    assert!(got.contains(&FormFace::Start) && got.contains(&FormFace::End));
    assert!((1..=4).all(|l| got.contains(&side(l).along(Some(guid(11))))), "{got:?}");
}

#[test]
fn a_profile_whose_lines_have_no_guid_is_tagged_by_edge() {
    let f: Family = serde_json::from_value(json!({
        "category": "generic", "types": {"A": {}},
        "forms": [{"profile": [[0, 0], [1000, 0], [1000, 500], [0, 500]], "from": 0, "to": 100},
                  {"circle": {"centre": [3000, 0], "radius": 100}, "from": 0, "to": 100}]
    }))
    .unwrap();
    let p = pieces_of(&f, "A");
    let first: BTreeSet<FormFace> = tagged(&p[0]).into_iter().map(|(_, k)| k.clone()).collect();
    assert_eq!(first, [FormFace::Start, FormFace::End, FormFace::Edge(0), FormFace::Edge(1), FormFace::Edge(2), FormFace::Edge(3)].into());
    // Edge 0 runs from the first corner to the second: y = 0.
    for (t, k) in tagged(&p[0]) {
        if *k == FormFace::Edge(0) {
            assert!(t.iter().all(|q| q[1] == 0.0), "{t:?}");
        }
    }
    let circle: BTreeSet<FormFace> = tagged(&p[1]).into_iter().map(|(_, k)| k.clone()).collect();
    assert_eq!(circle, [FormFace::Start, FormFace::End, FormFace::Edge(0)].into());
}

#[test]
fn every_piece_of_every_family_has_a_tag_per_triangle_in_range() {
    let (mut tagged_pieces, mut all) = (0, 0);
    for (name, f) in crate::family_kernel_tests::families() {
        for t in f.types.keys() {
            let Ok(Ok(pieces)) = f.values(t, &BTreeMap::new()).map(|v| f.pieces(&v)) else { continue };
            for p in pieces {
                all += 1;
                assert!(
                    p.faces.is_empty() || p.faces.len() == p.tris.len(),
                    "{name} {t}: {} tags, {} triangles",
                    p.faces.len(),
                    p.tris.len()
                );
                assert!(p.faces.iter().all(|k| (*k as usize) < p.keys.len()), "{name} {t}: a tag past its keys");
                tagged_pieces += usize::from(!p.faces.is_empty());
            }
        }
    }
    assert!(tagged_pieces == all && all > 100, "{tagged_pieces} of {all} pieces tagged");
}

#[test]
fn a_voids_hole_is_tagged_by_the_void_and_its_own_face() {
    let f = family(json!([
        {"guid": gstr(100), "sketch": rect(1, 0.0, 0.0, 1000.0, 1000.0), "from": 0, "to": 100},
        {"guid": gstr(200), "sketch": rect(21, 400.0, 400.0, 600.0, 600.0), "from": -10, "to": 110, "void": true}
    ]));
    let p = one(&f);
    let got: BTreeSet<FormFace> = tagged(&p).into_iter().map(|(_, k)| k.clone()).collect();
    let mut want: BTreeSet<FormFace> = [FormFace::Start, FormFace::End].into();
    want.extend((1..=4).map(side));
    want.extend((21..=24).map(|l| FormFace::Cut { by: guid(200), face: Box::new(side(l)) }));
    assert_eq!(got, want);
    for (t, k) in tagged(&p) {
        if *k == (FormFace::Cut { by: guid(200), face: Box::new(side(21)) }) {
            assert!(t.iter().all(|q| q[1] == 400.0 && (400.0..=600.0).contains(&q[0])), "{t:?}");
        }
    }
}

#[test]
fn a_flex_and_a_line_moved_keep_the_keys() {
    let forms = |x: f64| {
        json!([
            {"guid": gstr(100), "sketch": [
                line(1, [0.0, 0.0], [x, 0.0]), line(2, [x, 0.0], [x, 500.0]), {"guid": gstr(3), "start": [x, 500], "end": [0, 500], "turn": 30},
                line(4, [0.0, 500.0], [0.0, 0.0])], "from": 0, "to": "L"},
            {"guid": gstr(200), "sketch": [{"guid": gstr(21), "centre": [300, 250], "radius": "W / 4"}], "from": -10, "to": 4000, "void": true}
        ])
    };
    let f = family(forms(1000.0));
    let a = keys(&pieces_of(&f, "A")[0]);
    assert_eq!(a, keys(&pieces_of(&f, "B")[0]), "flexed");
    let moved = family(forms(1400.0));
    assert_eq!(a, keys(&pieces_of(&moved, "A")[0]), "a line moved");
    assert!(a.len() == 7, "{a:?}");
}

#[test]
fn every_key_is_written_as_its_json() {
    let (l, p, g) = (guid(1), guid(2), guid(3));
    let cases = [
        (FormFace::Start, json!("start")),
        (FormFace::End, json!("end")),
        (FormFace::All, json!("all")),
        (FormFace::side(l), json!({"side": gstr(1)})),
        (FormFace::Side { line: l, along: Some(p) }, json!({"side": gstr(1), "along": gstr(2)})),
        (FormFace::TopSide(l), json!({"top_side": gstr(1)})),
        (FormFace::Edge(3), json!({"edge": 3})),
        (
            FormFace::Cut { by: g, face: Box::new(FormFace::Side { line: l, along: Some(p) }) },
            json!({"cut": gstr(3), "face": {"side": gstr(1), "along": gstr(2)}}),
        ),
        (FormFace::Cut { by: g, face: Box::new(FormFace::End) }, json!({"cut": gstr(3), "face": "end"})),
    ];
    for (key, want) in cases {
        assert_eq!(serde_json::to_value(&key).unwrap(), want);
        assert_eq!(serde_json::from_value::<FormFace>(want).unwrap(), key);
    }
}

#[test]
fn a_bad_key_is_refused_naming_every_shape_a_face_takes() {
    let shapes = r#"a face is "start", "end", "all", {"side": L}, {"side": L, "along": P}, {"top_side": T}, {"edge": i} or {"cut": G, "face": …}; got "#;
    for bad in [
        json!("middle"),
        json!({"side": 3.5}),
        json!({"side": gstr(1), "x": 1}),
        json!({"side": gstr(1), "top_side": gstr(2)}),
        json!({"edge": -1}),
        json!({}),
        json!({"cut": gstr(1), "face": "middle"}),
    ] {
        let e = serde_json::from_value::<FormFace>(bad.clone()).unwrap_err().to_string();
        assert!(e.starts_with(shapes), "{bad}: {e}");
    }
    // What it got is said, short.
    let long = json!({"side": "x".repeat(500)});
    let e = serde_json::from_value::<FormFace>(long).unwrap_err().to_string();
    assert!(e.contains("{\"side\":\"xxx") && e.chars().count() < shapes.chars().count() + 100, "{e}");
}

#[test]
fn a_sweep_round_a_closed_path_runs_each_side_along_its_own_path_line() {
    // A rectangle of four path lines, drawn from (0, 0) round to it again: the last, back to the start, is the wrap.
    let path = rect(11, 0.0, 0.0, 2000.0, 1000.0);
    let p = one(&family(json!([{"sketch": path, "sweep": {"profile": rect(1, -50.0, 0.0, 50.0, 100.0)}}])));
    let lines =
        [((0.0, 0.0), (2000.0, 0.0)), ((2000.0, 0.0), (2000.0, 1000.0)), ((2000.0, 1000.0), (0.0, 1000.0)), ((0.0, 1000.0), (0.0, 0.0))];
    let mut seen = BTreeSet::new();
    for (t, k) in tagged(&p) {
        let FormFace::Side { along: Some(a), .. } = k else { panic!("a closed path has no caps: {k:?}") };
        let n = (11..=14).position(|m| guid(m) == *a).unwrap();
        seen.insert(n);
        let c = P2(t.iter().map(|q| q[0]).sum::<f64>() / 3.0, t.iter().map(|q| q[1]).sum::<f64>() / 3.0);
        let near = |((x0, y0), (x1, y1)): ((f64, f64), (f64, f64))| crate::geom::dist_to_segment(c, P2(x0, y0), P2(x1, y1));
        let d = near(lines[n]);
        assert!(d <= 50.0 + 1e-6, "{k:?}: its centroid {c:?} is {d} off its path line");
        assert!(lines.iter().all(|l| near(*l) >= d - 1e-9), "{k:?}: its centroid {c:?} is nearer another path line");
    }
    assert_eq!(seen.len(), 4, "every path line, the wrap's too");
}

/// A slab of four lines (GUIDs 1 to 4) less a square void through it (form GUID 200, lines 21 to 24) and a round one
/// (form GUID 300, line 31).
fn drilled_slab() -> Family {
    family(json!([
        {"guid": gstr(100), "sketch": rect(1, 0.0, 0.0, 1000.0, 1000.0), "from": 0, "to": 100},
        {"guid": gstr(200), "sketch": rect(21, 400.0, 400.0, 600.0, 600.0), "from": -10, "to": 110, "void": true},
        {"guid": gstr(300), "sketch": [{"guid": gstr(31), "centre": [200, 200], "radius": 50}], "from": -10, "to": 110, "void": true}
    ]))
}

#[test]
fn a_drilled_hole_is_tagged_as_the_cut_that_made_it_would_be() {
    let f = drilled_slab();
    let drilled = one(&f);
    let cut = crate::family::without_drilling(|| one(&f));
    assert!(drilled.tris.len() < cut.tris.len(), "drilled, not cut: {} triangles to {}", drilled.tris.len(), cut.tris.len());
    let set = |p: &Piece| tagged(p).into_iter().map(|(_, k)| k.clone()).collect::<BTreeSet<FormFace>>();
    assert_eq!(set(&drilled), set(&cut));
    let mut want: BTreeSet<FormFace> = [FormFace::Start, FormFace::End].into();
    want.extend((1..=4).map(side));
    want.extend((21..=24).map(|l| FormFace::Cut { by: guid(200), face: Box::new(side(l)) }));
    want.insert(FormFace::Cut { by: guid(300), face: Box::new(side(31)) });
    assert_eq!(set(&drilled), want);
    for (t, k) in tagged(&drilled) {
        if *k == (FormFace::Cut { by: guid(200), face: Box::new(side(21)) }) {
            assert!(t.iter().all(|q| q[1] == 400.0 && (400.0..=600.0).contains(&q[0])), "{t:?}");
        }
        if *k == (FormFace::Cut { by: guid(300), face: Box::new(side(31)) }) {
            assert!(t.iter().all(|q| ((q[0] - 200.0).hypot(q[1] - 200.0) - 50.0).abs() < 1e-6), "{t:?}");
        }
    }
}

#[test]
fn a_cutter_without_a_guid_cuts_faces_that_are_all() {
    // Built as a file from before GUIDs would be: two voids with none, which no key could tell apart.
    let f: Family = serde_json::from_value(json!({
        "category": "generic", "types": {"A": {}},
        "forms": [
            {"sketch": rect(1, 0.0, 0.0, 1000.0, 1000.0), "from": 0, "to": 100},
            {"sketch": rect(21, 100.0, 100.0, 300.0, 300.0), "from": -10, "to": 110, "void": true},
            {"sketch": rect(31, 600.0, 600.0, 800.0, 800.0), "from": -10, "to": 50, "void": true}
        ]
    }))
    .unwrap();
    assert!(f.forms.iter().all(|x| x.guid.is_nil()));
    for p in [one(&f), crate::family::without_drilling(|| one(&f))] {
        let got: BTreeSet<FormFace> = tagged(&p).into_iter().map(|(_, k)| k.clone()).collect();
        let mut want: BTreeSet<FormFace> = [FormFace::Start, FormFace::End, FormFace::All].into();
        want.extend((1..=4).map(side));
        assert_eq!(got, want);
    }
}

#[test]
fn a_piece_keeps_only_the_keys_its_triangles_use() {
    let mut sketch = rect(1, 200.0, 0.0, 500.0, 800.0);
    sketch.push(json!({"guid": gstr(5), "start": [0, 0], "end": [0, 100], "axis": true}));
    let p = one(&family(json!([{"sketch": sketch, "from": 0, "to": 360}])));
    assert!(!p.keys.contains(&FormFace::Start) && !p.keys.contains(&FormFace::End), "{:?}", p.keys);
    // A void flush with the slab's top: its own top face makes none of the slab's.
    let flush = one(&family(json!([
        {"guid": gstr(100), "sketch": rect(1, 0.0, 0.0, 1000.0, 1000.0), "from": 0, "to": 100},
        {"guid": gstr(200), "sketch": rect(21, 400.0, 400.0, 600.0, 600.0), "from": 50, "to": 100, "void": true}
    ])));
    let all_used = |p: &Piece, what: &str| {
        let used: BTreeSet<u32> = p.faces.iter().copied().collect();
        assert_eq!(used.len(), p.keys.len(), "{what}: {:?} keys, {} used", p.keys, used.len());
    };
    all_used(&flush, "a flush void");
    for (name, f) in crate::family_kernel_tests::families() {
        for t in f.types.keys() {
            let Ok(Ok(pieces)) = f.values(t, &BTreeMap::new()).map(|v| f.pieces(&v)) else { continue };
            pieces.iter().for_each(|p| all_used(p, &format!("{name} {t}")));
        }
    }
}

#[test]
fn an_extrusion_on_a_plane_facing_minus_y_has_its_caps_and_sides_right() {
    let host = crate::family::CENTER_FRONT_BACK.to_string();
    let p = one(&family(json!([{"host": {"plane": host}, "sketch": rect(1, 0.0, 0.0, 1000.0, 500.0), "from": 0, "to": 300}])));
    let want: BTreeSet<FormFace> = [FormFace::Start, FormFace::End, side(1), side(2), side(3), side(4)].into();
    assert_eq!(tagged(&p).into_iter().map(|(_, k)| k.clone()).collect::<BTreeSet<_>>(), want);
    // Its plane's x is the world's x, its y the world's z, its normal -y: `from` at y = 0, `to` at y = -300.
    for (t, k) in tagged(&p) {
        let n = normal(t);
        match k {
            FormFace::Start => assert!(n[1] > 1.0 - 1e-9 && t.iter().all(|q| q[1].abs() < 1e-9), "{t:?}"),
            FormFace::End => assert!(n[1] < -1.0 + 1e-9 && t.iter().all(|q| (q[1] + 300.0).abs() < 1e-9), "{t:?}"),
            k if *k == side(1) => assert!(n[2] < -1.0 + 1e-9 && t.iter().all(|q| q[2].abs() < 1e-9), "{t:?}"),
            k if *k == side(2) => assert!(n[0] > 1.0 - 1e-9 && t.iter().all(|q| (q[0] - 1000.0).abs() < 1e-9), "{t:?}"),
            k if *k == side(3) => assert!(n[2] > 1.0 - 1e-9 && t.iter().all(|q| (q[2] - 500.0).abs() < 1e-9), "{t:?}"),
            k if *k == side(4) => assert!(n[0] < -1.0 + 1e-9 && t.iter().all(|q| q[0].abs() < 1e-9), "{t:?}"),
            k => panic!("{k:?}"),
        }
    }
}

#[test]
fn a_nested_instances_piece_keeps_its_own_keys_and_the_hosts_voids_leave_it_alone() {
    let leaf = json!({"category": "generic", "types": {"B": {}},
        "forms": [{"guid": gstr(500), "sketch": rect(41, 0.0, 0.0, 100.0, 100.0), "from": 0, "to": 50}]});
    let f: Family = serde_json::from_value(json!({
        "category": "generic", "types": {"A": {}},
        "forms": [
            {"guid": gstr(100), "sketch": rect(1, 0.0, 0.0, 1000.0, 1000.0), "from": 0, "to": 100},
            {"guid": gstr(200), "sketch": rect(21, 1990.0, -10.0, 2050.0, 200.0), "from": -10, "to": 300, "void": true}
        ],
        "nested": [{"name": "Leaf", "family": leaf}],
        "nested_instances": [{"family": "Leaf", "type": "B", "at": [2000, 0]}]
    }))
    .unwrap();
    let pieces = pieces_of(&f, "A");
    assert_eq!(pieces.len(), 2);
    let nested = &pieces[1];
    assert!(nested.tris.iter().flatten().all(|q| q[0] >= 2000.0 - 1e-9), "placed at 2000");
    let want: BTreeSet<FormFace> = [FormFace::Start, FormFace::End, side(41), side(42), side(43), side(44)].into();
    // A host's void cuts the host's own forms only: the nested piece is whole, its faces its own.
    assert_eq!(tagged(nested).into_iter().map(|(_, k)| k.clone()).collect::<BTreeSet<_>>(), want);
    for (t, k) in tagged(nested) {
        if *k == side(41) {
            assert!(t.iter().all(|q| q[1].abs() < 1e-9), "{t:?}");
        }
    }
}
