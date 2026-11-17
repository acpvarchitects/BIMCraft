use arris::Control;
use arris::geom::profile::{Profile, ProfileLoop, ProfileSegment};
use arris::geom::{FrameLaw, Path, PathSegment};
use arris::math::{Axis, Frame, Point2, Point3, Vec3};
use arris::ops::{self, LoftKind, OpError};
use arris::topo::{Body, Model, provenance::Provenance};
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

const C: Control<'static> = Control::NONE;

fn poly(plane: Frame, pts: &[(f64, f64)]) -> Profile {
    let mut segs: Vec<ProfileSegment> = pts[1..].iter().map(|p| ProfileSegment::LineTo(Point2::new(p.0, p.1))).collect();
    segs.push(ProfileSegment::LineTo(Point2::new(pts[0].0, pts[0].1)));
    Profile { plane, outer: ProfileLoop::Path { start: Point2::new(pts[0].0, pts[0].1), segments: segs }, holes: vec![] }
}
fn fr(o: Point3, n: Vec3, x: Vec3) -> Frame { Frame::new(o, n, x).unwrap() }
fn xy(z: f64) -> Frame { fr(Point3::new(0.0, 0.0, z), Vec3::z(), Vec3::x()) }
fn bx(m: &mut Model, a: [f64; 3], b: [f64; 3]) -> Body { ops::primitive_box(m, a, b, &C).unwrap().0 }

fn report(name: &str, m: &mut Model, f: impl FnOnce(&mut Model) -> Result<(Body, Provenance), OpError>) {
    if let Some(a) = std::env::args().nth(1) { if a != name { return; } }
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| f(m)));
    let dt = t.elapsed();
    let body = match r {
        Err(_) => { println!("| {name} | PANIC | | | | |"); return; }
        Ok(Err(e)) => { let s = format!("{e}"); println!("| {name} | ERR: {} | | | {:.2} ms | |", s.chars().take(200).collect::<String>(), dt.as_secs_f64()*1e3); return; }
        Ok(Ok((b, _))) => b,
    };
    let mut fk: BTreeMap<String, usize> = BTreeMap::new();
    for f in m.faces(body).unwrap() {
        let s = m.face(f.id).unwrap().surface();
        *fk.entry(format!("{:?}", m.surface(s).unwrap().kind())).or_default() += 1;
    }
    let mut ek: BTreeMap<String, usize> = BTreeMap::new();
    for e in m.edges(body).unwrap() {
        let k = match m.edge(e.id).unwrap().curve() { Some((c, _)) => format!("{:?}", m.curve(c).unwrap().kind()), None => "Degenerate".into() };
        *ek.entry(k).or_default() += 1;
    }
    let chk = if arris::check::check(m, body, arris::check::Level::Full).is_ok() { "valid" } else { "INVALID" };
    let mut ms = String::new();
    for chord in [2.0, 0.5] {
        let t2 = Instant::now();
        let mesh = catch_unwind(AssertUnwindSafe(|| arris::mesh::tessellate(m, body, chord, &C)));
        let dt2 = t2.elapsed();
        ms += &match mesh { Ok(Ok(me)) => format!("@{chord}mm {} tris, {:.2} ms, closed={}; ", me.triangles().len(), dt2.as_secs_f64()*1e3, me.is_closed()), Ok(Err(e)) => format!("@{chord}mm mesh ERR {e}; "), Err(_) => format!("@{chord}mm mesh PANIC; ") };
    }
    println!("| {name} | ok ({chk}) | {} faces {:?} | {} edges {:?} | {:.2} ms | {ms} |", fk.values().sum::<usize>(), fk, ek.values().sum::<usize>(), ek, dt.as_secs_f64()*1e3);
}

