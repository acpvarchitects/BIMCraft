//! Cut Geometry in the project (T23): a component of a family set to Cut with Voids When Loaded, standing on a level where its
//! voids pass through a wall, cuts the wall once `cut_geometry` names it, and stops when `uncut_geometry` does. The cut
//! shows in section, in plan, in 3D and as an IFC opening, is saved, undone and redone, follows the component, sleeps while
//! the family's flag is off, and goes with either element.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::face_void_tests::{filled, id_of, ifc, run, voided};
use crate::geom::P2;
use crate::model::{Element, Id};
use crate::render::{self, POCHE, View};

/// The first level, whose plan the first view is.
const LEVEL: Id = 100000;

/// The forms of a cutter: a void 400 along x, 800 along y, from 900 to 1500 up, which cuts a host (`cuts` `both`, the
/// default); then a small marker solid 600 to the south, clear of the void and of any wall along y 0.
fn cutter_forms() -> Value {
    json!([
        {"void": true, "profile": [[-200, -400], [200, -400], [200, 400], [-200, 400]], "from": 900, "to": 1500},
        {"profile": [[-50, -650], [50, -650], [50, -550], [-50, -550]], "from": 0, "to": 50}
    ])
}

/// Family `name`, a generic model on a level with the cutter's forms, Cut with Voids When Loaded as `cuts` says.
fn cutter_family(s: &mut Session, name: &str, cuts: bool) {
    run(
        s,
        "family",
        json!({"name": name, "category": "generic", "types": {"A": {}},
               "options": {"cut_with_voids_when_loaded": cuts}, "forms": cutter_forms()}),
    );
}

/// A wall along x from 0 to 6000, 3000 high, of the starter type (200 thick, centred on y 0).
fn wall(s: &mut Session) -> Id {
    run(s, "wall", json!({"points": [[0, 0], [6000, 0]], "height": 3000}))["ids"][0].as_u64().unwrap()
}

fn place(s: &mut Session, family: &str, at: [f64; 2]) -> Id {
    id_of(&run(s, "component", json!({"family": family, "at": at})))
}

fn cut(s: &mut Session, w: Id, c: Id) -> Result<Value, String> {
    s.execute("cut_geometry", &json!({"target": w, "cutter": c}))
}

/// Whether the wall is solid at (x, 1200) in the section along it, just in front of its centre line.
fn solid_at(s: &mut Session, w: Id, x: f64) -> bool {
    filled(s, w, [0.0, 10.0], [8000.0, 10.0])(x, 1200.0)
}

/// The colour of the level's plan at (x, y), at a scale that shows the wall's poché.
fn plan_pixel(s: &Session, x: f64, y: f64) -> render::Rgb {
    let view = View { scale: 0.5, cx: 3000.0, cy: 0.0, width: 1200, height: 1200, angle: 0.0 };
    let pm = tiny_skia::Pixmap::decode_png(&render::png(&render::draw(&s.model, LEVEL, view)).unwrap()).unwrap();
    let (u, v) = view.px(P2(x, y));
    let c = pm.pixel(u as u32, v as u32).unwrap();
    render::Rgb(c.red(), c.green(), c.blue())
}

/// A horizontal line across the wall at (x, z), along y: how often it crosses the wall's 3D triangles (0 through a hole).
fn across(s: &Session, w: Id, x: f64, z: f64) -> usize {
    let (x, z) = (x + 0.37, z + 0.13);
    let tris: Vec<[[f64; 3]; 3]> = crate::view3d::solids(&s.model).iter().filter(|t| t.owner() == w).flat_map(|t| t.triangles()).collect();
    let mut hits: Vec<f64> = tris
        .iter()
        .filter_map(|[a, b, c]| {
            // In the (x, z) plane, where the line is a point.
            let (pa, pb, pc) = (P2(a[0], a[2]), P2(b[0], b[2]), P2(c[0], c[2]));
            let area = (pb - pa).cross(pc - pa);
            if area.abs() < 1e-9 {
                return None;
            }
            let q = P2(x, z);
            let (u, v) = ((q - pa).cross(pc - pa) / area, (pb - pa).cross(q - pa) / area);
            (u >= 0.0 && v >= 0.0 && u + v <= 1.0).then(|| a[1] + (b[1] - a[1]) * u + (c[1] - a[1]) * v)
        })
        .collect();
    hits.sort_by(f64::total_cmp);
    hits.dedup_by(|x, y| (*x - *y).abs() < 1e-3);
    hits.len()
}

