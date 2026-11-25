//! Hard cases for arris 0.12.0: one case per run, named by argv[1]; one JSON line out.
//! The case list and the expected answers live in harness/hardcases.py.
use arris::Control;
use arris::geom::profile::{Profile, ProfileLoop, ProfileSegment};
use arris::math::{Axis, Frame, Point2, Point3, Vec3};
use arris::ops::{self, LoftKind, OpError, ShellSide};
use arris::topo::{Body, Model, provenance::Provenance};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

const C: Control<'static> = Control::NONE;
type R = Result<(Body, Provenance), OpError>;
type Out = Result<(Body, Option<f64>), String>; // the result and, for some cases, its input's volume

fn fr(o: Point3, n: Vec3, x: Vec3) -> Frame { Frame::new(o, n, x).unwrap() }
fn xy(z: f64) -> Frame { fr(Point3::new(0.0, 0.0, z), Vec3::z(), Vec3::x()) }
/// the XZ plane through y = 0: u along x, v along z
fn xz() -> Frame { fr(Point3::origin(), -Vec3::y(), Vec3::x()) }
fn poly(plane: Frame, pts: &[(f64, f64)]) -> Profile {
    let mut segs: Vec<ProfileSegment> = pts[1..].iter().map(|p| ProfileSegment::LineTo(Point2::new(p.0, p.1))).collect();
    segs.push(ProfileSegment::LineTo(Point2::new(pts[0].0, pts[0].1)));
    Profile { plane, outer: ProfileLoop::Path { start: Point2::new(pts[0].0, pts[0].1), segments: segs }, holes: vec![] }
}
fn circ(plane: Frame, r: f64) -> Profile { Profile { plane, outer: ProfileLoop::Circle { center: Point2::origin(), radius: r }, holes: vec![] } }
fn e(x: OpError) -> String { format!("{x}") }
fn bx(m: &mut Model, a: [f64; 3], b: [f64; 3]) -> Result<Body, String> { ops::primitive_box(m, a, b, &C).map(|r| r.0).map_err(e) }
fn cyl(m: &mut Model, o: [f64; 3], d: [f64; 3], r: f64, h: f64) -> Result<Body, String> {
    let ax = Axis::new(Point3::new(o[0], o[1], o[2]), Vec3::new(d[0], d[1], d[2])).map_err(|x| format!("{x:?}"))?;
    ops::primitive_cylinder(m, ax, r, h, &C).map(|r| r.0).map_err(e)
}
fn ext(m: &mut Model, p: &Profile, d: Vec3, h: f64) -> Result<Body, String> { ops::extrude(m, p, d, h, &C).map(|r| r.0).map_err(e) }
fn rev(m: &mut Model, pts: &[(f64, f64)]) -> Result<Body, String> {
    ops::revolve(m, &poly(xz(), pts), Axis::z_at(Point3::origin()), std::f64::consts::TAU, &C).map(|r| r.0).map_err(e)
}
fn cut(m: &mut Model, a: Body, b: Body) -> Result<Body, String> { ops::cut(m, a, b, &C).map(|r| r.0).map_err(e) }
fn fuse(m: &mut Model, a: Body, b: Body) -> Result<Body, String> { ops::fuse(m, a, b, &C).map(|r| r.0).map_err(e) }
fn vol(m: &Model, b: Body) -> Option<f64> { ops::measure::mass_properties(m, b, &C).ok().map(|p| p.volume) }
fn only(b: Body) -> Out { Ok((b, None)) }
fn sq(z: f64, h: f64) -> Profile { poly(xy(z), &[(h, h), (-h, h), (-h, -h), (h, -h)]) }
fn ruled_loft(m: &mut Model, dx: f64) -> Result<Body, String> {
    let f = |z: f64| fr(Point3::new(dx, 0.0, z), Vec3::z(), Vec3::x());
    ops::loft(m, &[poly(f(0.0), &[(250., 250.), (-250., 250.), (-250., -250.), (250., -250.)]), circ(f(900.0), 200.)], LoftKind::Ruled, false, &C).map(|r| r.0).map_err(e)
}
fn smooth_loft(m: &mut Model) -> Result<Body, String> {
    ops::loft(m, &[sq(0.0, 250.), circ(xy(450.), 300.), circ(xy(900.), 200.)], LoftKind::Smooth, false, &C).map(|r| r.0).map_err(e)
}
fn edge_mid(m: &Model, ed: &arris::topo::Edge) -> Option<Point3> {
    let (c, range) = m.edge(ed.id).ok()?.curve()?;
    Some(m.curve(c).ok()?.point(range.midpoint()))
}
fn edge_where(m: &Model, b: Body, pick: impl Fn(Point3) -> f64) -> Result<arris::topo::Edge, String> {
    m.edges(b).map_err(|x| format!("{x:?}"))?.into_iter().filter_map(|ed| edge_mid(m, &ed).map(|p| (pick(p), ed)))
        .min_by(|a, b| a.0.total_cmp(&b.0)).map(|x| x.1).ok_or("no edge".into())
}
fn face_at(m: &Model, b: Body, p: Point3) -> Result<arris::topo::Face, String> {
    let c = arris::check::classify::classify_point(m, b, p).map_err(|x| format!("{x:?}"))?;
    m.faces(b).map_err(|x| format!("{x:?}"))?.into_iter().find(|f| c == arris::check::classify::Classification::On(f.shape())).ok_or("no face at the point".into())
}
fn inclined_cutter(m: &mut Model, t: f64) -> Result<Body, String> {
    // the half-space above z = 250 + x t, as a prism in XZ extruded along y from -400 to 400
    let f = fr(Point3::new(0.0, -400.0, 0.0), -Vec3::y(), Vec3::x());
    let p = poly(f, &[(-400., 250. - 400. * t), (400., 250. + 400. * t), (400., 900.), (-400., 900.)]);
    ext(m, &p, Vec3::y(), 800.0)
}
fn slope_cutter(m: &mut Model, y0: f64, s: f64) -> Result<Body, String> {
    let p = poly(xy(-10.0), &[(-10., y0 - 10. * s), (1010., y0 + 1010. * s), (1010., 700.), (-10., 700.)]);
    ext(m, &p, Vec3::z(), 770.0)
}

