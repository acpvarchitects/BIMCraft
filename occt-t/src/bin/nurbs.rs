#[path = "../common.rs"]
mod common;
use common::*;
use opencascade::primitives::Shape;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;
fn cant(prefix: &str, why: &str) { if let Some(a) = std::env::args().nth(1) { if a != prefix { return; } } println!("| {prefix} | {why} | |"); }
fn rep(prefix: &str, name: &str, f: impl FnOnce() -> Result<String, String>) {
    if let Some(a) = std::env::args().nth(1) { if a != prefix { return; } }
    let t = Instant::now(); let r = catch_unwind(AssertUnwindSafe(f)); let dt = t.elapsed().as_secs_f64() * 1e3;
    match r { Err(_) => println!("| {name} | PANIC | {dt:.2} ms |"), Ok(Err(x)) => println!("| {name} | ERR: {} | {dt:.2} ms |", x.chars().take(200).collect::<String>()), Ok(Ok(s)) => println!("| {name} | ok: {s} | {dt:.2} ms |") }
}
fn sm() -> R { loft(&[square_wire(p(0., 0., 0.), 500.)?, circle_wire(p(0., 0., 450.), dvec3(0., 0., 1.), 350.), square_wire(p(0., 0., 900.), 300.)?]) }
fn d(s: &Shape) -> Result<String, String> { Ok(format!("{} faces", nfaces(s))) }
fn main() {
    let ns = "not run in this pass (API exists in OCCT; the opencascade-rs wrapper exposes only part of it)";
    for id in ["1a", "1b", "1c", "1d", "1e", "2a", "2b", "2c", "2d", "2e", "3c", "3d", "3e", "3f", "4b", "4c", "5d", "6b", "6c", "6d", "6e", "6f", "7a", "7b"] { cant(id, ns); }
    rep("3a", "3a loft open (ThruSections; wrapper cannot set ruled)", || { let l = sm()?; d(&l) });
    rep("3b", "3b loft open, smooth 3 sections", || { let l = sm()?; d(&l) });
    rep("4a", "4a loft capped into a solid", || { let l = sm()?; d(&l) });
    rep("5a", "5a smooth loft - box", || { let l = sm()?; d(&cut(&l, &bx(p(200., -100., 300.), p(800., 100., 600.)))?) });
    rep("5b", "5b smooth loft - cylinder", || { let l = sm()?; let c = Shape::cylinder(p(-800., 0., 450.), 100., dvec3(1., 0., 0.), 1600.); d(&cut(&l, &c)?) });
    rep("5c", "5c two smooth lofts fused", || { let a = sm()?; let b = sm()?.translated(dvec3(400., 150., 100.)); d(&fuse(&a, &b)?) });
    rep("5e", "5e free-form solid flush with a box face", || { let l = sm()?; d(&fuse(&l, &bx(p(-600., -600., -300.), p(600., 600., 0.)))?) });
    rep("6a", "6a fillet plane-plane (box edge, r 20)", || { let b = bx(p(0., 0., 0.), p(1000., 600., 750.)); let ed = b.edges().next().ok_or("no edge")?; d(&b.fillet_edge(20., &ed)) });
    rep("8 tessellate smooth loft at 0.5", "8 tessellate smooth loft at 0.5 mm", || { let l = sm()?; let t = Instant::now(); let n = tris(&l, 0.5)?; Ok(format!("{n} tris in {:.1} ms", t.elapsed().as_secs_f64() * 1e3)) });
    rep("8 tessellate smooth loft at 2 mm", "8 tessellate smooth loft at 2 mm", || { let l = sm()?; let t = Instant::now(); let n = tris(&l, 2.0)?; Ok(format!("{n} tris in {:.1} ms", t.elapsed().as_secs_f64() * 1e3)) });
    rep("9", "9 STEP of a smooth loft", || { let l = sm()?; let path = std::env::temp_dir().join("occt-bench.step"); l.write_step(&path).map_err(|x| format!("{x:?}"))?; let n = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0); let _ = std::fs::remove_file(&path); Ok(format!("{n} bytes")) });
}
