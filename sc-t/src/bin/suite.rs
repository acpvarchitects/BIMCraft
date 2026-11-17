#[path = "../common.rs"]
mod common;
use common::*;
use solidcore::curve::curve::Curve;
use solidcore::math::Vec3;
use solidcore::primitive::api_curves::create_line_segment;
use solidcore::sweep::api_brep::create_sweep_along_planar_path;
use solidcore::topology::integrity::check;
use solidcore::Kernel;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

fn rep(name: &str, f: impl FnOnce(&mut Kernel) -> R) {
    if let Some(a) = std::env::args().nth(1) { if a != name { return; } }
    let mut k = Kernel::default();
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| f(&mut k)));
    let dt = t.elapsed().as_secs_f64() * 1e3;
    match r {
        Err(_) => println!("| {name} | PANIC | {dt:.2} ms | |"),
        Ok(Err(e)) => println!("| {name} | ERR: {} | {dt:.2} ms | |", e.to_string().chars().take(200).collect::<String>()),
        Ok(Ok(b)) => {
            let valid = match check(&b) { Ok(()) => "valid".to_string(), Err(e) => format!("INVALID: {}", e.to_string().chars().take(80).collect::<String>()) };
            println!("| {name} | ok ({valid}) | {} faces, {} edges | {dt:.2} ms | mesh@2mm,0.5mm: can't build (no tessellator) |", b.faces().len(), b.edges().len());
        }
    }
}
fn main() {
    let z = Vec3::new(0., 0., 1.);
    rep("1a extrude rect", |k| extrude(k, &[p(0., 0., 0.), p(1000., 0., 0.), p(1000., 600., 0.), p(0., 600., 0.)], z, 750.));
    rep("1b extrude L", |k| extrude(k, &[p(0., 0., 0.), p(1000., 0., 0.), p(1000., 200., 0.), p(200., 200., 0.), p(200., 600., 0.), p(0., 600., 0.)], z, 750.));
    rep("2 revolve ring", |k| ring(k, p(0., 0., 0.), 100., 300., 500.));
    // sweeps: profile square in the YZ plane at the origin
    let sq = || poly(&[p(0., -100., -100.), p(0., 100., -100.), p(0., 100., 100.), p(0., -100., 100.)]);
    rep("3a sweep L path (corner)", |k| { let pr = sq()?; let pa = vec![line(p(0., 0., 0.), p(1000., 0., 0.))?, line(p(1000., 0., 0.), p(1000., 800., 0.))?]; create_sweep_along_planar_path(k, &crefs(&pr), &crefs(&pa), false, 3) });
    rep("3a2 L as fuse of 2 boxes", |k| { let a = bx(k, p(0., -100., -100.), p(1100., 100., 100.))?; let b = bx(k, p(900., -100., -100.), p(1100., 800., 100.))?; fuse(k, a, b) });
    rep("3b sweep arc R1000", |k| { let pr = sq()?; let pa = vec![arc_path(p(0., 1000., 0.), 1000., 270., 360.)?]; create_sweep_along_planar_path(k, &crefs(&pr), &crefs(&pa), false, 3) });
    rep("3c sweep straight line", |k| { let pr = sq()?; let pa = vec![Curve::from(create_line_segment(p(0., 0., 0.), p(1000., 0., 0.))?)]; create_sweep_along_planar_path(k, &crefs(&pr), &crefs(&pa), false, 3) });
    let circ_sq = |zc: f64, h: f64, r: f64, deg: usize| -> solidcore::Result<Vec<solidcore::curve::curve::Curve>> { Ok(vec![square_closed(p(0., 0., 0.), h, deg)?, circle_at(p(0., 0., zc), r)?]) };
    let _ = circ_sq;
    rep("4 loft square->circle", |k| loft_profiles(k, &[square_lines(p(0., 0., 0.), 500.)?, circle_arcs(p(0., 0., 900.), 200.)?], 1));
    rep("4b loft smooth 3 sect", |k| { let s = vec![square_closed(p(0., 0., 0.), 500., 3)?, circle_at(p(0., 0., 450.), 300.)?, circle_at(p(0., 0., 900.), 200.)?]; loft(k, &s, 2) });
    rep("5b blend along arc (smooth loft 3 sect)", |_k| Err(solidcore::SolidError::invalid_input("bench", "not built: curve constructors make XY-plane profiles only; rotating sections onto the arc's tangent planes was not attempted (an unrotated attempt fails: coplanar profiles cannot be capped)")));
    rep("6a box - cylinder", |k| { let a = bx(k, p(0., 0., 0.), p(1000., 600., 750.))?; let c = cyl(k, p(500., 300., -100.), 100., 950.)?; cut(k, a, c) });
    rep("6b box - coplanar box", |k| { let a = bx(k, p(0., 0., 0.), p(1000., 600., 750.))?; let b = bx(k, p(200., 0., 0.), p(800., 300., 750.))?; cut(k, a, b) });
    rep("6c fuse boxes sharing face", |k| { let a = bx(k, p(0., 0., 0.), p(1000., 600., 750.))?; let b = bx(k, p(1000., 0., 0.), p(1500., 600., 750.))?; fuse(k, a, b) });
    rep("6d fuse partly shared face", |k| { let a = bx(k, p(0., 0., 0.), p(1000., 600., 750.))?; let b = bx(k, p(1000., 100., 0.), p(1500., 400., 500.))?; fuse(k, a, b) });
    rep("6e ring(revolve) - box", |k| { let r = ring(k, p(0., 0., 0.), 100., 300., 500.)?; let b = bx(k, p(0., -400., 250.), p(400., 400., 600.))?; cut(k, r, b) });
    rep("hostile: NaN extrude", |k| extrude(k, &[p(0., 0., 0.), p(f64::NAN, 0., 0.), p(1., 1., 0.)], z, 750.));
    rep("hostile: zero-length extrude", |k| extrude(k, &[p(0., 0., 0.), p(10., 0., 0.), p(10., 10., 0.)], z, 0.));
    rep("hostile: self-intersecting", |k| extrude(k, &[p(0., 0., 0.), p(10., 10., 0.), p(10., 0., 0.), p(0., 10., 0.)], z, 5.));
}
