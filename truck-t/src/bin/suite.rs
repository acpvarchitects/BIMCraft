#[path = "../common.rs"]
mod common;
use common::*;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

fn cant(name: &str, why: &str) { if let Some(a) = std::env::args().nth(1) { if a != name { return; } } println!("| {name} | CAN'T BUILD: {why} | | | |"); }
fn rep(name: &str, f: impl FnOnce() -> R) {
    if let Some(a) = std::env::args().nth(1) { if a != name { return; } }
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(f));
    let dt = t.elapsed().as_secs_f64() * 1e3;
    let s = match r {
        Err(_) => { println!("| {name} | PANIC | | {dt:.2} ms | |"); return; }
        Ok(Err(e)) => { println!("| {name} | ERR: {} | | {dt:.2} ms | |", e.chars().take(200).collect::<String>()); return; }
        Ok(Ok(s)) => s,
    };
    let mut fk: BTreeMap<String, usize> = BTreeMap::new(); let mut nf = 0;
    for sh in s.boundaries() { for f in sh.face_iter() { nf += 1; let k = format!("{:?}", f.surface()); *fk.entry(k.split('(').next().unwrap_or("").to_string()).or_default() += 1; } }
    let mut ms = String::new();
    for chord in [2.0, 0.5] {
        let t2 = Instant::now();
        let m = catch_unwind(AssertUnwindSafe(|| tris(&s, chord)));
        ms += &match m { Ok(n) => format!("@{chord}mm {n} tris, {:.2} ms; ", t2.elapsed().as_secs_f64() * 1e3), Err(_) => format!("@{chord}mm mesh PANIC; ") };
    }
    println!("| {name} | ok | {nf} faces {fk:?} | {dt:.2} ms | {ms} |");
}
fn main() {
    let z = |h: f64| Vector3::new(0., 0., h);
    rep("1a extrude rect", || extrude(&[p(0., 0., 0.), p(1000., 0., 0.), p(1000., 600., 0.), p(0., 600., 0.)], z(750.)));
    rep("1b extrude L", || extrude(&[p(0., 0., 0.), p(1000., 0., 0.), p(1000., 200., 0.), p(200., 200., 0.), p(200., 600., 0.), p(0., 600., 0.)], z(750.)));
    rep("2 revolve ring", || ring(p(0., 0., 0.), 100., 300., 500., 4));
    cant("3a sweep L path (corner)", "no sweep-along-path builder (only tsweep straight / rsweep); nearest = fuse of 2 boxes (see 3a2)");
    rep("3a2 L as fuse of 2 boxes", || fuse(&cuboid(p(0., -100., -100.), p(1100., 100., 100.)), &cuboid(p(900., -100., -100.), p(1100., 800., 100.))));
    rep("3b sweep arc R1000", || { let f = poly_face(&[p(0., -100., -100.), p(0., 100., -100.), p(0., 100., 100.), p(0., -100., 100.)])?; Ok(builder::rsweep(&f, p(0., 1000., 0.), Vector3::unit_z(), Rad(std::f64::consts::FRAC_PI_2))) });
    rep("3c sweep straight line", || { let f = poly_face(&[p(0., -100., -100.), p(0., 100., -100.), p(0., 100., 100.), p(0., -100., 100.)])?; Ok(builder::tsweep(&f, Vector3::new(1000., 0., 0.))) });
    rep("4 loft square->circle", || loft_sq_circle(p(0., 0., 0.), 500., 200., 900.));
    cant("4b loft smooth 3 sect", "only ruled two-wire homotopy; no multi-section smooth loft");
    cant("5b blend along arc (smooth loft 3 sect)", "no multi-section/smooth loft, no frame-placed sections");
    rep("6a box - cylinder", || cut(&cuboid(p(0., 0., 0.), p(1000., 600., 750.)), &cylinder(p(500., 300., -100.), 100., 950.)?));
    rep("6b box - coplanar box", || cut(&cuboid(p(0., 0., 0.), p(1000., 600., 750.)), &cuboid(p(200., 0., 0.), p(800., 300., 750.))));
    rep("6c fuse boxes sharing face", || fuse(&cuboid(p(0., 0., 0.), p(1000., 600., 750.)), &cuboid(p(1000., 0., 0.), p(1500., 600., 750.))));
    rep("6d fuse partly shared face", || fuse(&cuboid(p(0., 0., 0.), p(1000., 600., 750.)), &cuboid(p(1000., 100., 0.), p(1500., 400., 500.))));
    rep("6e ring(revolve) - box", || cut(&ring(p(0., 0., 0.), 100., 300., 500., 4)?, &cuboid(p(0., -400., 250.), p(400., 400., 600.))));
    rep("hostile: NaN extrude", || extrude(&[p(0., 0., 0.), p(f64::NAN, 0., 0.), p(1., 1., 0.)], z(750.)));
    rep("hostile: zero-length extrude", || extrude(&[p(0., 0., 0.), p(10., 0., 0.), p(10., 10., 0.)], z(0.)));
    rep("hostile: self-intersecting", || extrude(&[p(0., 0., 0.), p(10., 10., 0.), p(10., 0., 0.), p(0., 10., 0.)], z(5.)));
}
