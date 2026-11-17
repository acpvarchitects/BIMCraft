//! What arris 0.11.1 does with free-form geometry.
use arris::Control;
use arris::geom::profile::{Profile, ProfileLoop, ProfileSegment};
use arris::geom::{FrameLaw, NurbsCurve, NurbsSurface, Path, PathSegment, SurfaceKind, fit_curve_periodic, interpolate_skin};
use arris::math::{Axis, Frame, Interval, Meter, Point2, Point3, Vec3};
use arris::ops::{self, LoftKind, ShellSide};
use arris::topo::{Body, Model};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

const C: Control<'static> = Control::NONE;
type R = Result<String, String>;

fn rep(name: &str, f: impl FnOnce() -> R) {
    if let Some(f) = std::env::args().nth(1) { if !name.starts_with(f.as_str()) { return; } }
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(f));
    let dt = t.elapsed().as_secs_f64() * 1e3;
    match r {
        Err(_) => println!("| {name} | PANIC | {dt:.2} ms |"),
        Ok(Ok(s)) => println!("| {name} | ok: {s} | {dt:.2} ms |"),
        Ok(Err(e)) => println!("| {name} | ERR: {} | {dt:.2} ms |", e.chars().take(220).collect::<String>()),
    }
}
fn s<E: std::fmt::Display>(e: E) -> String { e.to_string() }
fn fr(o: Point3, n: Vec3, x: Vec3) -> Frame { Frame::new(o, n, x).unwrap() }
fn poly(plane: Frame, pts: &[(f64, f64)]) -> Profile {
    let mut segs: Vec<ProfileSegment> = pts[1..].iter().map(|p| ProfileSegment::LineTo(Point2::new(p.0, p.1))).collect();
    segs.push(ProfileSegment::LineTo(Point2::new(pts[0].0, pts[0].1)));
    Profile { plane, outer: ProfileLoop::Path { start: Point2::new(pts[0].0, pts[0].1), segments: segs }, holes: vec![] }
}
fn sq(o: Point3, h: f64) -> Profile { poly(fr(o, Vec3::z(), Vec3::x()), &[(h, h), (-h, h), (-h, -h), (h, -h)]) }
fn circ(o: Point3, r: f64) -> Profile { Profile { plane: fr(o, Vec3::z(), Vec3::x()), outer: ProfileLoop::Circle { center: Point2::origin(), radius: r }, holes: vec![] } }
fn kinds(m: &Model, b: Body) -> String {
    let mut n = 0; let mut nn = 0;
    for f in m.faces(b).unwrap() { n += 1; if m.surface(m.face(f.id).unwrap().surface()).unwrap().kind() == SurfaceKind::Nurbs { nn += 1; } }
    let valid = arris::check::check(m, b, arris::check::Level::Fast).is_ok();
    format!("{n} faces ({nn} NURBS), valid={valid}")
}
fn clamped(n: usize, p: usize) -> Vec<f64> {
    let mut k = vec![0.0; p + 1];
    for i in 1..(n - p) { k.push(i as f64); }
    k.extend(vec![(n - p) as f64; p + 1]);
    k
}
// a smooth (NURBS) loft: square 1000 at z0, circle r300 at 450, square 600 rotated at 900
fn smooth_loft(m: &mut Model, at: Vec3) -> Result<Body, String> {
    let o = Point3::origin() + at;
    ops::loft(m, &[sq(o, 500.), circ(o + Vec3::new(0., 0., 450.), 350.), sq(o + Vec3::new(0., 0., 900.), 300.)], LoftKind::Smooth, false, &C).map(|x| x.0).map_err(s)
}

