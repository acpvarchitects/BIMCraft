//! Facade-panel throughput spike for arris: every panel a distinct definition.
//! usage: arris-perf <a|b> <n> <threads> [full]
//!   a = extrude L + revolve + loft (no boolean); b = a + one void cylinder cut through the L slab
use arris::Control;
use arris::geom::Curve;
use arris::geom::profile::{Profile, ProfileLoop, ProfileSegment};
use arris::math::{Axis, Frame, Point2, Point3, Vec3};
use arris::ops::{self, LoftKind};
use arris::topo::{Body, Model};
use rayon::prelude::*;
use std::time::{Duration, Instant};

const C: Control<'static> = Control::NONE;

fn poly(plane: Frame, pts: &[(f64, f64)]) -> Profile {
    let mut segs: Vec<ProfileSegment> = pts[1..].iter().map(|p| ProfileSegment::LineTo(Point2::new(p.0, p.1))).collect();
    segs.push(ProfileSegment::LineTo(Point2::new(pts[0].0, pts[0].1)));
    Profile { plane, outer: ProfileLoop::Path { start: Point2::new(pts[0].0, pts[0].1), segments: segs }, holes: vec![] }
}
fn fr(o: Point3, n: Vec3, x: Vec3) -> Frame { Frame::new(o, n, x).unwrap() }

#[derive(Clone, Copy, PartialEq, Debug)]
struct Params { w: f64, d: f64, t: f64, h: f64, r_in: f64, r_out: f64, ring_h: f64, loft_a: f64, loft_r: f64, hole_r: f64, void: bool }

fn params(i: usize, drive: f64, void: bool) -> Params {
    let f = (i % 997) as f64;
    Params { w: 1200.0 + f * 0.37 + drive, d: 600.0 + (i % 13) as f64 * 3.0, t: 200.0 + (i % 7) as f64, h: 50.0 + (i % 11) as f64,
        r_in: 40.0 + (i % 5) as f64, r_out: 80.0 + (i % 9) as f64, ring_h: 150.0 + (i % 17) as f64,
        loft_a: 300.0 + (i % 19) as f64, loft_r: 60.0 + (i % 23) as f64, hole_r: 30.0 + (i % 29) as f64 * 0.5, void }
}

#[derive(Default, Clone, Copy)]
struct Times { extrude: Duration, revolve: Duration, loft: Duration, boolean: Duration }
impl std::ops::AddAssign for Times { fn add_assign(&mut self, o: Self) { self.extrude += o.extrude; self.revolve += o.revolve; self.loft += o.loft; self.boolean += o.boolean; } }

struct Def { m: Model, bodies: Vec<Body> }

fn build(p: &Params) -> Result<(Def, Times), String> {
    let mut m = Model::default();
    let mut t = Times::default();
    let e = |x: ops::OpError| x.to_string();
    let xy = fr(Point3::origin(), Vec3::z(), Vec3::x());
    let s = Instant::now();
    // the panel: an L-shaped slab of thickness h (plan L, extruded up h)
    let (slab, _) = ops::extrude(&mut m, &poly(xy, &[(0., 0.), (p.w, 0.), (p.w, p.t), (p.t, p.t), (p.t, p.d), (0., p.d)]), Vec3::z(), p.h, &C).map_err(e)?;
    t.extrude = s.elapsed();
    let s = Instant::now();
    let xz = fr(Point3::new(p.w * 0.25, p.t * 0.5, p.h), -Vec3::y(), Vec3::x());
    let (ring, _) = ops::revolve(&mut m, &poly(xz, &[(p.r_in, 0.), (p.r_out, 0.), (p.r_out, p.ring_h), (p.r_in, p.ring_h)]), Axis::new(Point3::new(p.w * 0.25, p.t * 0.5, p.h), Vec3::z()).map_err(|x| format!("{x:?}"))?, std::f64::consts::TAU, &C).map_err(e)?;
    t.revolve = s.elapsed();
    let s = Instant::now();
    let a = p.loft_a * 0.5;
    let base = Point3::new(p.w * 0.75, p.t * 0.5, p.h);
    let sq = poly(fr(base, Vec3::z(), Vec3::x()), &[(a, a), (-a, a), (-a, -a), (a, -a)]);
    let ci = Profile { plane: fr(base + Vec3::new(0., 0., 400.), Vec3::z(), Vec3::x()), outer: ProfileLoop::Circle { center: Point2::origin(), radius: p.loft_r }, holes: vec![] };
    let (loft, _) = ops::loft(&mut m, &[sq, ci], LoftKind::Ruled, false, &C).map_err(e)?;
    t.loft = s.elapsed();
    let mut slab = slab;
    if p.void {
        let s = Instant::now();
        let (cyl, _) = ops::primitive_cylinder(&mut m, Axis::z_at(Point3::new(p.w * 0.5, p.t * 0.5, -10.0)), p.hole_r, p.h + 20.0, &C).map_err(e)?;
        slab = ops::cut(&mut m, slab, cyl, &C).map_err(e)?.0;
        t.boolean = s.elapsed();
    }
    Ok((Def { m, bodies: vec![slab, ring, loft] }, t))
}

