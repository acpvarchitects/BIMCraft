#[path = "../common.rs"]
mod common;
use common::*;
use fj_core::Core;
use fj_core::objects::Solid;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

fn cant(name: &str, why: &str) {
    if let Some(a) = std::env::args().nth(1) { if a != name { return; } }
    println!("| {name} | CAN'T BUILD: {why} | | | |");
}
fn rep(name: &str, f: impl FnOnce(&mut Core) -> Solid) {
    if let Some(a) = std::env::args().nth(1) { if a != name { return; } }
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| {
        let mut core = Core::new();
        let s = f(&mut core);
        let dt = t.elapsed().as_secs_f64() * 1e3;
        let (v, s) = check(&mut core, s);
        let mut ms = String::new();
        for chord in [2.0, 0.5] {
            let t2 = Instant::now();
            let m = catch_unwind(AssertUnwindSafe(|| mesh_tris(&mut core, &s, chord)));
            ms += &match m { Ok(n) => format!("@{chord}mm {n} tris, {:.2} ms; ", t2.elapsed().as_secs_f64() * 1e3), Err(_) => format!("@{chord}mm mesh PANIC; ") };
        }
        let nf: usize = s.shells().iter().map(|sh| sh.faces().iter().count()).sum();
        println!("| {name} | ok ({v}) | {nf} faces | {dt:.2} ms | {ms} |");
    }));
    if r.is_err() { println!("| {name} | PANIC | | {:.2} ms | |", t.elapsed().as_secs_f64() * 1e3); }
}
fn main() {
    rep("1a extrude rect", |c| extrude(c, &[[0., 0.], [1000., 0.], [1000., 600.], [0., 600.]], None, 750.));
    rep("1b extrude L", |c| extrude(c, &[[0., 0.], [1000., 0.], [1000., 200.], [200., 200.], [200., 600.], [0., 600.]], None, 750.));
    cant("2 revolve ring", "no revolve in fj-core 0.49 (no rotational sweep op; only straight-path sweep)");
    cant("3a sweep L path (corner)", "sweep takes one straight vector; no path, no mitred corner");
    cant("3a2 L as fuse of 2 boxes", "no boolean operations in fj-core 0.49");
    cant("3b sweep arc R1000", "no curved-path sweep");
    rep("3c sweep straight line", |c| extrude(c, &[[0., 0.], [200., 0.], [200., 200.], [0., 200.]], None, 1000.));
    cant("4 loft square->circle", "no loft / skin");
    cant("4b loft smooth 3 sect", "no loft / skin");
    cant("5b blend along arc (smooth loft 3 sect)", "no loft / blend / curved sweep");
    rep("6a box - cylinder", |c| extrude_with_through_hole(c, &[[0., 0.], [1000., 0.], [1000., 600.], [0., 600.]], [500., 300.], 100., 750.));
    cant("6b box - coplanar box", "no boolean operations");
    cant("6c fuse boxes sharing face", "no boolean operations");
    cant("6d fuse partly shared face", "no boolean operations");
    cant("6e ring(revolve) - box", "no revolve, no boolean");
    rep("hostile: NaN extrude", |c| extrude(c, &[[0., 0.], [f64::NAN, 0.], [1., 1.]], None, 750.));
    rep("hostile: zero-length extrude", |c| extrude(c, &[[0., 0.], [10., 0.], [10., 10.]], None, 0.));
    rep("hostile: self-intersecting", |c| extrude(c, &[[0., 0.], [10., 10.], [10., 0.], [0., 10.]], None, 5.));
    let _ = Solid::new([]);
}