fn main() {
    let mut m = Model::default();
    let step_only = std::env::args().nth(1).as_deref() == Some("STEP");
    report("1a extrude rect", &mut m, |m| ops::extrude(m, &poly(xy(0.0), &[(0.,0.),(1000.,0.),(1000.,600.),(0.,600.)]), Vec3::z(), 750.0, &C));
    report("1b extrude L", &mut m, |m| ops::extrude(m, &poly(xy(0.0), &[(0.,0.),(1000.,0.),(1000.,200.),(200.,200.),(200.,600.),(0.,600.)]), Vec3::z(), 750.0, &C));
    let xz = fr(Point3::origin(), -Vec3::y(), Vec3::x());
    report("2 revolve ring", &mut m, |m| ops::revolve(m, &poly(xz, &[(100.,0.),(300.,0.),(300.,500.),(100.,500.)]), Axis::z_at(Point3::origin()), std::f64::consts::TAU, &C));
    let sq = |o: Point3, n: Vec3, x: Vec3, h: f64| poly(fr(o, n, x), &[(h,h),(-h,h),(-h,-h),(h,-h)]);
    report("3a sweep L path (corner)", &mut m, |m| ops::sweep(m, &sq(Point3::origin(), Vec3::x(), Vec3::y(), 100.), &Path { start: Point3::origin(), segments: vec![PathSegment::LineTo(Point3::new(1000.,0.,0.)), PathSegment::LineTo(Point3::new(1000.,800.,0.))] }, FrameLaw::default(), &C));
    report("3a2 L as fuse of 2 boxes", &mut m, |m| { let a = bx(m, [0.,-100.,-100.],[1100.,100.,100.]); let b = bx(m, [900.,-100.,-100.],[1100.,800.,100.]); ops::fuse(m, a, b, &C) });
    let s45 = 1000.0 * std::f64::consts::FRAC_1_SQRT_2;
    let arc = Path { start: Point3::origin(), segments: vec![PathSegment::ArcTo { via: Point3::new(s45, 1000.-s45, 0.), to: Point3::new(1000.,1000.,0.) }] };
    report("3b sweep arc R1000", &mut m, |m| ops::sweep(m, &sq(Point3::origin(), Vec3::x(), Vec3::y(), 100.), &arc, FrameLaw::default(), &C));
    report("3c sweep straight line", &mut m, |m| ops::sweep(m, &sq(Point3::origin(), Vec3::x(), Vec3::y(), 100.), &Path { start: Point3::origin(), segments: vec![PathSegment::LineTo(Point3::new(1000.,0.,0.))] }, FrameLaw::default(), &C));
    let circ = |z: f64, r: f64| Profile { plane: xy(z), outer: ProfileLoop::Circle { center: Point2::origin(), radius: r }, holes: vec![] };
    report("4 loft square->circle", &mut m, |m| ops::loft(m, &[sq(Point3::origin(), Vec3::z(), Vec3::x(), 500.), circ(900., 200.)], LoftKind::Ruled, false, &C));
    report("4b loft smooth 3 sect", &mut m, |m| ops::loft(m, &[sq(Point3::origin(), Vec3::z(), Vec3::x(), 500.), circ(450., 300.), circ(900., 200.)], LoftKind::Smooth, false, &C));
    let on_arc = |deg: f64| { let a = f64::to_radians(deg); (Point3::new(1000.*a.sin(), 1000.-1000.*a.cos(), 0.), Vec3::new(a.cos(), a.sin(), 0.)) };
    report("5b blend along arc (smooth loft 3 sect)", &mut m, |m| {
        let (p0, t0) = on_arc(0.); let (p1, t1) = on_arc(45.); let (p2, t2) = on_arc(90.);
        let c = |p: Point3, t: Vec3, r: f64| Profile { plane: fr(p, t, Vec3::z()), outer: ProfileLoop::Circle { center: Point2::origin(), radius: r }, holes: vec![] };
        ops::loft(m, &[sq(p0, t0, Vec3::z(), 100.), c(p1, t1, 80.), c(p2, t2, 60.)], LoftKind::Smooth, false, &C) });
    report("6a box - cylinder", &mut m, |m| { let a = bx(m, [0.,0.,0.],[1000.,600.,750.]); let c = ops::primitive_cylinder(m, Axis::z_at(Point3::new(500.,300.,-100.)), 100., 950., &C)?.0; ops::cut(m, a, c, &C) });
    report("6b box - coplanar box", &mut m, |m| { let a = bx(m, [0.,0.,0.],[1000.,600.,750.]); let b = bx(m, [200.,0.,0.],[800.,300.,750.]); ops::cut(m, a, b, &C) });
    report("6c fuse boxes sharing face", &mut m, |m| { let a = bx(m, [0.,0.,0.],[1000.,600.,750.]); let b = bx(m, [1000.,0.,0.],[1500.,600.,750.]); ops::fuse(m, a, b, &C) });
    report("6d fuse partly shared face", &mut m, |m| { let a = bx(m, [0.,0.,0.],[1000.,600.,750.]); let b = bx(m, [1000.,100.,0.],[1500.,400.,500.]); ops::fuse(m, a, b, &C) });
    report("6e ring(revolve) - box", &mut m, |m| { let r = ops::revolve(m, &poly(xz, &[(100.,0.),(300.,0.),(300.,500.),(100.,500.)]), Axis::z_at(Point3::origin()), std::f64::consts::TAU, &C)?.0; let b = bx(m, [0.,-400.,250.],[400.,400.,600.]); ops::cut(m, r, b, &C) });
    report("hostile: NaN extrude", &mut m, |m| ops::extrude(m, &poly(xy(0.0), &[(0.,0.),(f64::NAN,0.),(1.,1.)]), Vec3::z(), 750.0, &C));
    report("hostile: zero-length extrude", &mut m, |m| ops::extrude(m, &poly(xy(0.0), &[(0.,0.),(10.,0.),(10.,10.)]), Vec3::z(), 0.0, &C));
    report("hostile: self-intersecting", &mut m, |m| ops::extrude(m, &poly(xy(0.0), &[(0.,0.),(10.,10.),(10.,0.),(0.,10.)]), Vec3::z(), 5.0, &C));
    if std::env::args().nth(1).is_some() && !step_only { return; }
    let b = bx(&mut m, [0.,0.,0.],[1.,1.,1.]);
    let c = ops::primitive_cylinder(&mut m, Axis::z_at(Point3::new(0.5,0.5,-1.)), 0.2, 3., &C).unwrap().0;
    let (r, _) = ops::cut(&mut m, b, c, &C).unwrap();
    match arris::io::step::write(&m, &[r]) {
        Ok(s) => println!("STEP: {} bytes, CIRCLE={} B_SPLINE={} CYLINDRICAL_SURFACE={} {}", s.len(), s.matches("CIRCLE(").count(), s.matches("B_SPLINE").count(), s.matches("CYLINDRICAL_SURFACE").count(), s.lines().find(|l| l.contains("FILE_SCHEMA")).unwrap_or("")),
        Err(e) => println!("STEP err {e}"),
    }
}
