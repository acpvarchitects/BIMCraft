#[path = "../common.rs"]
mod common;
use common::*;
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
        let res: Vec<Result<Vec<solidcore::Brep>, String>> = (0..n).into_par_iter().map(|i| {
            std::panic::catch_unwind(|| panel(&params(i, void)).map_err(|e| e.to_string())).unwrap_or_else(|_| Err("PANIC".into()))
        }).collect();
        let ok = res.iter().filter(|r| r.is_ok()).count();
        let first = res.iter().find_map(|r| r.as_ref().err()).cloned().unwrap_or_default();
        let panics = res.iter().filter(|r| matches!(r, Err(e) if e == "PANIC")).count();
        println!("PANELS variant={} n={n} threads={threads} build_ok={ok} errors={} panics={panics} build_wall_ms={:.0} first_error={:?}", a[1], n - ok, s.elapsed().as_secs_f64() * 1e3, first);
        println!("MESH can't build: solidcore 1.0.1 has no surface tessellator (only tessellate_boundaries polylines)");
    });
}
