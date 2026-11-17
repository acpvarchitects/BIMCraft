#[path = "../common.rs"]
mod common;
use common::*;
use fj_core::Core;
use rayon::prelude::*;
use std::time::Instant;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let void: u8 = match a.get(1).map(|s| s.as_str()) { Some("b") => 1, Some("c") => 2, _ => 0 };
    let n: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let threads: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    pool.install(|| {
        let s = Instant::now();
        let res: Vec<Option<(Core, fj_core::objects::Solid)>> = (0..n).into_par_iter().map(|i| {
            std::panic::catch_unwind(|| { let mut core = Core::new(); let s = panel(&mut core, &params(i, void)); Some((core, s)) }).unwrap_or(None)
        }).collect();
        let ok = res.iter().filter(|r| r.is_some()).count();
        println!("PANELS variant={} n={n} threads={threads} build_ok={ok} errors={} panics={} build_wall_ms={:.0} note=slab+void only (ring revolve and loft: can't build)", a[1], n - ok, n - ok, s.elapsed().as_secs_f64() * 1e3);
        use std::io::Write; std::io::stdout().flush().ok();
        let mut res = res;
        for chord in [2.0, 0.5] {
            let s = Instant::now();
            let tr: usize = res.par_iter_mut().filter_map(|r| r.as_mut()).map(|(c, so)| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mesh_tris(c, so, chord))).unwrap_or(0)).sum();
            println!("MESH chord={chord}mm tris={tr} per_panel={:.0} wall_ms={:.0}", tr as f64 / n as f64, s.elapsed().as_secs_f64() * 1e3);
            std::io::stdout().flush().ok();
        }
    });
}