#[derive(Debug)]
#[allow(dead_code)]
enum Prim2 { Seg([f64; 2], [f64; 2]), Arc { c: [f64; 2], r: f64, a0: f64, a1: f64 }, Conic(usize), Nurbs { pts: Vec<[f64; 2]>, w: Vec<f64> } }

fn plan_edges(d: &Def) -> Vec<Prim2> {
    let mut out = Vec::new();
    for &b in &d.bodies {
        let Ok(edges) = d.m.edges(b) else { continue };
        for e in edges {
            let Ok(ed) = d.m.edge(e.id) else { continue };
            let Some((cid, range)) = ed.curve() else { continue };
            let Ok(c) = d.m.curve(cid) else { continue };
            let p2 = |p: Point3| [p.x, p.y];
            match c {
                Curve::Line { .. } => {
                    let (Ok(a), Ok(bb)) = (d.m.vertex(ed.start()), d.m.vertex(ed.end())) else { continue };
                    let (a, bb) = (p2(a.point()), p2(bb.point()));
                    if (a[0] - bb[0]).abs() + (a[1] - bb[1]).abs() > 1e-9 { out.push(Prim2::Seg(a, bb)); } // vertical lines collapse to points
                }
                Curve::Circle { frame, radius } => {
                    let z = frame.vec_to_world(Vec3::z());
                    if z.z.abs() > 1.0 - 1e-12 { out.push(Prim2::Arc { c: p2(frame.to_world(Point3::origin())), r: *radius, a0: range.lo(), a1: range.hi() }); } else { out.push(Prim2::Conic(1)); }
                }
                Curve::Nurbs(n) => out.push(Prim2::Nurbs { pts: n.control_points().iter().map(|p| p2(*p)).collect(), w: n.weights().to_vec() }),
                _ => out.push(Prim2::Conic(2)),
            }
        }
    }
    out
}

fn tess(d: &Def, chord: f64) -> (usize, usize) {
    let (mut tris, mut verts) = (0, 0);
    for &b in &d.bodies {
        if let Ok(me) = arris::mesh::tessellate(&d.m, b, chord, &C) { tris += me.triangles().len(); verts += me.positions().len(); }
    }
    (tris, verts)
}

fn assert_send_sync<T: Send + Sync>() {}

fn ms(d: Duration) -> f64 { d.as_secs_f64() * 1e3 }

