#[path = "../common.rs"]
mod common;
use common::*;
use opencascade::primitives::Shape;
use rayon::prelude::*;
use std::time::Instant;
struct S(Vec<Shape>);
unsafe impl Send for S {}
unsafe impl Sync for S {}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let void: u8 = match a.get(1).map(|s| s.as_str()) { Some("b") => 1, Some("c") => 2, _ => 0 };
    let n: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let threads: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    pool.install(|| {
        let s = Instant::now();
        let res: Vec<Result<S, String>> = (0..n).into_par_iter().map(|i| std::panic::catch_unwind(|| panel(&params(i, void)).map(S)).unwrap_or_else(|_| Err("PANIC".into()))).collect();
        let ok = res.iter().filter(|r| r.is_ok()).count();
        let panics = res.iter().filter(|r| matches!(r, Err(e) if e == "PANIC")).count();
        let first = res.iter().find_map(|r| r.as_ref().err()).cloned().unwrap_or_default();
        println!("PANELS variant={} n={n} threads={threads} build_ok={ok} errors={} panics={panics} build_wall_ms={:.0} first_error={:?}", a[1], n - ok, s.elapsed().as_secs_f64() * 1e3, first);
        use std::io::Write; std::io::stdout().flush().ok();
        let good: Vec<&S> = res.iter().filter_map(|r| r.as_ref().ok()).collect();
        for chord in [2.0, 0.5] {
            let s = Instant::now();
            let tr: usize = good.par_iter().map(|d| d.0.iter().map(|so| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tris(so, chord).unwrap_or(0))).unwrap_or(0)).sum::<usize>()).sum();
            println!("MESH chord={chord}mm tris={tr} per_panel={:.0} wall_ms={:.0} (meshed {} good panels)", tr as f64 / n as f64, s.elapsed().as_secs_f64() * 1e3, good.len());
            std::io::stdout().flush().ok();
        }
    });
}