#[test]
fn a_cutter_cuts_a_wall_only_once_cut_geometry_names_it_and_uncut_takes_it_back() {
    let mut s = Session::new();
    cutter_family(&mut s, "Cutter", true);
    let w = wall(&mut s);
    let c = place(&mut s, "Cutter", [3000.0, 0.0]);
    // Placed, it cuts nothing: Revit's project voids cut only what Cut Geometry names.
    assert!(solid_at(&mut s, w, 3000.0), "not cut until named");
    assert_eq!(across(&s, w, 3000.0, 1200.0), 2);
    assert_eq!(voided(&ifc(&mut s), "IFCWALL"), 0);
    let reply = cut(&mut s, w, c).unwrap();
    assert_eq!((reply["id"].as_u64(), reply["cutter"].as_u64()), (Some(w), Some(c)));
    assert_eq!(s.model.wall(w).unwrap().void_cuts, vec![s.model.guid(c)]);
    // Section: the hole 400 wide at the void's height, the wall round it.
    assert!(!solid_at(&mut s, w, 3000.0), "the hole");
    assert!(solid_at(&mut s, w, 1000.0) && solid_at(&mut s, w, 3300.0), "the wall either side");
    let at = filled(&mut s, w, [0.0, 10.0], [8000.0, 10.0]);
    assert!(at(3000.0, 600.0) && at(3000.0, 2000.0), "under and over the void");
    // 3D: a line across the wall through the hole meets nothing; one beside it, both faces.
    assert_eq!(across(&s, w, 3000.0, 1200.0), 0);
    assert_eq!(across(&s, w, 1000.0, 1200.0), 2);
    // Plan, cut at 1200: open where the void is, poché beside it.
    assert_eq!(plan_pixel(&s, 1000.0, 0.0), POCHE);
    assert_ne!(plan_pixel(&s, 3000.0, 0.0), POCHE);
    // IFC: an opening of the void's shape voiding the wall.
    assert_eq!(voided(&ifc(&mut s), "IFCWALL"), 1);
    // Uncut: whole again, everywhere.
    let (gw, gc) = (s.model.guid(w), s.model.guid(c));
    run(&mut s, "uncut_geometry", json!({"target": gw, "cutter": gc}));
    assert!(s.model.wall(w).unwrap().void_cuts.is_empty());
    assert!(solid_at(&mut s, w, 3000.0));
    assert_eq!(across(&s, w, 3000.0, 1200.0), 2);
    assert_eq!(plan_pixel(&s, 3000.0, 0.0), POCHE);
    assert_eq!(voided(&ifc(&mut s), "IFCWALL"), 0);
}

