#[path = "../common.rs"]
mod common;
use common::*;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;
fn cant(prefix: &str, why: &str) { if let Some(a) = std::env::args().nth(1) { if a != prefix { return; } } println!("| {prefix} | {why} | |"); }
fn rep(prefix: &str, name: &str, f: impl FnOnce() -> std::result::Result<String, String>) {
    if let Some(a) = std::env::args().nth(1) { if a != prefix { return; } }
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(f));
    let dt = t.elapsed().as_secs_f64() * 1e3;
    match r { Err(_) => println!("| {name} | PANIC | {dt:.2} ms |"), Ok(Err(e)) => println!("| {name} | ERR: {} | {dt:.2} ms |", e.chars().take(200).collect::<String>()), Ok(Ok(s)) => println!("| {name} | ok: {s} | {dt:.2} ms |") }
}
fn summ(s: &Solid) -> String { let nf: usize = s.boundaries().iter().map(|sh| sh.face_iter().count()).sum(); format!("{nf} faces, shell_condition={:?}", s.boundaries()[0].shell_condition()) }
fn lf() -> R { loft_sq_circle(p(0., 0., 0.), 500., 350., 900.) }
fn main() {
    let ns = "not run: truck exposes BSplineCurve/NurbsSurface (truck-geometry) but no op in this pass";
    for id in ["1a", "1b", "1c", "1d", "1e", "2a", "2b", "2c", "2d", "2e", "3c", "3e", "3f", "4b", "4c", "5d", "6b", "6c", "6d", "6e", "7a", "7b"] { cant(id, ns); }
    cant("3d", "can't build: no rails/sweep2/network patch/blend surface");
    cant("6a", "can't build: no fillet/chamfer in truck-modeling 0.6");
    cant("6f", "can't build: no fillet");
    rep("3a", "3a loft open, ruled", || { let l = lf()?; Ok(summ(&l)) });
    cant("3b", "can't build: only ruled homotopy between two wires; no smooth multi-section loft");
    rep("4a", "4a loft capped into a solid", || { let l = lf()?; Ok(summ(&l)) });
    rep("5a", "5a loft - box (box through a free-form side)", || { let l = lf()?; let b = cuboid(p(-200., -100., 300.), p(800., 100., 600.)); let r = cut(&l, &b)?; Ok(summ(&r)) });
    rep("5b", "5b loft - cylinder through it (horizontal)", || { let l = lf()?; let mut c = cylinder(p(0., 0., -800.), 100., 1600.)?; c = builder::rotated(&c, p(0., 0., 0.), Vector3::unit_y(), Rad(std::f64::consts::FRAC_PI_2)); let c = builder::translated(&c, Vector3::new(0., 0., 450.)); let r = cut(&l, &c)?; Ok(summ(&r)) });
    rep("5c", "5c two lofts fused (overlapping)", || { let a = lf()?; let b = builder::translated(&lf()?, Vector3::new(400., 150., 100.)); let r = fuse(&a, &b)?; Ok(summ(&r)) });
    rep("5e", "5e free-form solid flush with a box face", || { let l = lf()?; let b = cuboid(p(-600., -600., -300.), p(600., 600., 0.)); let r = fuse(&l, &b)?; Ok(summ(&r)) });
    for (id, chord) in [("8 tessellate smooth loft at 0.5", 0.5), ("8 tessellate smooth loft at 2 mm", 2.0)] { rep(id, &format!("8 tessellate ruled loft at {chord} mm"), move || { let l = lf()?; let t = Instant::now(); let n = tris(&l, chord); Ok(format!("{n} tris in {:.1} ms", t.elapsed().as_secs_f64() * 1e3)) }); }
    cant("9", "can't build here: STEP export is the separate truck-stepio crate, not in this dependency set");
}
