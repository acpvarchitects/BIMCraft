//! (bin panels) The 10,000-panel facade for solidcore from its git main, through solidcore-ops:
//! the same panel and parameters as sc-t/src/common.rs (solidcore 1.0.1) and arris-perf/src/bin/panels.rs.
//! usage: panels <a|b|c> <n> <threads> [mesh]   a=no void, b=void by boolean cut, c=void as a profile hole
//! The panel: an L slab h thick, a ring revolved, a ruled loft square->circle; mesh with solidcore-tess
//! at a 2 mm and a 0.5 mm chord (size 1000, as in hardcases.rs).

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
use rayon::prelude::*;
use solidcore_ops::body::{make_cylinder, Body};
use solidcore_ops::boolean::{boolean, BooleanOp};
use solidcore_ops::sweep::{extrude, loft, revolve, Curve, DraftCorner};
use std::f64::consts::{PI, TAU};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

fn e(x: impl std::fmt::Display) -> String { x.to_string() }
fn line(a: [f64; 3], b: [f64; 3]) -> Curve { Curve::Line { start: a, end: b } }
fn poly(p: &[[f64; 3]]) -> Vec<Curve> { (0..p.len()).map(|i| line(p[i], p[(i + 1) % p.len()])).collect() }
fn circle(c: [f64; 3], r: f64) -> Curve { Curve::Arc { center: c, axis: [0., 0., 1.], reference: [1., 0., 0.], radius: r, start: 0.0, end: TAU } }
/// a circle as four quarter arcs from 45 degrees, matching a square's four sides edge by edge
fn circle4(c: [f64; 3], r: f64) -> Vec<Curve> {
    (0..4).map(|i| { let a = PI / 4. + PI / 2. * i as f64; Curve::Arc { center: c, axis: [0., 0., 1.], reference: [1., 0., 0.], radius: r, start: a, end: a + PI / 2. } }).collect()
}
fn square(c: [f64; 3], h: f64) -> Vec<Curve> { poly(&[[c[0] + h, c[1] + h, c[2]], [c[0] - h, c[1] + h, c[2]], [c[0] - h, c[1] - h, c[2]], [c[0] + h, c[1] - h, c[2]]]) }

#[derive(Clone, Copy)]
struct Params { w: f64, d: f64, t: f64, h: f64, r_in: f64, r_out: f64, ring_h: f64, loft_a: f64, loft_r: f64, hole_r: f64, void: u8 }
fn params(i: usize, void: u8) -> Params {
    let f = (i % 997) as f64;
    Params { w: 1200.0 + f * 0.37, d: 600.0 + (i % 13) as f64 * 3.0, t: 200.0 + (i % 7) as f64, h: 50.0 + (i % 11) as f64,
        r_in: 40.0 + (i % 5) as f64, r_out: 80.0 + (i % 9) as f64, ring_h: 150.0 + (i % 17) as f64,
        loft_a: 300.0 + (i % 19) as f64, loft_r: 60.0 + (i % 23) as f64, hole_r: 30.0 + (i % 29) as f64 * 0.5, void }
}

fn panel(p: &Params) -> Result<Vec<Body>, String> {
    let l = [[0., 0., 0.], [p.w, 0., 0.], [p.w, p.t, 0.], [p.t, p.t, 0.], [p.t, p.d, 0.], [0., p.d, 0.]];
    let mut prof = poly(&l);
    if p.void == 2 { prof.push(circle([p.w * 0.5, p.t * 0.5, 0.0], p.hole_r)); }
    let mut slab = extrude(&prof, [0., 0., 1.], p.h, 0.0, DraftCorner::Curvature, true).map_err(e)?;
    let (cx, cy, cz) = (p.w * 0.25, p.t * 0.5, p.h);
    let ring = poly(&[[cx + p.r_in, cy, cz], [cx + p.r_out, cy, cz], [cx + p.r_out, cy, cz + p.ring_h], [cx + p.r_in, cy, cz + p.ring_h]]);
    let ring = revolve(&ring, [cx, cy, cz], [0., 0., 1.], TAU, true).map_err(e)?;
    let base = [p.w * 0.75, p.t * 0.5, p.h];
    let lf = loft(&[square(base, p.loft_a * 0.5), circle4([base[0], base[1], base[2] + 400.0], p.loft_r)], 1, true).map_err(e)?;
    if p.void == 1 {
        let c = make_cylinder([p.w * 0.5, p.t * 0.5, -10.0], p.hole_r, p.h + 20.0).map_err(e)?;
        slab = boolean(slab, c, BooleanOp::Subtract).map_err(e)?.ok_or("the cut came back empty")?;
    }
    Ok(vec![slab, ring, lf])
}

/// triangles of one panel at `chord`, or the first error
fn tess(bodies: &[Body], chord: f64) -> Result<usize, String> {
    let mut tris = 0;
    for b in bodies {
        let (rec, _) = b.to_record().map_err(e)?;
        let (plan, _) = solidcore_tess::mesh(&rec, &solidcore_tess::MeshSpec::chord(chord), 1000.0, None).map_err(e)?;
        tris += plan.welded().map_err(e)?.0.triangles.len();
    }
    Ok(tris)
}

fn ms(d: Duration) -> f64 { d.as_secs_f64() * 1e3 }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let void: u8 = match a.get(1).map(|s| s.as_str()) { Some("b") => 1, Some("c") => 2, _ => 0 };
    let n: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let threads: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mesh = a.get(4).is_some();
    std::panic::set_hook(Box::new(|_| {}));
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    pool.install(|| {
        let s = Instant::now();
        let res: Vec<Result<Vec<Body>, String>> = (0..n).into_par_iter()
            .map(|i| catch_unwind(AssertUnwindSafe(|| panel(&params(i, void)))).unwrap_or_else(|_| Err("PANIC".into())))
            .collect();
        let wall = s.elapsed();
        let panics = res.iter().filter(|r| matches!(r, Err(x) if x == "PANIC")).count();
        let first = res.iter().find_map(|r| r.as_ref().err()).cloned().unwrap_or_default();
        let defs: Vec<Vec<Body>> = res.into_iter().filter_map(Result::ok).collect();
        println!("PANELS variant={} n={n} threads={threads} build_ok={} errors={} panics={panics} build_wall_ms={:.0} first_error={first:?}",
            a[1], defs.len(), n - defs.len(), ms(wall));
        if !mesh { return; }
        use std::io::Write; std::io::stdout().flush().ok();
        for chord in [2.0, 0.5] {
            let s = Instant::now();
            let tv: Vec<Result<usize, String>> = defs.par_iter()
                .map(|d| catch_unwind(AssertUnwindSafe(|| tess(d, chord))).unwrap_or_else(|_| Err("PANIC".into())))
                .collect();
            let tris: usize = tv.iter().filter_map(|x| x.as_ref().ok()).sum();
            let failed = tv.iter().filter(|x| x.is_err()).count();
            let first = tv.iter().find_map(|x| x.as_ref().err()).cloned().unwrap_or_default();
            println!("MESH chord={chord}mm tris={tris} per_panel={:.0} failed={failed} wall_ms={:.0} first_error={first:?}",
                tris as f64 / n as f64, ms(s.elapsed()));
            std::io::stdout().flush().ok();
        }
    });
}
