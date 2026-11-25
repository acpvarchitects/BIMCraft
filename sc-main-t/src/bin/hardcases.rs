//! Hard cases for solidcore from its git main (through solidcore-ops, its application API):
//! one case per run, named by argv[1]; one JSON line out. Cases and answers: harness/hardcases.py.

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
use solidcore_ops::blend::{blend, Blend, Corners};
use solidcore_ops::body::{make_box, make_cylinder, Body};
use solidcore_ops::boolean::{boolean, BooleanOp};
use solidcore_ops::check::CheckLevel;
use solidcore_ops::offset::{offset_faces, shell};
use solidcore_ops::pick;
use solidcore_ops::sweep::{extrude, loft, loft_to_point, revolve, Curve, DraftCorner};
use std::f64::consts::{PI, TAU};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

type Out = Result<Option<(Body, Option<f64>)>, String>; // None: the result is empty
fn e(x: impl std::fmt::Display) -> String { x.to_string() }
fn line(a: [f64; 3], b: [f64; 3]) -> Curve { Curve::Line { start: a, end: b } }
fn poly(p: &[[f64; 3]]) -> Vec<Curve> { (0..p.len()).map(|i| line(p[i], p[(i + 1) % p.len()])).collect() }
fn xy(z: f64, p: &[(f64, f64)]) -> Vec<Curve> { poly(&p.iter().map(|q| [q.0, q.1, z]).collect::<Vec<_>>()) }
fn perp(d: [f64; 3]) -> [f64; 3] {
    let a = if d[2].abs() < 0.9 { [0., 0., 1.] } else { [1., 0., 0.] };
    let c = [d[1] * a[2] - d[2] * a[1], d[2] * a[0] - d[0] * a[2], d[0] * a[1] - d[1] * a[0]];
    let n = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
    [c[0] / n, c[1] / n, c[2] / n]
}
fn circle(c: [f64; 3], axis: [f64; 3], r: f64) -> Curve { Curve::Arc { center: c, axis, reference: perp(axis), radius: r, start: 0.0, end: TAU } }
fn bx(a: [f64; 3], b: [f64; 3]) -> Result<Body, String> { make_box(a, [b[0] - a[0], b[1] - a[1], b[2] - a[2]]).map_err(e) }
/// a cylinder on any axis: a circle extruded (the primitive is on +z only)
fn cyl(o: [f64; 3], d: [f64; 3], r: f64, h: f64) -> Result<Body, String> { extrude(&[circle(o, d, r)], d, h, 0.0, DraftCorner::Curvature, true).map_err(e) }
fn ext(p: &[Curve], d: [f64; 3], h: f64) -> Result<Body, String> { extrude(p, d, h, 0.0, DraftCorner::Curvature, true).map_err(e) }
/// a profile in the XZ plane (x, z) turned a full turn about z
fn rev(p: &[(f64, f64)]) -> Result<Body, String> { revolve(&poly(&p.iter().map(|q| [q.0, 0.0, q.1]).collect::<Vec<_>>()), [0., 0., 0.], [0., 0., 1.], TAU, true).map_err(e) }
fn op(a: Body, b: Body, o: BooleanOp) -> Out { boolean(a, b, o).map(|r| r.map(|b| (b, None))).map_err(e) }
fn cut(a: Body, b: Body) -> Out { op(a, b, BooleanOp::Subtract) }
fn fuse(a: Body, b: Body) -> Out { op(a, b, BooleanOp::Union) }
fn some(b: Body) -> Out { Ok(Some((b, None))) }
fn vol(b: &Body) -> Option<f64> { b.volume(1e-9).ok() }
fn sq(z: f64, h: f64) -> Vec<Curve> { xy(z, &[(h, h), (-h, h), (-h, -h), (h, -h)]) }
/// a circle as four quarter arcs from 45 degrees, matching a square's four sides
fn circle4(c: [f64; 3], r: f64) -> Vec<Curve> { (0..4).map(|i| { let a = PI / 4. + PI / 2. * i as f64; Curve::Arc { center: c, axis: [0., 0., 1.], reference: [1., 0., 0.], radius: r, start: a, end: a + PI / 2. } }).collect() }
fn ruled_loft(dx: f64) -> Result<Body, String> {
    let s = xy(0., &[(dx + 250., 250.), (dx - 250., 250.), (dx - 250., -250.), (dx + 250., -250.)]);
    loft(&[s, circle4([dx, 0., 900.], 200.)], 1, true).map_err(e)
}
fn smooth_loft() -> Result<Body, String> { loft(&[sq(0., 250.), circle4([0., 0., 450.], 300.), circle4([0., 0., 900.], 200.)], 2, true).map_err(e) }
fn pick_edge(b: &Body, p: [f64; 3]) -> Result<usize, String> {
    match pick::edges(b, &[p], 1.0).map_err(e)? { Ok(v) => Ok(v[0]), Err(m) => Err(format!("no edge picked at {p:?}: {m:?}")) }
}
fn pick_face(b: &Body, p: [f64; 3]) -> Result<usize, String> {
    match pick::faces(b, &[p], 1.0).map_err(e)? { Ok(v) => Ok(v[0]), Err(m) => Err(format!("no face picked at {p:?}: {m:?}")) }
}
fn round(b: Body, ed: usize, r: f64) -> Result<Body, String> { blend(&b, Blend::Round, &[ed], &[r], &[], Corners::Filled).map_err(e)?; Ok(b) }
fn inclined_cutter(t: f64) -> Result<Body, String> {
    let p = poly(&[[-400., -400., 250. - 400. * t], [400., -400., 250. + 400. * t], [400., -400., 900.], [-400., -400., 900.]]);
    ext(&p, [0., 1., 0.], 800.)
}
fn slope_cutter(y0: f64, s: f64) -> Result<Body, String> { ext(&xy(-10., &[(-10., y0 - 10. * s), (1010., y0 + 1010. * s), (1010., 700.), (-10., 700.)]), [0., 0., 1.], 770.) }

