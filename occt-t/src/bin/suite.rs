#[path = "../common.rs"]
mod common;
use common::*;
use opencascade::primitives::{Edge, Face, Wire};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;
fn rep(name: &str, f: impl FnOnce() -> R) {
    if let Some(a) = std::env::args().nth(1) { if a != name { return; } }
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(f));
    let dt = t.elapsed().as_secs_f64() * 1e3;
    let s = match r { Err(_) => { println!("| {name} | PANIC | | {dt:.2} ms | |"); return; } Ok(Err(x)) => { println!("| {name} | ERR: {} | | {dt:.2} ms | |", x.chars().take(200).collect::<String>()); return; } Ok(Ok(s)) => s };
    let mut ms = String::new();
    for chord in [2.0, 0.5] { let t2 = Instant::now(); ms += &match catch_unwind(AssertUnwindSafe(|| tris(&s, chord))) { Ok(Ok(n)) => format!("@{chord}mm {n} tris, {:.2} ms; ", t2.elapsed().as_secs_f64() * 1e3), Ok(Err(x)) => format!("@{chord}mm mesh ERR {x}; "), Err(_) => format!("@{chord}mm mesh PANIC; ") }; }
    println!("| {name} | ok | {} faces | {dt:.2} ms | {ms} |", nfaces(&s));
}
fn sqp() -> Result<Face, String> { poly_face(&[p(0., -100., -100.), p(0., 100., -100.), p(0., 100., 100.), p(0., -100., 100.)]) }
fn main() {
    let z = |h: f64| dvec3(0., 0., h);
    rep("1a extrude rect", || extrude(&[p(0., 0., 0.), p(1000., 0., 0.), p(1000., 600., 0.), p(0., 600., 0.)], z(750.)));
    rep("1b extrude L", || extrude(&[p(0., 0., 0.), p(1000., 0., 0.), p(1000., 200., 0.), p(200., 200., 0.), p(200., 600., 0.), p(0., 600., 0.)], z(750.)));
    rep("2 revolve ring", || ring(p(0., 0., 0.), 100., 300., 500.));
    rep("3a sweep L path (corner)", || { let path = Wire::from_edges([&Edge::segment(p(0., 0., 0.), p(1000., 0., 0.)), &Edge::segment(p(1000., 0., 0.), p(1000., 800., 0.))]); Ok(sqp()?.sweep_along(&path).into()) });
    rep("3a2 L as fuse of 2 boxes", || fuse(&bx(p(0., -100., -100.), p(1100., 100., 100.)), &bx(p(900., -100., -100.), p(1100., 800., 100.))));
    rep("3b sweep arc R1000", || { let s45 = 1000. * std::f64::consts::FRAC_1_SQRT_2; let path = Wire::from_edges([&Edge::arc(p(0., 0., 0.), p(s45, 1000. - s45, 0.), p(1000., 1000., 0.))]); Ok(sqp()?.sweep_along(&path).into()) });
    rep("3c sweep straight line", || { let path = Wire::from_edges([&Edge::segment(p(0., 0., 0.), p(1000., 0., 0.))]); Ok(sqp()?.sweep_along(&path).into()) });
    rep("4 loft square->circle", || loft(&[square_wire(p(0., 0., 0.), 500.)?, circle_wire(p(0., 0., 900.), z(1.), 200.)]));
    rep("4b loft smooth 3 sect", || loft(&[square_wire(p(0., 0., 0.), 500.)?, circle_wire(p(0., 0., 450.), z(1.), 300.), circle_wire(p(0., 0., 900.), z(1.), 200.)]));
    rep("5b blend along arc (smooth loft 3 sect)", || {
        // sections on a quarter arc R1000 (centre (0,1000)): plane normal = tangent
        let on = |deg: f64| { let a = f64::to_radians(deg); (p(1000. * a.sin(), 1000. - 1000. * a.cos(), 0.), dvec3(a.cos(), a.sin(), 0.)) };
        let (p0, t0) = on(0.); let (p1, t1) = on(45.); let (p2, t2) = on(90.);
        let sq = { let nx = dvec3(0., 0., 1.); let ny = t0.cross(nx); let h = 100.; poly_wire(&[p0 + nx * h + ny * h, p0 - nx * h + ny * h, p0 - nx * h - ny * h, p0 + nx * h - ny * h])? };
        loft(&[sq, circle_wire(p1, t1, 80.), circle_wire(p2, t2, 60.)]) });
    rep("6a box - cylinder", || cut(&bx(p(0., 0., 0.), p(1000., 600., 750.)), &cyl(p(500., 300., -100.), 100., 950.)));
    rep("6b box - coplanar box", || cut(&bx(p(0., 0., 0.), p(1000., 600., 750.)), &bx(p(200., 0., 0.), p(800., 300., 750.))));
    rep("6c fuse boxes sharing face", || fuse(&bx(p(0., 0., 0.), p(1000., 600., 750.)), &bx(p(1000., 0., 0.), p(1500., 600., 750.))));
    rep("6d fuse partly shared face", || fuse(&bx(p(0., 0., 0.), p(1000., 600., 750.)), &bx(p(1000., 100., 0.), p(1500., 400., 500.))));
    rep("6e ring(revolve) - box", || cut(&ring(p(0., 0., 0.), 100., 300., 500.)?, &bx(p(0., -400., 250.), p(400., 400., 600.))));
    rep("hostile: NaN extrude", || extrude(&[p(0., 0., 0.), p(f64::NAN, 0., 0.), p(1., 1., 0.)], z(750.)));
    rep("hostile: zero-length extrude", || extrude(&[p(0., 0., 0.), p(10., 0., 0.), p(10., 10., 0.)], z(0.)));
    rep("hostile: self-intersecting", || extrude(&[p(0., 0., 0.), p(10., 10., 0.), p(10., 0., 0.), p(0., 10., 0.)], z(5.)));
}
