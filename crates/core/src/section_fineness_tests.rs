//! GEO-2c-2a: sections and elevations build their components' curves at the fineness their scale asks for, as plans do
//! (GEO-2c): what they show within about half a pixel, the cut and the picture under it of the same builds, kept across
//! drawings in the plans' store; at a scale that asks for the default, every pixel and line as before.

use serde_json::{Value, json};

use crate::component_geometry::{builds, fine_builds, without_fineness};
use crate::component_paint_tests::gstr;
use crate::engine::Session;
use crate::family::Fineness;
use crate::geom::P2;
use crate::ids::Id;
use crate::render::{self, Prim, View};

/// The family's Center (Front/Back) plane: a sketch on it is in x and z, and goes from its `from` to its `to` down y.
const FRONT: &str = "9e3c0a10-1f0b-4d8c-8a5e-0000000000a2";
/// The cylinders' radius: 32 chords a turn stray 3.85 mm from it.
const R: f64 = 800.0;

fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

fn id(v: &Value) -> Id {
    v["id"].as_u64().unwrap_or_else(|| panic!("no id: {v}"))
}

/// Generic family `name` of `forms`, one type `T` and an instance parameter `K` that changes nothing but its key.
fn family(s: &mut Session, name: &str, forms: Vec<Value>) {
    run(
        s,
        "family",
        json!({"name": name, "category": "generic", "parameters": [{"name": "K", "value": 0, "instance": true}],
               "types": {"T": {}}, "forms": forms}),
    );
}

/// A component of family `name` at `at`, its `K` `k`.
fn place(s: &mut Session, name: &str, at: [f64; 2], k: f64) -> Id {
    id(&run(s, "component", json!({"family": name, "type": "T", "at": at, "params": {"K": k}})))
}

/// A cylinder of radius [`R`] lying along y from 0 to 2000, its axis at x = 0 and z = 1000: its round ends face south and
/// north.
fn lying() -> Value {
    json!({"guid": gstr(700), "sketch": [{"centre": [0, 1000], "radius": R}], "host": {"plane": FRONT}, "from": -2000, "to": 0})
}

/// A cylinder of radius [`R`] standing from 0 to 2000 round x = 0, y = 1000.
fn standing() -> Value {
    json!({"sketch": [{"centre": [0, 1000], "radius": R}], "from": 0, "to": 2000})
}

/// A section from `start` to `end` (looking left of its way: north for a line running east), as deep as `depth`.
fn section(s: &mut Session, start: [f64; 2], end: [f64; 2], depth: f64) -> Id {
    id(&run(s, "section", json!({"start": start, "end": end, "depth": depth})))
}

/// Every edge drawing `d` (through `view`) draws of `owner` with both its ends on the screen, in the drawing's mm: a fill's
/// rings, closed, and lines.
fn edges(d: &render::Drawing, view: &View, owner: Id) -> Vec<[P2; 2]> {
    let on = |(x, y): (f32, f32)| (0.0..=view.width as f32).contains(&x) && (0.0..=view.height as f32).contains(&y);
    let mut out = Vec::new();
    for (p, o) in d.prims.iter().zip(&d.owners) {
        if *o != Some(owner) {
            continue;
        }
        let rings: Vec<(Vec<(f32, f32)>, bool)> = match p {
            Prim::Fill { rings, .. } => rings.iter().map(|r| (r.clone(), true)).collect(),
            Prim::Line { pts, .. } => vec![(pts.clone(), false)],
            _ => continue,
        };
        for (ring, closed) in rings {
            let n = ring.len();
            let last = if closed { n } else { n.saturating_sub(1) };
            for k in 0..last {
                let (a, b) = (ring[k], ring[(k + 1) % n]);
                if on(a) && on(b) && a != b {
                    out.push([view.mm(a), view.mm(b)]);
                }
            }
        }
    }
    out
}

