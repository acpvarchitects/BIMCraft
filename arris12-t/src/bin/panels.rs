//! (bin panels) arris-perf/src/bin/panels.rs, unchanged but for this header, built on arris 0.12.0 (arris-perf pins 0.11.1).
//! (bin panels) Facade-panel throughput spike for arris: every panel a distinct definition.
//! usage: panels <a|b|c> <n> <threads> [mesh]   a=no void, b=void by boolean cut, c=void as a profile hole
//!   a = extrude L + revolve + loft (no boolean); b = a + one void cylinder cut through the L slab

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
use arris::Control;
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
struct Params { w: f64, d: f64, t: f64, h: f64, r_in: f64, r_out: f64, ring_h: f64, loft_a: f64, loft_r: f64, hole_r: f64, void: u8 }


fn params(i: usize, drive: f64, void: u8) -> Params {
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
    let mut lp = poly(xy, &[(0., 0.), (p.w, 0.), (p.w, p.t), (p.t, p.t), (p.t, p.d), (0., p.d)]);
    if p.void == 2 { lp.holes.push(ProfileLoop::Circle { center: Point2::new(p.w * 0.5, p.t * 0.5), radius: p.hole_r }); }
    let (slab, _) = ops::extrude(&mut m, &lp, Vec3::z(), p.h, &C).map_err(e)?;
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
    if p.void == 1 {
        let s = Instant::now();
        let (cyl, _) = ops::primitive_cylinder(&mut m, Axis::z_at(Point3::new(p.w * 0.5, p.t * 0.5, -10.0)), p.hole_r, p.h + 20.0, &C).map_err(e)?;
        slab = ops::cut(&mut m, slab, cyl, &C).map_err(e)?.0;
        t.boolean = s.elapsed();
    }
    Ok((Def { m, bodies: vec![slab, ring, loft] }, t))
}

fn tess(d: &Def, chord: f64) -> (usize, usize) {
    let (mut tris, mut verts) = (0, 0);
    for &b in &d.bodies {
        if let Ok(me) = arris::mesh::tessellate(&d.m, b, chord, &C) { tris += me.triangles().len(); verts += me.positions().len(); }
    }
    (tris, verts)
}


fn ms(d: Duration) -> f64 { d.as_secs_f64() * 1e3 }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let void: u8 = match a.get(1).map(|s| s.as_str()) { Some("b") => 1, Some("c") => 2, _ => 0 };
    let n: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let threads: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mesh = a.get(4).is_some();
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    pool.install(|| {
        let ps: Vec<Params> = (0..n).map(|i| params(i, 0.0, void)).collect();
        let s = Instant::now();
        let res: Vec<Result<(Def, Times), String>> = ps.par_iter().map(build).collect();
        let wall = s.elapsed();
        let (mut errs, mut defs) = (0, Vec::with_capacity(n));
        for r in res { match r { Ok((d, _)) => defs.push(d), Err(x) => { if errs == 0 { eprintln!("err: {x}"); } errs += 1; } } }
        println!("PANELS variant={} n={n} threads={threads} build_ok={} errors={errs} build_wall_ms={:.0}", a[1], defs.len(), ms(wall));
        if !mesh { return; }
        use std::io::Write; std::io::stdout().flush().ok();
        for chord in [2.0, 0.5] {
            let s = Instant::now();
            let tv: Vec<(usize, usize)> = defs.par_iter().map(|d| tess(d, chord)).collect();
            let tris: usize = tv.iter().map(|x| x.0).sum();
            println!("MESH chord={chord}mm tris={tris} per_panel={:.0} wall_ms={:.0}", tris as f64 / n as f64, ms(s.elapsed()));
            std::io::stdout().flush().ok();
        }
    });
}