fn run(name: &str) -> Option<Out> {
    let s01 = 0.1f64.to_radians().tan();
    let t30 = 30f64.to_radians().tan();
    let rect = || xy(0., &[(-200., -100.), (200., -100.), (200., 100.), (-200., 100.)]);
    Some(match name {
        "1a loft rect to apex" => (|| some(loft_to_point(&rect(), [0., 0., 300.]).map_err(e)?))(),
        "1b loft side shrinks to zero" => (|| some(loft(&[rect(), xy(300., &[(-100., 0.), (100., -50.), (100., 50.), (-100., 0.)])], 1, true).map_err(e)?))(),
        "1c revolve profile touching axis" => (|| some(rev(&[(0., 0.), (300., 0.), (300., 500.), (0., 500.)])?))(),
        "1d revolve profile crossing axis" => (|| some(rev(&[(-100., 0.), (300., 0.), (300., 500.), (-100., 500.)])?))(),
        "1e revolve cone apex on axis" => (|| some(rev(&[(0., 0.), (300., 0.), (0., 500.)])?))(),
        "1f cone cut through its apex" => (|| cut(rev(&[(0., 0.), (300., 0.), (0., 500.)])?, bx([0., -400., -10.], [400., 400., 600.])?))(),
        "1g cone apex inside a box fuse" => (|| fuse(rev(&[(0., 0.), (300., 0.), (0., 500.)])?, bx([-50., -50., 450.], [50., 50., 550.])?))(),
        "2a cylinder tangent to a face fuse" => (|| fuse(bx([0., 0., 0.], [1000., 600., 200.])?, cyl([0., 300., 300.], [1., 0., 0.], 100., 1000.)?))(),
        "2b cylinder tangent inside a face cut" => (|| cut(bx([0., 0., 0.], [1000., 600., 750.])?, cyl([100., 300., -100.], [0., 0., 1.], 100., 950.)?))(),
        "2c equal cylinders crossing at 90deg fuse" => (|| fuse(cyl([-500., 0., 0.], [1., 0., 0.], 100., 1000.)?, cyl([0., -500., 0.], [0., 1., 0.], 100., 1000.)?))(),
        "2d coplanar faces partly overlapping fuse" => (|| fuse(bx([0., 0., 0.], [1000., 600., 750.])?, bx([500., 300., 0.], [1500., 900., 750.])?))(),
        "2e boxes stacked sharing a face fuse" => (|| fuse(bx([0., 0., 0.], [1000., 600., 750.])?, bx([0., 0., 750.], [1000., 600., 1000.])?))(),
        "2f void face on a body face cut" => (|| cut(bx([0., 0., 0.], [1000., 600., 750.])?, bx([200., 100., 0.], [800., 500., 400.])?))(),
        "3a cup wall 1mm" | "3b cup wall 0.1mm" | "3c cup wall 0.01mm" => {
            let t = match &name[..2] { "3a" => 1.0, "3b" => 0.1, _ => 0.01 };
            (|| cut(bx([0., 0., 0.], [1000., 600., 750.])?, bx([t, t, t], [1000. - t, 600. - t, 800.])?))()
        }
        "3d sliver 0.01mm from a near-miss cut" => (|| cut(bx([0., 0., 0.], [1000., 600., 750.])?, bx([-10., -10., -10.], [999.99, 610., 760.])?))(),
        "3e plane cut at 0.1deg" => (|| cut(bx([0., 0., 0.], [1000., 600., 750.])?, slope_cutter(300., s01)?))(),
        "3f wedge of 0.1deg" => (|| cut(bx([0., 0., 0.], [1000., 600., 750.])?, slope_cutter(0., s01)?))(),
        "3g box minus a thin NURBS panel" => (|| {
            let r = |z: f64, c: f64| xy(z, &[(100., c - 1.), (900., c - 1.), (900., c + 1.), (100., c + 1.)]);
            let p = loft(&[r(50., 200.), r(375., 300.), r(700., 200.)], 2, true).map_err(e)?;
            let pv = vol(&p);
            Ok(cut(bx([0., 0., 0.], [1000., 600., 750.])?, p)?.map(|(b, _)| (b, pv)))
        })(),
        "4a cylinder minus a cylinder at 45deg" => (|| { let h = 0.5f64.sqrt(); cut(cyl([0., 0., -1000.], [0., 0., 1.], 100., 2000.)?, cyl([-1000. * h, 0., -1000. * h], [h, 0., h], 100., 2000.)?) })(),
        "4b ruled loft minus a cylinder" => (|| { let l = ruled_loft(0.)?; let lv = vol(&l); Ok(cut(l, cyl([0., 0., -10.], [0., 0., 1.], 100., 920.)?)?.map(|(b, _)| (b, lv))) })(),
        "4c two lofts fused" => (|| { let a = ruled_loft(0.)?; let av = vol(&a); Ok(fuse(a, ruled_loft(300.)?)?.map(|(b, _)| (b, av))) })(),
        "4d revolved ring cut by an inclined plane" => (|| cut(rev(&[(100., 0.), (300., 0.), (300., 500.), (100., 500.)])?, inclined_cutter(t30)?))(),
        "4e cylinder cut by an inclined plane" => (|| cut(make_cylinder([0., 0., 0.], 300., 500.).map_err(e)?, inclined_cutter(t30)?))(),
        "5a fillet plane-plane" => (|| { let b = bx([0., 0., 0.], [1000., 600., 750.])?; let ed = pick_edge(&b, [1000., 600., 375.])?; some(round(b, ed, 50.)?) })(),
        "5b fillet plane-cylinder" => (|| {
            let Some((b, _)) = fuse(bx([-500., -500., 0.], [500., 500., 200.])?, make_cylinder([0., 0., 200.], 100., 400.).map_err(e)?)? else { return Err("the fuse came back empty".into()) };
            let ed = pick_edge(&b, [100. * 1f64.cos(), 100. * 1f64.sin(), 200.])?;
            some(round(b, ed, 20.)?)
        })(),
        "5c fillet NURBS-plane" => (|| { let b = smooth_loft()?; let bv = vol(&b); let ed = pick_edge(&b, [200. * 1f64.cos(), 200. * 1f64.sin(), 900.])?; Ok(Some((round(b, ed, 20.)?, bv))) })(),
        "5d fillet NURBS-NURBS" => (|| {
            let b = smooth_loft()?; let bv = vol(&b);
            let order = b.own_order();
            let n = b.counts().map_err(e)?[1];
            let mut best: Option<(f64, usize)> = None;
            for i in 0..n {
                let Ok(p) = b.edge_middle(&order, i) else { continue };
                if p[2] > 100. && p[2] < 800. && p[0] > 0. && p[1] > 0. { let d = (p[0] - p[1]).abs(); if best.map_or(true, |x| d < x.0) { best = Some((d, i)); } }
            }
            let Some((_, ed)) = best else { return Err("no side edge between two loft faces (the loft has one smooth side face)".into()) };
            Ok(Some((round(b, ed, 20.)?, bv)))
        })(),
        "5e shell a cylinder open at the top" => (|| { let b = make_cylinder([0., 0., 0.], 300., 500.).map_err(e)?; let top = pick_face(&b, [10., 10., 500.])?; shell(&b, &b.own_order(), &[top], -10.).map_err(e)?; some(b) })(),
        "5f offset a cylindrical face" => (|| { let b = make_cylinder([0., 0., 0.], 300., 500.).map_err(e)?; let side = pick_face(&b, [300. * 1f64.cos(), 300. * 1f64.sin(), 250.])?; offset_faces(&b, &b.own_order(), &[side], 10.).map_err(e)?; some(b) })(),
        "6a box with a NaN corner" => (|| some(bx([0., f64::NAN, 0.], [1000., 600., 750.])?))(),
        "6b box of zero thickness" => (|| some(bx([0., 0., 0.], [1000., 600., 0.])?))(),
        "6c box with inverted corners" => (|| some(bx([1000., 600., 750.], [0., 0., 0.])?))(),
        "6d cylinder of radius 0" => (|| some(make_cylinder([0., 0., 0.], 0., 750.).map_err(e)?))(),
        "6e cylinder of radius NaN" => (|| some(make_cylinder([0., 0., 0.], f64::NAN, 750.).map_err(e)?))(),
        "6f cylinder of negative radius" => (|| some(make_cylinder([0., 0., 0.], -100., 750.).map_err(e)?))(),
        "6g unit box at 1e9 from the origin" => (|| some(bx([1e9, 1e9, 1e9], [1e9 + 1., 1e9 + 1., 1e9 + 1.])?))(),
        "6h box fused with itself" => (|| fuse(bx([0., 0., 0.], [1000., 600., 750.])?, bx([0., 0., 0.], [1000., 600., 750.])?))(),
        "6i box cut by itself" => (|| cut(bx([0., 0., 0.], [1000., 600., 750.])?, bx([0., 0., 0.], [1000., 600., 750.])?))(),
        "6j extrude a NaN profile" => (|| some(ext(&xy(0., &[(0., 0.), (f64::NAN, 0.), (1., 1.)]), [0., 0., 1.], 750.)?))(),
        "6k extrude by zero" => (|| some(ext(&xy(0., &[(0., 0.), (10., 0.), (10., 10.)]), [0., 0., 1.], 0.)?))(),
        "6l extrude a self-intersecting profile" => (|| some(ext(&xy(0., &[(0., 0.), (10., 10.), (10., 0.), (0., 10.)]), [0., 0., 1.], 5.)?))(),
        _ => return None,
    })
}