fn run(name: &str, m: &mut Model) -> Option<Out> {
    let pi = std::f64::consts::PI;
    let s01 = 0.1f64.to_radians().tan();
    Some(match name {
        "1a loft rect to apex" => (|| only(ops::loft(m, &[poly(xy(0.), &[(-200., -100.), (200., -100.), (200., 100.), (-200., 100.)]), poly(xy(300.), &[(0., 0.), (0., 0.), (0., 0.), (0., 0.)])], LoftKind::Ruled, false, &C).map_err(e)?.0))(),
        "1b loft side shrinks to zero" => (|| only(ops::loft(m, &[poly(xy(0.), &[(-200., -100.), (200., -100.), (200., 100.), (-200., 100.)]), poly(xy(300.), &[(-100., 0.), (100., -50.), (100., 50.), (-100., 0.)])], LoftKind::Ruled, false, &C).map_err(e)?.0))(),
        "1c revolve profile touching axis" => (|| only(rev(m, &[(0., 0.), (300., 0.), (300., 500.), (0., 500.)])?))(),
        "1d revolve profile crossing axis" => (|| only(rev(m, &[(-100., 0.), (300., 0.), (300., 500.), (-100., 500.)])?))(),
        "1e revolve cone apex on axis" => (|| only(rev(m, &[(0., 0.), (300., 0.), (0., 500.)])?))(),
        "1f cone cut through its apex" => (|| { let c = rev(m, &[(0., 0.), (300., 0.), (0., 500.)])?; let b = bx(m, [0., -400., -10.], [400., 400., 600.])?; only(cut(m, c, b)?) })(),
        "1g cone apex inside a box fuse" => (|| { let c = rev(m, &[(0., 0.), (300., 0.), (0., 500.)])?; let b = bx(m, [-50., -50., 450.], [50., 50., 550.])?; only(fuse(m, c, b)?) })(),
        "2a cylinder tangent to a face fuse" => (|| { let a = bx(m, [0., 0., 0.], [1000., 600., 200.])?; let c = cyl(m, [0., 300., 300.], [1., 0., 0.], 100., 1000.)?; only(fuse(m, a, c)?) })(),
        "2b cylinder tangent inside a face cut" => (|| { let a = bx(m, [0., 0., 0.], [1000., 600., 750.])?; let c = cyl(m, [100., 300., -100.], [0., 0., 1.], 100., 950.)?; only(cut(m, a, c)?) })(),
        "2c equal cylinders crossing at 90deg fuse" => (|| { let a = cyl(m, [-500., 0., 0.], [1., 0., 0.], 100., 1000.)?; let b = cyl(m, [0., -500., 0.], [0., 1., 0.], 100., 1000.)?; only(fuse(m, a, b)?) })(),
        "2d coplanar faces partly overlapping fuse" => (|| { let a = bx(m, [0., 0., 0.], [1000., 600., 750.])?; let b = bx(m, [500., 300., 0.], [1500., 900., 750.])?; only(fuse(m, a, b)?) })(),
        "2e boxes stacked sharing a face fuse" => (|| { let a = bx(m, [0., 0., 0.], [1000., 600., 750.])?; let b = bx(m, [0., 0., 750.], [1000., 600., 1000.])?; only(fuse(m, a, b)?) })(),
        "2f void face on a body face cut" => (|| { let a = bx(m, [0., 0., 0.], [1000., 600., 750.])?; let b = bx(m, [200., 100., 0.], [800., 500., 400.])?; only(cut(m, a, b)?) })(),
        "3a cup wall 1mm" | "3b cup wall 0.1mm" | "3c cup wall 0.01mm" => {
            let t = match &name[..2] { "3a" => 1.0, "3b" => 0.1, _ => 0.01 };
            (|| { let a = bx(m, [0., 0., 0.], [1000., 600., 750.])?; let b = bx(m, [t, t, t], [1000. - t, 600. - t, 800.])?; only(cut(m, a, b)?) })()
        }
        "3d sliver 0.01mm from a near-miss cut" => (|| { let a = bx(m, [0., 0., 0.], [1000., 600., 750.])?; let b = bx(m, [-10., -10., -10.], [999.99, 610., 760.])?; only(cut(m, a, b)?) })(),
        "3e plane cut at 0.1deg" => (|| { let a = bx(m, [0., 0., 0.], [1000., 600., 750.])?; let c = slope_cutter(m, 300., s01)?; only(cut(m, a, c)?) })(),
        "3f wedge of 0.1deg" => (|| { let a = bx(m, [0., 0., 0.], [1000., 600., 750.])?; let c = slope_cutter(m, 0., s01)?; only(cut(m, a, c)?) })(),
        "3g box minus a thin NURBS panel" => (|| {
            let r = |z: f64, c: f64| poly(xy(z), &[(100., c - 1.), (900., c - 1.), (900., c + 1.), (100., c + 1.)]);
            let p = ops::loft(m, &[r(50., 200.), r(375., 300.), r(700., 200.)], LoftKind::Smooth, false, &C).map_err(e)?.0;
            let pv = vol(m, p);
            let a = bx(m, [0., 0., 0.], [1000., 600., 750.])?;
            Ok((cut(m, a, p)?, pv))
        })(),
        "4a cylinder minus a cylinder at 45deg" => (|| {
            let a = cyl(m, [0., 0., -1000.], [0., 0., 1.], 100., 2000.)?;
            let d = [0.5f64.sqrt(), 0., 0.5f64.sqrt()];
            let b = cyl(m, [-1000. * d[0], 0., -1000. * d[2]], d, 100., 2000.)?;
            only(cut(m, a, b)?)
        })(),
        "4b ruled loft minus a cylinder" => (|| { let l = ruled_loft(m, 0.)?; let lv = vol(m, l); let c = cyl(m, [0., 0., -10.], [0., 0., 1.], 100., 920.)?; Ok((cut(m, l, c)?, lv)) })(),
        "4c two lofts fused" => (|| { let a = ruled_loft(m, 0.)?; let av = vol(m, a); let b = ruled_loft(m, 300.)?; Ok((fuse(m, a, b)?, av)) })(),
        "4d revolved ring cut by an inclined plane" => (|| { let r = rev(m, &[(100., 0.), (300., 0.), (300., 500.), (100., 500.)])?; let c = inclined_cutter(m, 30f64.to_radians().tan())?; only(cut(m, r, c)?) })(),
        "4e cylinder cut by an inclined plane" => (|| { let r = cyl(m, [0., 0., 0.], [0., 0., 1.], 300., 500.)?; let c = inclined_cutter(m, 30f64.to_radians().tan())?; only(cut(m, r, c)?) })(),
        "5a fillet plane-plane" => (|| { let b = bx(m, [0., 0., 0.], [1000., 600., 750.])?; let ed = edge_where(m, b, |p| (p - Point3::new(1000., 600., 375.)).norm())?; only(ops::fillet(m, b, &[ed], 50., &C).map_err(e)?.0) })(),
        "5b fillet plane-cylinder" => (|| {
            let a = bx(m, [-500., -500., 0.], [500., 500., 200.])?; let c = cyl(m, [0., 0., 200.], [0., 0., 1.], 100., 400.)?;
            let b = fuse(m, a, c)?;
            let ed = edge_where(m, b, |p| ((p.x * p.x + p.y * p.y).sqrt() - 100.).abs() + (p.z - 200.).abs())?;
            only(ops::fillet(m, b, &[ed], 20., &C).map_err(e)?.0)
        })(),
        "5c fillet NURBS-plane" => (|| { let t0 = Instant::now(); let b = smooth_loft(m)?; let bv = vol(m, b); eprintln!("loft and its volume: {:.1} ms", t0.elapsed().as_secs_f64() * 1e3); let ed = edge_where(m, b, |p| ((p.x * p.x + p.y * p.y).sqrt() - 200.).abs() + (p.z - 900.).abs())?; Ok((ops::fillet(m, b, &[ed], 20., &C).map_err(e)?.0, bv)) })(),
        "5d fillet NURBS-NURBS" => (|| {
            let t0 = Instant::now(); let b = smooth_loft(m)?; let bv = vol(m, b); eprintln!("loft and its volume: {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
            // a side edge running up from a corner of the square: its middle is off both caps, in the first quadrant
            let ed = edge_where(m, b, |p| if p.z > 100. && p.z < 800. && p.x > 0. && p.y > 0. { (p.x - p.y).abs() } else { f64::INFINITY })?;
            if edge_mid(m, &ed).map(|p| p.z <= 100. || p.z >= 800.).unwrap_or(true) { return Err("no side edge between two loft faces (the loft has one smooth side face)".into()); }
            Ok((ops::fillet(m, b, &[ed], 20., &C).map_err(e)?.0, bv))
        })(),
        "5e shell a cylinder open at the top" => (|| { let b = cyl(m, [0., 0., 0.], [0., 0., 1.], 300., 500.)?; let top = face_at(m, b, Point3::new(0., 0., 500.))?; only(ops::shell(m, b, &[top], 10., ShellSide::Inward, &C).map_err(e)?.0) })(),
        "5f offset a cylindrical face" => (|| { let b = cyl(m, [0., 0., 0.], [0., 0., 1.], 300., 500.)?; let side = face_at(m, b, Point3::new(300. * 1f64.cos(), 300. * 1f64.sin(), 250.))?; only(ops::offset_faces(m, b, &[side], 10., &C).map_err(e)?.0) })(),
        "6a box with a NaN corner" => (|| only(bx(m, [0., f64::NAN, 0.], [1000., 600., 750.])?))(),
        "6b box of zero thickness" => (|| only(bx(m, [0., 0., 0.], [1000., 600., 0.])?))(),
        "6c box with inverted corners" => (|| only(bx(m, [1000., 600., 750.], [0., 0., 0.])?))(),
        "6d cylinder of radius 0" => (|| only(cyl(m, [0., 0., 0.], [0., 0., 1.], 0., 750.)?))(),
        "6e cylinder of radius NaN" => (|| only(cyl(m, [0., 0., 0.], [0., 0., 1.], f64::NAN, 750.)?))(),
        "6f cylinder of negative radius" => (|| only(cyl(m, [0., 0., 0.], [0., 0., 1.], -100., 750.)?))(),
        "6g unit box at 1e9 from the origin" => (|| only(bx(m, [1e9, 1e9, 1e9], [1e9 + 1., 1e9 + 1., 1e9 + 1.])?))(),
        "6h box fused with itself" => (|| { let a = bx(m, [0., 0., 0.], [1000., 600., 750.])?; let b = bx(m, [0., 0., 0.], [1000., 600., 750.])?; only(fuse(m, a, b)?) })(),
        "6i box cut by itself" => (|| { let a = bx(m, [0., 0., 0.], [1000., 600., 750.])?; let b = bx(m, [0., 0., 0.], [1000., 600., 750.])?; only(cut(m, a, b)?) })(),
        "6j extrude a NaN profile" => (|| only(ext(m, &poly(xy(0.), &[(0., 0.), (f64::NAN, 0.), (1., 1.)]), Vec3::z(), 750.)?))(),
        "6k extrude by zero" => (|| only(ext(m, &poly(xy(0.), &[(0., 0.), (10., 0.), (10., 10.)]), Vec3::z(), 0.)?))(),
        "6l extrude a self-intersecting profile" => (|| only(ext(m, &poly(xy(0.), &[(0., 0.), (10., 10.), (10., 0.), (0., 10.)]), Vec3::z(), 5.)?))(),
        _ => { let _ = pi; return None; }
    })
}

fn esc(s: &str) -> String { s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ") }
fn num(x: Option<f64>) -> String { match x { Some(v) if v.is_finite() => format!("{v:.17e}"), _ => "null".into() } }

fn main() {
    let name = std::env::args().nth(1).expect("case name");
    std::panic::set_hook(Box::new(|_| {}));
    let mut m = Model::default();
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| run(&name, &mut m)));
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let head = format!("{{\"case\":\"{}\",\"kernel\":\"arris 0.12.0\",\"ms\":{ms:.3}", esc(&name));
    match r {
        Err(p) => { let s = p.downcast_ref::<String>().cloned().or(p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default(); println!("{head},\"outcome\":\"panic\",\"err\":\"{}\"}}", esc(&s)); }
        Ok(None) => { eprintln!("unknown case"); std::process::exit(2); }
        Ok(Some(Err(x))) => println!("{head},\"outcome\":\"error\",\"err\":\"{}\"}}", esc(&x.chars().take(300).collect::<String>())),
        Ok(Some(Ok((b, rf)))) => {
            let faces = m.faces(b).map(|f| f.len()).unwrap_or(0);
            let edges = m.edges(b).map(|f| f.len()).unwrap_or(0);
            if faces == 0 { println!("{head},\"outcome\":\"empty\",\"faces\":0}}"); return; }
            let valid = catch_unwind(AssertUnwindSafe(|| arris::check::check(&m, b, arris::check::Level::Full).is_ok())).ok();
            let v = catch_unwind(AssertUnwindSafe(|| vol(&m, b))).ok().flatten();
            let mesh = match catch_unwind(AssertUnwindSafe(|| arris::mesh::tessellate(&m, b, 0.5, &C))) {
                Ok(Ok(me)) => format!("ok {} tris closed={}", me.triangles().len(), me.is_closed()),
                Ok(Err(x)) => format!("err {}", esc(&format!("{x}").chars().take(120).collect::<String>())),
                Err(_) => "err panic".into(),
            };
            let step = match catch_unwind(AssertUnwindSafe(|| arris::io::step::write(&m, &[b]))) { Ok(Ok(s)) => format!("ok {} bytes", s.len()), Ok(Err(x)) => format!("err {}", esc(&format!("{x}").chars().take(80).collect::<String>())), Err(_) => "err panic".into() };
            println!("{head},\"outcome\":\"solid\",\"volume\":{},\"vsrc\":\"exact\",\"ref\":{},\"faces\":{faces},\"edges\":{edges},\"valid\":{},\"mesh\":\"{mesh}\",\"step\":\"{step}\"}}",
                num(v), num(rf), valid.map(|x| x.to_string()).unwrap_or("null".into()));
        }
    }
}