#[test]
fn the_cut_is_saved_undone_and_redone() {
    let mut s = Session::new();
    cutter_family(&mut s, "Cutter", true);
    let w = wall(&mut s);
    let c = place(&mut s, "Cutter", [3000.0, 0.0]);
    cut(&mut s, w, c).unwrap();
    let g = s.model.guid(c);
    run(&mut s, "undo", json!({}));
    assert!(s.model.wall(w).unwrap().void_cuts.is_empty(), "undone");
    assert_eq!(across(&s, w, 3000.0, 1200.0), 2);
    run(&mut s, "redo", json!({}));
    assert_eq!(s.model.wall(w).unwrap().void_cuts, vec![g], "redone");
    assert_eq!(across(&s, w, 3000.0, 1200.0), 0);
    // Saved and opened again, the wall still names it and is still cut.
    let path = std::env::temp_dir().join(format!("bimcraft-t23-{}-{:?}.bimc", std::process::id(), std::thread::current().id()));
    run(&mut s, "save", json!({"path": path}));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("void_cuts"), "written");
    let mut t = Session::new();
    run(&mut t, "open", json!({"path": path}));
    let _ = std::fs::remove_file(&path);
    let w2 = t.model.elements.id(s.model.guid(w)).unwrap();
    assert_eq!(t.model.wall(w2).unwrap().void_cuts, vec![g]);
    assert_eq!(across(&t, w2, 3000.0, 1200.0), 0);
    // A wall that is cut by nothing writes no `void_cuts`.
    let mut u = Session::new();
    wall(&mut u);
    run(&mut u, "save", json!({"path": path}));
    assert!(!std::fs::read_to_string(&path).unwrap().contains("void_cuts"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_cut_follows_the_cutter_sleeps_while_the_flag_is_off_and_goes_with_either_element() {
    let mut s = Session::new();
    cutter_family(&mut s, "Cutter", true);
    let w = wall(&mut s);
    let c = place(&mut s, "Cutter", [3000.0, 0.0]);
    cut(&mut s, w, c).unwrap();
    // Moved, the hole goes with it.
    run(&mut s, "move", json!({"ids": [c], "by": [1000, 0]}));
    assert_eq!(across(&s, w, 3000.0, 1200.0), 2, "where it was, wall");
    assert_eq!(across(&s, w, 4000.0, 1200.0), 0, "where it is, the hole");
    // Moved off the wall, the tie stays and cuts nothing; back on, it cuts again.
    run(&mut s, "move", json!({"ids": [c], "by": [0, 3000]}));
    assert_eq!(s.model.wall(w).unwrap().void_cuts.len(), 1);
    assert_eq!(across(&s, w, 4000.0, 1200.0), 2);
    run(&mut s, "move", json!({"ids": [c], "by": [0, -3000]}));
    assert_eq!(across(&s, w, 4000.0, 1200.0), 0);
    // The family's flag off: the tie sleeps, the wall is whole; on again, cut.
    run(&mut s, "family", json!({"name": "Cutter", "options": {"cut_with_voids_when_loaded": false}}));
    assert_eq!(s.model.wall(w).unwrap().void_cuts.len(), 1, "kept");
    assert_eq!(across(&s, w, 4000.0, 1200.0), 2, "asleep");
    run(&mut s, "family", json!({"name": "Cutter", "options": {"cut_with_voids_when_loaded": true}}));
    assert_eq!(across(&s, w, 4000.0, 1200.0), 0, "awake");
    // Copied together, the copy of the wall is cut by the copy of the cutter.
    let copies = run(&mut s, "copy", json!({"ids": [w, c], "by": [0, 5000]}));
    let ids: Vec<Id> = copies["ids"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    let (w2, c2) = (ids[0], ids[1]);
    assert!(matches!(s.model.elements.get(c2), Some(Element::Component(_))));
    assert_eq!(s.model.wall(w2).unwrap().void_cuts, vec![s.model.guid(c2)]);
    // Deleting the cutter frees the wall; deleting the other wall leaves the other cutter as it was.
    run(&mut s, "delete", json!({"ids": [c]}));
    assert!(s.model.wall(w).unwrap().void_cuts.is_empty());
    assert_eq!(across(&s, w, 4000.0, 1200.0), 2);
    run(&mut s, "delete", json!({"ids": [w2]}));
    assert!(s.model.elements.contains_key(c2));
}

#[test]
fn cut_geometry_refuses_what_it_cannot_cut_and_says_why() {
    let mut s = Session::new();
    cutter_family(&mut s, "Cutter", true);
    cutter_family(&mut s, "Off", false);
    // A void that cuts only the family's own forms (`cuts`: `forms`) never cuts a project's wall.
    run(
        &mut s,
        "family",
        json!({"name": "Attached", "category": "generic", "types": {"A": {}}, "options": {"cut_with_voids_when_loaded": true},
               "forms": [{"profile": [[-300, -300], [300, -300], [300, 300], [-300, 300]], "from": 1000, "to": 1400},
                         {"void": true, "cuts": "forms", "profile": [[-100, -100], [100, -100], [100, 100], [-100, 100]], "from": 900, "to": 1500}]}),
    );
    let w = wall(&mut s);
    let c = place(&mut s, "Cutter", [3000.0, 0.0]);
    let off = place(&mut s, "Off", [2000.0, 0.0]);
    let attached = place(&mut s, "Attached", [1000.0, 0.0]);
    let far = place(&mut s, "Cutter", [3000.0, 3000.0]);
    let err = |s: &mut Session, w: Id, c: Id| cut(s, w, c).unwrap_err();
    assert!(err(&mut s, c, c).contains("not a wall"), "{}", err(&mut s, c, c));
    assert!(err(&mut s, w, w).contains("not a component"), "{}", err(&mut s, w, w));
    let e = err(&mut s, w, off);
    assert!(e.contains("Cut with Voids When Loaded") && e.contains("cut_with_voids_when_loaded"), "{e}");
    let e = err(&mut s, w, attached);
    assert!(e.contains("no void to cut with"), "{e}");
    let e = err(&mut s, w, far);
    assert!(e.contains("don't pass into"), "{e}");
    assert!(s.execute("cut_geometry", &json!({"target": w})).unwrap_err().contains("`cutter` is required"));
    assert!(s.execute("cut_geometry", &json!({"target": w, "cutter": c, "extra": 1})).is_err(), "a misspelled key");
    cut(&mut s, w, c).unwrap();
    assert!(err(&mut s, w, c).contains("already cuts"));
    run(&mut s, "uncut_geometry", json!({"target": w, "cutter": c}));
    let e = s.execute("uncut_geometry", &json!({"target": w, "cutter": c})).unwrap_err();
    assert!(e.contains("nothing to uncut"), "{e}");
    // Nothing refused changed the wall.
    assert!(s.model.wall(w).unwrap().void_cuts.is_empty());
}

#[test]
fn a_concave_void_whose_outline_spans_the_wall_but_misses_it_is_refused() {
    let mut s = Session::new();
    // An L in plan: an arm along x north of the wall (y 150 to 450) and an arm down its west side past the wall's start, so the
    // L's hull crosses the wall's first 300 or so while the L itself stays clear of it.
    run(
        &mut s,
        "family",
        json!({"name": "Ell", "category": "generic", "types": {"A": {}}, "options": {"cut_with_voids_when_loaded": true},
               "forms": [{"void": true, "profile": [[-500, -400], [-300, -400], [-300, 150], [300, 150], [300, 450], [-500, 450]],
                          "from": 900, "to": 1500}]}),
    );
    let w = wall(&mut s);
    let ell = place(&mut s, "Ell", [200.0, 0.0]);
    let e = cut(&mut s, w, ell).unwrap_err();
    assert!(e.contains("don't pass into"), "{e}");
    assert!(s.model.wall(w).unwrap().void_cuts.is_empty());
    // Moved 300 south, its arm along x (y -150 to 150) passes right through the wall: then it cuts.
    run(&mut s, "move", json!({"ids": [ell], "by": [0, -300]}));
    cut(&mut s, w, ell).unwrap();
    assert_eq!(across(&s, w, 300.0, 1200.0), 0);
}

#[test]
fn a_wall_names_only_components_and_each_once() {
    let mut s = Session::new();
    cutter_family(&mut s, "Cutter", true);
    let w = wall(&mut s);
    let c = place(&mut s, "Cutter", [3000.0, 0.0]);
    let level = s.model.guid(LEVEL);
    let mut m = (*s.model).clone();
    if let Some(Element::Wall(x)) = m.elements.get_mut(w) {
        x.void_cuts = vec![level];
    }
    assert!(m.check().unwrap_err().contains("isn't a component"));
    let g = s.model.guid(c);
    if let Some(Element::Wall(x)) = m.elements.get_mut(w) {
        x.void_cuts = vec![g, g];
    }
    assert!(m.check().unwrap_err().contains("twice"));
}

#[test]
fn many_cutters_on_one_wall_each_cut_their_own_hole() {
    let mut s = Session::new();
    cutter_family(&mut s, "Cutter", true);
    let w = run(&mut s, "wall", json!({"points": [[0, 0], [40000, 0]], "height": 3000}))["ids"][0].as_u64().unwrap();
    let cutters: Vec<Id> = (0..30).map(|i| place(&mut s, "Cutter", [1000.0 + 1200.0 * f64::from(i), 0.0])).collect();
    for c in &cutters {
        cut(&mut s, w, *c).unwrap();
    }
    assert_eq!(s.model.wall(w).unwrap().void_cuts.len(), 30);
    for i in [0, 14, 29] {
        let x = 1000.0 + 1200.0 * f64::from(i);
        assert_eq!(across(&s, w, x, 1200.0), 0, "hole {i}");
        assert_eq!(across(&s, w, x + 600.0, 1200.0), 2, "wall after hole {i}");
    }
    assert_eq!(voided(&ifc(&mut s), "IFCWALL"), 30);
}

#[test]
fn the_tool_picks_a_void_only_cutter_by_its_outline_and_a_sleeping_tie_by_its_place() {
    use crate::project_geometry::{cutters, pick};
    let mut s = Session::new();
    // A family that is only a void: a plan draws nothing of it.
    run(
        &mut s,
        "family",
        json!({"name": "Hole", "category": "generic", "types": {"A": {}}, "options": {"cut_with_voids_when_loaded": true},
               "forms": [{"void": true, "profile": [[-200, -400], [200, -400], [200, 400], [-200, 400]], "from": 900, "to": 1500}]}),
    );
    let w = wall(&mut s);
    let c = place(&mut s, "Hole", [3000.0, 0.0]);
    let found = cutters(&s.model, w, false);
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].id, found[0].anchor, found[0].edges.len()), (c, P2(3000.0, 0.0), 4), "{:?}", found[0].edges);
    // Its outline picks it (the east side at x 3200), and its place; inside, away from both, nothing.
    assert_eq!(pick(&found, P2(3203.0, 300.0), 10.0), Some(c));
    assert_eq!(pick(&found, P2(3000.0, 5.0), 10.0), Some(c));
    assert_eq!(pick(&found, P2(3100.0, 200.0), 10.0), None);
    cut(&mut s, w, c).unwrap();
    assert!(cutters(&s.model, w, false).is_empty(), "Cut no longer offers what cuts already");
    assert_eq!(cutters(&s.model, w, true).len(), 1);
    // The flag off, the tie sleeps and Uncut still offers it by its outline.
    run(&mut s, "family", json!({"name": "Hole", "options": {"cut_with_voids_when_loaded": false}}));
    let asleep = cutters(&s.model, w, true);
    assert_eq!((asleep.len(), asleep[0].edges.len()), (1, 4));
    // Its void taken out of the family: no outline left, only its place, which Uncut picks it by.
    run(
        &mut s,
        "family",
        json!({"name": "Hole", "forms": [{"profile": [[-50, -50], [50, -50], [50, 50], [-50, 50]], "from": 0, "to": 50}]}),
    );
    let bare = cutters(&s.model, w, true);
    assert!(bare.len() == 1 && bare[0].edges.is_empty(), "{bare:?}");
    assert_eq!(pick(&bare, P2(3004.0, 0.0), 10.0), Some(c));
    run(&mut s, "uncut_geometry", json!({"target": w, "cutter": c}));
    assert!(cutters(&s.model, w, true).is_empty());
}