fn esc(s: &str) -> String { s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ") }
fn num(x: Option<f64>) -> String { match x { Some(v) if v.is_finite() => format!("{v:.17e}"), _ => "null".into() } }

fn main() {
    let name = std::env::args().nth(1).expect("case name");
    std::panic::set_hook(Box::new(|_| {}));
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| run(&name)));
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let head = format!("{{\"case\":\"{}\",\"kernel\":\"solidcore main\",\"ms\":{ms:.3}", esc(&name));
    match r {
        Err(p) => { let s = p.downcast_ref::<String>().cloned().or(p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default(); println!("{head},\"outcome\":\"panic\",\"err\":\"{}\"}}", esc(&s)); }
        Ok(None) => { eprintln!("unknown case"); std::process::exit(2); }
        Ok(Some(Err(x))) => println!("{head},\"outcome\":\"error\",\"err\":\"{}\"}}", esc(&x.chars().take(300).collect::<String>())),
        Ok(Some(Ok(None))) => println!("{head},\"outcome\":\"empty\",\"faces\":0}}"),
        Ok(Some(Ok(Some((b, rf))))) => {
            let [faces, edges, _] = b.counts().unwrap_or([0, 0, 0]);
            if faces == 0 { println!("{head},\"outcome\":\"empty\",\"faces\":0}}"); return; }
            let valid = catch_unwind(AssertUnwindSafe(|| b.check(CheckLevel::All).map(|r| r.is_valid()).ok())).ok().flatten();
            let v = catch_unwind(AssertUnwindSafe(|| vol(&b))).ok().flatten();
            let mesh = match catch_unwind(AssertUnwindSafe(|| -> Result<String, String> {
                let (rec, _) = b.to_record().map_err(e)?;
                let (plan, _) = solidcore_tess::mesh(&rec, &solidcore_tess::MeshSpec::chord(0.5), 1000.0, None).map_err(e)?;
                let (w, refused) = plan.welded().map_err(e)?;
                Ok(format!("ok {} tris{}", w.triangles.len(), if refused.is_empty() { String::new() } else { format!(", {} faces refused", refused.len()) }))
            })) { Ok(Ok(s)) => s, Ok(Err(x)) => format!("err {}", esc(&x.chars().take(120).collect::<String>())), Err(_) => "err panic".into() };
            println!("{head},\"outcome\":\"solid\",\"volume\":{},\"vsrc\":\"exact\",\"ref\":{},\"faces\":{faces},\"edges\":{edges},\"valid\":{},\"mesh\":\"{mesh}\",\"step\":\"not wired\"}}",
                num(v), num(rf), valid.map(|x| x.to_string()).unwrap_or("null".into()));
        }
    }
}