fn main() {
    assert_send_sync::<Model>();
    assert_send_sync::<Body>();
    let args: Vec<String> = std::env::args().collect();
    let void = args.get(1).map(|s| s == "b").unwrap_or(false);
    let n: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let threads: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let full = args.get(4).is_some();
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    pool.install(|| {
        let ps: Vec<Params> = (0..n).map(|i| params(i, 0.0, void)).collect();
        let s = Instant::now();
        let res: Vec<Result<(Def, Times), String>> = ps.par_iter().map(build).collect();
        let wall = s.elapsed();
        let mut t = Times::default();
        let mut errs = 0;
        let mut defs = Vec::with_capacity(n);
        for r in res { match r { Ok((d, tt)) => { t += tt; defs.push(d); } Err(x) => { if errs == 0 { eprintln!("err: {x}"); } errs += 1; } } }
        let cpu = t.extrude + t.revolve + t.loft + t.boolean;
        println!("kind={} n={n} threads={threads} errors={errs}", if void { "b(void cut)" } else { "a(no boolean)" });
        println!("build wall {:.0} ms ({:.3} ms/def wall); op cpu sum {:.0} ms: extrude {:.0}, revolve {:.0}, loft {:.0}, boolean {:.0} ({:.0}%)",
            ms(wall), ms(wall) / n as f64, ms(cpu), ms(t.extrude), ms(t.revolve), ms(t.loft), ms(t.boolean), 100.0 * ms(t.boolean) / ms(cpu).max(1e-9));
        if !full { return; }
        // full regen: the driving parameter changes every panel
        let ps2: Vec<Params> = (0..n).map(|i| params(i, 5.0, void)).collect();
        let s = Instant::now();
        let regen: Vec<_> = ps2.par_iter().map(build).collect();
        println!("full regen wall {:.0} ms", ms(s.elapsed()));
        drop(regen);
        // incremental: 10 changed panels, found by comparing recipes (the cache key)
        let mut ps3 = ps.clone();
        for k in 0..10 { ps3[k * (n / 10)].w += 1.0; }
        let s = Instant::now();
        let changed: Vec<usize> = (0..n).filter(|&i| ps3[i] != ps[i]).collect();
        let rebuilt: Vec<_> = changed.par_iter().map(|&i| build(&ps3[i])).collect();
        println!("incremental: {} changed, diff+rebuild {:.2} ms", rebuilt.len(), ms(s.elapsed()));
        for chord in [2.0, 0.5] {
            let s = Instant::now();
            let tv: Vec<(usize, usize)> = defs.par_iter().map(|d| tess(d, chord)).collect();
            let wall = s.elapsed();
            let tris: usize = tv.iter().map(|x| x.0).sum();
            let verts: usize = tv.iter().map(|x| x.1).sum();
            // GPU: f32 position+normal (24 B) per vertex, u32 index triple (12 B) per triangle; flat-shaded split ~ x2 verts
            let gpu = (verts as f64 * 2.0 * 24.0 + tris as f64 * 12.0) / 1e6;
            println!("tess {chord} mm: {tris} tris ({:.0}/panel), {verts} verts, wall {:.0} ms, GPU ~{gpu:.0} MB", tris as f64 / n as f64, ms(wall));
        }
        let s = Instant::now();
        let edges: Vec<Vec<Prim2>> = defs.par_iter().map(plan_edges).collect();
        let cnt: usize = edges.iter().map(|e| e.len()).sum();
        println!("plan edges: {cnt} primitives ({:.0}/panel), wall {:.1} ms", cnt as f64 / n as f64, ms(s.elapsed()));
        // boolean cost probes (single op, averaged)
        let probe = |hit: bool| {
            let s = Instant::now();
            for _ in 0..50 {
                let mut m = Model::default();
                let slab = ops::primitive_box(&mut m, [0., 0., 0.], [1200., 600., 50.], &C).unwrap().0;
                let x = if hit { 600.0 } else { 5000.0 };
                let cyl = ops::primitive_cylinder(&mut m, Axis::z_at(Point3::new(x, 300., -10.)), 40.0, 70.0, &C).unwrap().0;
                let _ = ops::cut(&mut m, slab, cyl, &C);
            }
            ms(s.elapsed()) / 50.0
        };
        let probe_box = || {
            let s = Instant::now();
            for _ in 0..50 {
                let mut m = Model::default();
                let slab = ops::primitive_box(&mut m, [0., 0., 0.], [1200., 600., 50.], &C).unwrap().0;
                let b = ops::primitive_box(&mut m, [500., 200., -10.], [700., 400., 60.], &C).unwrap().0;
                let _ = ops::cut(&mut m, slab, b, &C);
            }
            ms(s.elapsed()) / 50.0
        };
        println!("probe: box slab - cylinder through it {:.2} ms; cylinder missing the slab {:.2} ms; slab - box void {:.2} ms", probe(true), probe(false), probe_box());
        let avg = |f: &dyn Fn(&mut Model)| { let s = Instant::now(); for _ in 0..50 { let mut m = Model::default(); f(&mut m); } ms(s.elapsed()) / 50.0 };
        let xy = fr(Point3::origin(), Vec3::z(), Vec3::x());
        let rect = [(0., 0.), (1200., 0.), (1200., 600.), (0., 600.)];
        let holed = |m: &mut Model| {
            let mut p = poly(xy, &rect);
            p.holes.push(ProfileLoop::Circle { center: Point2::new(600., 300.), radius: 40. });
            let r = ops::extrude(m, &p, Vec3::z(), 50.0, &C);
            assert!(r.is_ok(), "{:?}", r.err());
        };
        let ngon = |m: &mut Model| {
            let slab = ops::primitive_box(m, [0., 0., 0.], [1200., 600., 50.], &C).unwrap().0;
            let pts: Vec<(f64, f64)> = (0..32).map(|k| { let a = k as f64 * std::f64::consts::TAU / 32.0; (600. + 40. * a.cos(), 300. + 40. * a.sin()) }).collect();
            let tool = ops::extrude(m, &poly(fr(Point3::new(0., 0., -10.), Vec3::z(), Vec3::x()), &pts), Vec3::z(), 70.0, &C).unwrap().0;
            let _ = ops::cut(m, slab, tool, &C);
        };
        let cyl_only = |m: &mut Model| { let _ = ops::primitive_cylinder(m, Axis::z_at(Point3::new(600., 300., -10.)), 40.0, 70.0, &C); };
        println!("probe: slab extruded from a profile with a circular hole (no boolean) {:.2} ms; slab - 32-gon prism {:.2} ms; cylinder alone {:.3} ms", avg(&holed), avg(&ngon), avg(&cyl_only));
    });
}