/// The chords a whole turn of a circle of radius [`R`] round `centre` takes, as `edges` draw it: the least angle between
/// two of their corners on it.
fn chords_per_turn(edges: &[[P2; 2]], centre: P2) -> usize {
    let mut angles: Vec<f64> =
        edges.iter().flatten().filter(|p| (p.dist(centre) - R).abs() < 0.01).map(|p| (p.1 - centre.1).atan2(p.0 - centre.0)).collect();
    angles.sort_by(f64::total_cmp);
    angles.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let least = angles.windows(2).map(|w| w[1] - w[0]).fold(f64::INFINITY, f64::min);
    assert!(least.is_finite(), "no two corners on the circle among {} edges", edges.len());
    (std::f64::consts::TAU / least).round() as usize
}

/// A cylinder lying across a section's line, cut by it zoomed in to 4 px a mm on its right: the cut's corners within half a
/// pixel of the circle, and each chord between them too. The default's 32 chords a turn stray some 15 px.
#[test]
fn a_sections_chords_lie_within_half_a_pixel_zoomed_in() {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    let c = place(&mut s, "Lying", [0.0, 0.0], 0.0);
    // Across its middle, looking north: its cut is the circle, round 3000 along the line and 1000 up.
    let sec = section(&mut s, [-3000.0, 1000.0], [3000.0, 1000.0], 3000.0);
    let scale = 4.0;
    let view = View { angle: 0.0, scale, cx: 3000.0 + R - 50.0, cy: 1000.0, width: 800, height: 1600 };
    let cut = edges(&render::section(&s.model, sec, view), &view, c);
    let (centre, half) = (P2(3000.0, 1000.0), 0.5 / scale);
    let on_it: Vec<&[P2; 2]> = cut.iter().filter(|[a, b]| (a.dist(centre) - R).abs() < 20.0 && (b.dist(centre) - R).abs() < 20.0).collect();
    assert!(on_it.len() >= 2, "the cut's edges round the circle on the screen: {cut:?}");
    let (mut off, mut sagitta) = (0f64, 0f64);
    for [a, b] in on_it {
        let mid = P2((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        off = off.max((a.dist(centre) - R).abs()).max((b.dist(centre) - R).abs());
        sagitta = sagitta.max(R - mid.dist(centre));
    }
    assert!(off <= half + 1e-3, "a point of the cut {:.2} px off the circle", off * scale);
    assert!(sagitta <= half + 1e-3, "a chord {:.2} px inside the circle", sagitta * scale);
}

/// The same cylinder's round end in an elevation, zoomed in to 4 px a mm on its right: the picture shows it out to the
/// circle, on every row the middle of the last pixel it covers within a pixel and a half of it (half a pixel of chord, a
/// pixel of raster). The default's chords leave some 15 px.
#[test]
fn an_elevations_picture_draws_a_round_face_within_half_a_pixel_zoomed_in() {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    let c = place(&mut s, "Lying", [0.0, 0.0], 0.0);
    // Looking north at its south end, from y = -1000; its line from x = -1000.
    let sec = section(&mut s, [-1000.0, -1000.0], [3000.0, -1000.0], 5000.0);
    let scale = 4.0;
    let (w, h) = (400u32, 1600u32);
    let view = View { angle: 0.0, scale, cx: 1000.0 + R - 30.0, cy: 1000.0, width: w, height: h };
    let img = render::section_picture(&s.model, sec, view, &[]).unwrap_or_else(|| panic!("a picture"));
    let column = |x: f64| f64::from(w) / 2.0 + (x + 1000.0 - view.cx) * scale;
    let mut rows = 0;
    let mut worst: f64 = 0.0;
    for y in 0..h {
        let z = view.cy - (f64::from(y) + 0.5 - f64::from(h) / 2.0) / scale;
        let dz = z - 1000.0;
        let edge = column((R * R - dz * dz).max(0.0).sqrt());
        if dz.abs() >= R - 1.0 || !(2.0..f64::from(w) - 2.0).contains(&edge) {
            continue;
        }
        // The rightmost pixel the cylinder shows on this row.
        let Some(last) = (0..w).rev().find(|x| img.pick(f64::from(*x) + 0.5, f64::from(y) + 0.5) == Some(c)) else {
            panic!("row {y} shows nothing of it")
        };
        // Its chords lie inside the circle: no pixel's middle outside it.
        assert!(f64::from(last) + 0.5 <= edge + 0.01, "row {y}: a pixel past the circle");
        worst = worst.max(edge - (f64::from(last) + 0.5));
        rows += 1;
    }
    assert!(rows > 100, "{rows} rows across its edge");
    assert!(worst <= 1.5 + 1e-6, "its edge {worst:.2} px from the circle");
}

/// A plan and a section at the same scale ask the same fineness: the standing cylinder's circle in plan and the lying one's
/// in the section's cut take as many chords a turn, and a section of the standing one after the plan builds nothing finer,
/// the plan's build being kept for it.
#[test]
fn a_plan_and_a_section_at_the_same_scale_chord_alike_and_share_their_builds() {
    let mut s = Session::new();
    family(&mut s, "Standing", vec![standing()]);
    family(&mut s, "Lying", vec![lying()]);
    let up = place(&mut s, "Standing", [0.0, 0.0], 0.0);
    let down = place(&mut s, "Lying", [20_000.0, 0.0], 0.0);
    let scale = 2.0;
    // The plan at its circle's right, at 2 px a mm.
    let plan = View { angle: 0.0, scale, cx: R - 50.0, cy: 1000.0, width: 600, height: 1600 };
    let in_plan = chords_per_turn(&edges(&render::draw(&s.model, 100000, plan), &plan, up), P2(0.0, 1000.0));
    // The section across the lying one, at its circle's right, at the same scale.
    let across = section(&mut s, [17_000.0, 1000.0], [23_000.0, 1000.0], 3000.0);
    let view = View { angle: 0.0, scale, cx: 3000.0 + R - 50.0, cy: 1000.0, width: 600, height: 1600 };
    let in_section = chords_per_turn(&edges(&render::section(&s.model, across, view), &view, down), P2(3000.0, 1000.0));
    assert!(in_plan > 32, "finer than the default in plan: {in_plan}");
    assert_eq!(in_section, in_plan, "chords a turn in the section and in the plan");
    // A section looking north at the standing one, at the same scale over the same part of it: nothing finer built again.
    let north = section(&mut s, [-3000.0, -1000.0], [3000.0, -1000.0], 5000.0);
    render::draw(&s.model, 100000, plan);
    let before = fine_builds();
    let view = View { angle: 0.0, scale, cx: 3000.0 + R - 50.0, cy: 1000.0, width: 600, height: 1600 };
    render::section_picture_and_over(&s.model, north, view, view, &[]);
    assert_eq!(fine_builds() - before, 0, "the plan's finer build, kept, drawn in the section");
}

/// A family of no curve a fitted view shows past half a pixel: a box with a round hole through its front and a short post on
/// it, 60 and 100 mm round.
fn small_curves(s: &mut Session) -> Id {
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| {
        json!([{"start": [x0, y0], "end": [x1, y0]}, {"start": [x1, y0], "end": [x1, y1]},
               {"start": [x1, y1], "end": [x0, y1]}, {"start": [x0, y1], "end": [x0, y0]}])
    };
    family(
        s,
        "Small",
        vec![
            json!({"sketch": rect(0.0, 0.0, 1000.0, 500.0), "from": 0, "to": 300}),
            json!({"sketch": [{"centre": [500, 150], "radius": 60}], "host": {"plane": FRONT}, "from": -100, "to": 600, "void": true}),
            json!({"sketch": [{"centre": [500, 250], "radius": 100}], "from": 300, "to": 900}),
        ],
    );
    place(s, "Small", [0.0, 0.0], 0.0)
}

/// The fit view's guard: at a scale whose fineness is the default, and fitted to a family whose curves are within half a
/// pixel at the default, a section's picture, its picking and its drawing are those it drew before GEO-2c-2a, to the pixel
/// and the line; and nothing finer is built.
#[test]
fn at_a_scale_that_asks_for_the_default_a_section_is_as_it_was() {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    place(&mut s, "Lying", [0.0, 0.0], 0.0);
    small_curves(&mut s);
    let sec = section(&mut s, [-3000.0, 250.0], [3000.0, 250.0], 3000.0);
    let fit = render::section_fit(&s.model, sec, 1000, 800);
    let far = View { scale: 0.0019, ..fit };
    assert_eq!(Fineness::for_view(far.scale), Fineness::DEFAULT);
    let small = {
        let mut s = Session::new();
        small_curves(&mut s);
        let sec = section(&mut s, [-500.0, 250.0], [1500.0, 250.0], 3000.0);
        let fit = render::section_fit(&s.model, sec, 1000, 800);
        assert!(Fineness::for_view(fit.scale) != Fineness::DEFAULT, "fitted, a fineness: {fit:?}");
        (s, sec, fit)
    };
    for (s, sec, view) in [(&s, sec, far), (&small.0, small.1, small.2)] {
        let before = fine_builds();
        let (picture, drawing) = render::section_picture_and_over(&s.model, sec, view, view, &[]);
        assert_eq!(fine_builds() - before, 0, "nothing finer at {} px/mm", view.scale);
        let (was, drawn) = without_fineness(|| render::section_picture_and_over(&s.model, sec, view, view, &[]));
        let (picture, was) = (picture.unwrap_or_else(|| panic!("a picture")), was.unwrap_or_else(|| panic!("a picture")));
        assert!(picture.rgba == was.rgba, "its pixels at {} px/mm", view.scale);
        for y in 0..picture.height {
            for x in 0..picture.width {
                let (x, y) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                assert_eq!((picture.pick(x, y), picture.face(x, y)), (was.pick(x, y), was.face(x, y)));
            }
        }
        assert_eq!(render::svg(&drawing), render::svg(&drawn), "its drawing at {} px/mm", view.scale);
    }
}

/// A section drawn again builds nothing: its default builds and its finer ones are kept across drawings, as a plan's are. A
/// pan, or a zoom within a power of two, builds nothing either; a zoom past one builds the one component on the screen whose
/// curves change, not the box beside it nor the cylinder off the screen.
#[test]
fn a_section_drawn_again_panned_or_zoomed_a_little_builds_nothing() {
    render::without_twins(|| {
        let mut s = Session::new();
        family(&mut s, "Lying", vec![lying()]);
        place(&mut s, "Lying", [0.0, 0.0], 0.0);
        place(&mut s, "Lying", [30_000.0, 0.0], 1.0);
        // A box of no curves beside it, on the screen.
        family(&mut s, "Plain", vec![json!({"profile": [[0, 0], [1000, 0], [1000, 500], [0, 500]], "to": 600})]);
        place(&mut s, "Plain", [R + 50.0, 800.0], 0.0);
        let sec = section(&mut s, [-3000.0, 1000.0], [40_000.0, 1000.0], 3000.0);
        let built = |v: View| {
            let before = builds();
            render::section_picture_and_over(&s.model, sec, v, v, &[]);
            builds() - before
        };
        let view = |scale: f64, cx: f64| View { angle: 0.0, scale, cx, cy: 1000.0, width: 600, height: 1600 };
        let cx = 3000.0 + R;
        assert!(built(view(2.0, cx)) > 0, "the first drawing builds");
        assert_eq!(built(view(2.0, cx)), 0, "drawn again");
        assert_eq!(built(view(2.0, cx - 40.0)), 0, "panned a little");
        assert_eq!(built(view(1.6, cx)), 0, "zoomed out within the power of two");
        let (all, fine) = (builds(), fine_builds());
        built(view(3.0, cx));
        assert_eq!((builds() - all, fine_builds() - fine), (1, 1), "zoomed in past it: the cylinder on the screen, finer");
    });
}

/// How dark `img` is at pixel `(x, y)`: the darkest of the pixels within two of it up and down, as the sum of its channels.
fn darkest(img: &crate::view3d::Image, x: f64, y: f64) -> u32 {
    (-2..=2)
        .map(|k| {
            let c = img.color(x, y + f64::from(k));
            u32::from(c.0) + u32::from(c.1) + u32::from(c.2)
        })
        .min()
        .unwrap_or(0)
}

/// FE-16c-2 with GEO-2c-2a: the lying cylinder's south end painted a material of courses 100 mm apart, in an elevation
/// zoomed in to 4 px a mm: each course shows a millimetre inside the circle, where the default's chords would leave a sliver
/// out of the picture. Unpainted, paper there.
#[test]
fn a_painted_round_face_shows_its_pattern_out_to_its_curve_in_an_elevation_zoomed_in() {
    let mut s = Session::new();
    run(&mut s, "fill_pattern", json!({"name": "Tiles", "target": "model", "lines": [{"angle": 0, "spacing": 100}]}));
    run(&mut s, "material", json!({"name": "Tile", "color": [200, 200, 200], "surface": "Tiles"}));
    family(&mut s, "Lying", vec![lying()]);
    let c = place(&mut s, "Lying", [0.0, 0.0], 0.0);
    let sec = section(&mut s, [-1000.0, -1000.0], [3000.0, -1000.0], 5000.0);
    let scale = 4.0;
    let (w, h) = (400u32, 2800u32);
    let view = View { angle: 0.0, scale, cx: 1000.0 + R - 30.0, cy: 1000.0, width: w, height: h };
    // A millimetre in from the circle on each course near its right.
    let at = |z: f64| {
        let x = (R * R - (z - 1000.0).powi(2)).sqrt() - 1.0;
        (f64::from(w) / 2.0 + (x + 1000.0 - view.cx) * scale, f64::from(h) / 2.0 - (z - view.cy) * scale)
    };
    let courses = [700.0, 800.0, 900.0, 1100.0, 1200.0, 1300.0];
    let picture = |s: &Session| render::section_picture(&s.model, sec, view, &[]).unwrap_or_else(|| panic!("a picture"));
    let face = s.model.component_face(c, [R - 10.0, 0.0, 1000.0], [0.0, -1.0, 0.0]).unwrap_or_else(|| panic!("its south end")).face;
    run(&mut s, "paint", json!({"id": c, "form": gstr(700), "face": face.clone(), "material": "Tile"}));
    let img = picture(&s);
    for z in courses {
        let (x, y) = at(z);
        assert!(darkest(&img, x, y) < 600, "the course at {z}, a millimetre inside the circle");
    }
    // Unpainted, no ink there: what was dark was the pattern, not an edge.
    run(&mut s, "paint", json!({"id": c, "form": gstr(700), "face": face, "material": null}));
    let img = picture(&s);
    for z in courses {
        let (x, y) = at(z);
        assert!(darkest(&img, x, y) > 700, "paper at {z} unpainted");
    }
}

/// A section's view of a scale that is no number, or none, draws as the default does and builds nothing finer; one nearer
/// than anything draws within the finest fineness, and neither panics.
#[test]
fn a_section_at_a_hostile_scale_draws_at_the_default_or_the_finest() {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    place(&mut s, "Lying", [0.0, 0.0], 0.0);
    let sec = section(&mut s, [-3000.0, 1000.0], [3000.0, 1000.0], 3000.0);
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e-300, f64::MIN_POSITIVE, 1e300, f64::MAX] {
        let view = View { angle: 0.0, scale, cx: 3000.0 + R, cy: 1000.0, width: 200, height: 200 };
        let before = fine_builds();
        render::section_picture_and_over(&s.model, sec, view, view, &[]);
        render::section(&s.model, sec, view);
        if Fineness::for_view(scale) == Fineness::DEFAULT {
            assert_eq!(fine_builds() - before, 0, "nothing finer at {scale} px/mm");
        }
    }
}