fn main() {
    println!("| test | result | time |\n|---|---|---|");
    // 1 curves
    let cps: Vec<Point3> = (0..6).map(|i| Point3::new(i as f64 * 200., if i % 2 == 0 { 0. } else { 300. }, 0.)).collect();
    let curve = NurbsCurve::new(3, clamped(6, 3), cps.clone(), vec![1.0; 6]);
    rep("1a curve by 6 control points, degree 3", || { let c = curve.clone().map_err(s)?; let p = c.eval(1.5).point; Ok(format!("eval(1.5)=({:.1},{:.1})", p.x, p.y)) });
    rep("1b split (segment + insert_knot)", || { let c = curve.clone().map_err(s)?; let a = c.segment(Interval::new(0.0, 1.2).map_err(|e| format!("{e:?}"))?).map_err(s)?; let k = c.insert_knot(1.2, 3).map_err(s)?; Ok(format!("piece {} cps, refined {} cps", a.control_points().len(), k.control_points().len())) });
    rep("1c closed periodic curve (fit to a closed function)", || { let f = |t: f64| Point3::new(500. * t.cos(), 300. * t.sin() + 50. * (3. * t).sin(), 0.); let c = fit_curve_periodic(f, Interval::TURN, 3, |t, q| (q - f(t)).norm(), 0.01, &mut Meter::default()).map_err(|e| format!("{e:?}"))?; Ok(format!("period {:?}, {} cps", c.period(), c.control_points().len())) });
    rep("1d curve through points (interpolation)", || Err("missing: no API; fit_curve fits a closure f(t), not a point list".into()));
    rep("1e offset curve in plane", || Err("missing: no curve offset in arris-geom".into()));
    // 2 surfaces
    let grid = |bump: f64| -> Vec<Point3> { (0..6).flat_map(|i| (0..6).map(move |j| Point3::new(i as f64 * 200., j as f64 * 200., if i == 2 && j == 3 { bump } else { ((i + j) % 3) as f64 * 40. }))).collect() };
    rep("2a surface 6x6 control points degree 3", || { let t = Instant::now(); let mut n = 0; for _ in 0..1000 { let srf = NurbsSurface::new([3, 3], [clamped(6, 3), clamped(6, 3)], grid(0.), vec![1.0; 36]).map_err(s)?; n += srf.counts()[0]; } Ok(format!("{:.4} ms each (x1000), {}", t.elapsed().as_secs_f64(), n / 1000)) });
    rep("2b edit a control point and rebuild", || { let a = NurbsSurface::new([3, 3], [clamped(6, 3), clamped(6, 3)], grid(0.), vec![1.0; 36]).map_err(s)?; let mut pts = a.control_points().to_vec(); pts[2 * 6 + 3].z += 250.; let b = NurbsSurface::new([3, 3], [clamped(6, 3), clamped(6, 3)], pts, vec![1.0; 36]).map_err(s)?; Ok(format!("z at (2.5,2.5): {:.1} -> {:.1}", a.eval(1.5, 1.5).point.z, b.eval(1.5, 1.5).point.z)) });
    rep("2c surface through a grid (interpolate_skin over row curves)", || {
        let rows: Result<Vec<NurbsCurve>, _> = (0..6).map(|i| NurbsCurve::new(3, clamped(6, 3), (0..6).map(|j| Point3::new(i as f64 * 200., j as f64 * 200., ((i * j) % 4) as f64 * 30.)).collect(), vec![1.0; 6])).collect();
        let rows = rows.map_err(s)?; let params: Vec<f64> = (0..6).map(|i| i as f64 / 5.).collect();
        let srf = interpolate_skin(&rows, &params, 3, false).map_err(|e| format!("{e:?}"))?; Ok(format!("{:?} cps; rows are control polygons, not fitted points", srf.counts())) });
    rep("2d offset NURBS surface 20 mm (offset_fitted)", || { let a = NurbsSurface::new([3, 3], [clamped(6, 3), clamped(6, 3)], grid(0.), vec![1.0; 36]).map_err(s)?; let o = a.offset_fitted(20.0, a.domain(), 0.01, &mut Meter::default()).map_err(s)?; Ok(format!("{:?} cps", o.counts())) });
    rep("2e sheet body from a bare NURBS surface", || Err("missing as an op: only the low-level Builder (you stage edges, pcurves, face)".into()));
    // 3 constructions
    rep("3a loft open, ruled", || { let mut m = Model::default(); let b = ops::loft(&mut m, &[sq(Point3::origin(), 500.), circ(Point3::new(0., 0., 900.), 200.)], LoftKind::Ruled, false, &C).map_err(s)?.0; Ok(kinds(&m, b)) });
    rep("3b loft open, smooth 3 sections", || { let mut m = Model::default(); let b = smooth_loft(&mut m, Vec3::zeros())?; Ok(kinds(&m, b)) });
    rep("3c loft closed (4 circles round a ring)", || { let mut m = Model::default();
        let secs: Vec<Profile> = (0..4).map(|k| { let a = k as f64 * std::f64::consts::FRAC_PI_2; let c = Point3::new(1000. * a.cos(), 1000. * a.sin(), 0.); Profile { plane: fr(c, Vec3::new(-a.sin(), a.cos(), 0.), Vec3::z()), outer: ProfileLoop::Circle { center: Point2::origin(), radius: 100. + 50. * (k % 2) as f64 }, holes: vec![] } }).collect();
        let b = ops::loft(&mut m, &secs, LoftKind::Smooth, true, &C).map_err(s)?.0; Ok(kinds(&m, b)) });
    rep("3d loft with rails / sweep2 / network patch / blend surface", || Err("missing".into()));
    rep("3e sweep1 along a NURBS rail", || { let mut m = Model::default();
        let rail = NurbsCurve::new(3, clamped(5, 3), vec![Point3::origin(), Point3::new(500., 0., 0.), Point3::new(1000., 500., 200.), Point3::new(1500., 500., 600.), Point3::new(2000., 0., 600.)], vec![1.0; 5]).map_err(s)?;
        let prof = poly(fr(Point3::origin(), Vec3::x(), Vec3::y()), &[(100., 50.), (-100., 50.), (-100., -50.), (100., -50.)]);
        let b = ops::sweep(&mut m, &prof, &Path { start: Point3::origin(), segments: vec![PathSegment::Nurbs(rail)] }, FrameLaw::default(), &C).map_err(s)?.0; Ok(kinds(&m, b)) });
    rep("3f revolve / extrude / planar-cap a NURBS curve", || Err("missing: ProfileSegment has LineTo, ArcTo, EllipseTo only; NURBS goes only in paths".into()));
    // 4 solids
    rep("4a loft capped into a solid", || { let mut m = Model::default(); let b = smooth_loft(&mut m, Vec3::zeros())?; Ok(format!("loft is born a capped solid: {}", kinds(&m, b))) });
    rep("4b shell a smooth loft 10 mm, top open", || { let mut m = Model::default(); let b = smooth_loft(&mut m, Vec3::zeros())?; let top = m.faces(b).unwrap()[1]; let r = ops::shell(&mut m, b, &[top], 10.0, ShellSide::Inward, &C).map_err(s)?.0; Ok(kinds(&m, r)) });
    rep("4c offset a NURBS face of a solid (offset_faces)", || { let mut m = Model::default(); let b = smooth_loft(&mut m, Vec3::zeros())?; let side = m.faces(b).unwrap()[2]; let r = ops::offset_faces(&mut m, b, &[side], 10.0, &C).map_err(s)?.0; Ok(kinds(&m, r)) });
    // 5 booleans
    rep("5a smooth loft - box (box through a NURBS side)", || { let mut m = Model::default(); let l = smooth_loft(&mut m, Vec3::zeros())?; let bx = ops::primitive_box(&mut m, [200., -100., 300.], [800., 100., 600.], &C).map_err(s)?.0; let t = Instant::now(); let r = ops::cut(&mut m, l, bx, &C).map_err(s)?.0; Ok(format!("{} in {:.1} ms", kinds(&m, r), t.elapsed().as_secs_f64() * 1e3)) });
    rep("5b smooth loft - cylinder through it (horizontal)", || { let mut m = Model::default(); let l = smooth_loft(&mut m, Vec3::zeros())?; let c = ops::primitive_cylinder(&mut m, Axis::new(Point3::new(-800., 0., 450.), Vec3::x()).map_err(|e| format!("{e:?}"))?, 100., 1600., &C).map_err(s)?.0; let t = Instant::now(); let r = ops::cut(&mut m, l, c, &C).map_err(s)?.0; Ok(format!("{} in {:.1} ms", kinds(&m, r), t.elapsed().as_secs_f64() * 1e3)) });
    rep("5c two smooth lofts fused (overlapping)", || { let mut m = Model::default(); let a = smooth_loft(&mut m, Vec3::zeros())?; let b = smooth_loft(&mut m, Vec3::new(400., 150., 100.))?; let t = Instant::now(); let r = ops::fuse(&mut m, a, b, &C).map_err(s)?.0;
        let mut ek = std::collections::BTreeMap::<String, usize>::new(); for e in m.edges(r).unwrap() { if let Some((c, _)) = m.edge(e.id).unwrap().curve() { *ek.entry(format!("{:?}", m.curve(c).unwrap().kind())).or_default() += 1; } }
        Ok(format!("{} in {:.1} ms, edges {ek:?}", kinds(&m, r), t.elapsed().as_secs_f64() * 1e3)) });
    let panel = |m: &mut Model| -> Result<Body, String> {
        // a thin curved panel: three 1000x30 rectangles on a bent line, smooth-lofted along x
        let sec = |x: f64, y: f64| poly(fr(Point3::new(x, y, 0.), Vec3::x(), Vec3::y()), &[(15., 0.), (15., 1000.), (-15., 1000.), (-15., 0.)]);
        ops::loft(m, &[sec(0., 0.), sec(600., 200.), sec(1200., 0.)], LoftKind::Smooth, false, &C).map(|x| x.0).map_err(s)
    };
    rep("5d box - thin NURBS panel", || { let mut m = Model::default(); let p = panel(&mut m)?; let bx = ops::primitive_box(&mut m, [100., -300., 200.], [1100., 400., 800.], &C).map_err(s)?.0; let t = Instant::now(); let r = ops::cut(&mut m, bx, p, &C).map_err(s)?.0; Ok(format!("{} in {:.1} ms", kinds(&m, r), t.elapsed().as_secs_f64() * 1e3)) });
    rep("5e NURBS face flush with a box face (loft cap on box top)", || { let mut m = Model::default(); let l = smooth_loft(&mut m, Vec3::zeros())?; let bx = ops::primitive_box(&mut m, [-600., -600., -300.], [600., 600., 0.], &C).map_err(s)?.0; let r = ops::fuse(&mut m, l, bx, &C).map_err(s)?.0; Ok(kinds(&m, r)) });
    // 6 fillets
    rep("6a fillet plane-plane (box edge, r 20)", || { let mut m = Model::default(); let b = ops::primitive_box(&mut m, [0.; 3], [1000., 600., 750.], &C).map_err(s)?.0; let e = m.edges(b).unwrap()[0]; let r = ops::fillet(&mut m, b, &[e], 20.0, &C).map_err(s)?.0; Ok(kinds(&m, r)) });
    rep("6b fillet plane-cylinder (hole rim, r 5)", || { let mut m = Model::default(); let b = ops::primitive_box(&mut m, [0.; 3], [1000., 600., 750.], &C).map_err(s)?.0; let c = ops::primitive_cylinder(&mut m, Axis::z_at(Point3::new(500., 300., -10.)), 100., 800., &C).map_err(s)?.0; let h = ops::cut(&mut m, b, c, &C).map_err(s)?.0;
        let rim: Vec<_> = m.edges(h).unwrap().into_iter().filter(|e| m.edge(e.id).unwrap().curve().map(|(c, _)| format!("{:?}", m.curve(c).unwrap().kind()) == "Circle").unwrap_or(false)).take(1).collect();
        let r = ops::fillet(&mut m, h, &rim, 5.0, &C).map_err(s)?.0; Ok(kinds(&m, r)) });
    rep("6c fillet NURBS-NURBS edge (loft rail, r 20)", || { let mut m = Model::default(); let b = smooth_loft(&mut m, Vec3::zeros())?; let e = *m.edges(b).unwrap().last().unwrap(); let r = ops::fillet(&mut m, b, &[e], 20.0, &C).map_err(s)?.0; Ok(kinds(&m, r)) });
    rep("6d fillet NURBS-plane edge (loft top rim, r 10)", || { let mut m = Model::default(); let b = smooth_loft(&mut m, Vec3::zeros())?; let e = m.edges(b).unwrap()[5]; let r = ops::fillet(&mut m, b, &[e], 10.0, &C).map_err(s)?.0; Ok(kinds(&m, r)) });
    rep("6e chamfer NURBS-NURBS edge (5 mm)", || { let mut m = Model::default(); let b = smooth_loft(&mut m, Vec3::zeros())?; let e = *m.edges(b).unwrap().last().unwrap(); let r = ops::chamfer(&mut m, b, &[e], 5.0, &C).map_err(s)?.0; Ok(kinds(&m, r)) });
    rep("6f variable-radius fillet", || Err("missing: fillet takes one radius".into()));
    // 7 trim/split
    rep("7a split smooth loft by a plane", || { let mut m = Model::default(); let b = smooth_loft(&mut m, Vec3::zeros())?; let sp = ops::split(&mut m, b, &fr(Point3::new(0., 0., 300.), Vec3::new(0.2, 0., 1.), Vec3::x()), &C).map_err(s)?; Ok(format!("+ {} / - {}", kinds(&m, sp.positive), kinds(&m, sp.negative))) });
    rep("7b trim a surface by a curve / by another surface", || Err("missing as ops (split takes a plane); geometry has intersect_surfaces + project; by a solid: cut/common".into()));
    // 8 tessellation
    for chord in [0.5, 2.0, 5.0] {
        rep(&format!("8 tessellate smooth loft at {chord} mm"), || { let mut m = Model::default(); let b = smooth_loft(&mut m, Vec3::zeros())?; let t = Instant::now(); let me = arris::mesh::tessellate(&m, b, chord, &C).map_err(s)?; Ok(format!("{} tris in {:.1} ms", me.triangles().len(), t.elapsed().as_secs_f64() * 1e3)) });
    }
    // 9 STEP
    rep("9 STEP of a smooth loft", || { let mut m = Model::default(); let b = smooth_loft(&mut m, Vec3::zeros())?; let st = arris::io::step::write(&m, &[b]).map_err(s)?;
        Ok(format!("{} bytes; B_SPLINE_SURFACE_WITH_KNOTS={} RATIONAL_B_SPLINE_SURFACE={} B_SPLINE_CURVE_WITH_KNOTS={}", st.len(), st.matches("B_SPLINE_SURFACE_WITH_KNOTS").count(), st.matches("RATIONAL_B_SPLINE_SURFACE").count(), st.matches("B_SPLINE_CURVE_WITH_KNOTS").count())) });
}
