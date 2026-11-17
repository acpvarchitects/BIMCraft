#[path = "../common.rs"]
mod common;
use common::*;
use solidcore::blend::fillet_executive::api::fillet_edges;
use solidcore::curve::curve::Curve;
use solidcore::topology::integrity::check;
use solidcore::Kernel;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

fn cant(prefix: &str, name: &str, why: &str) { if let Some(a) = std::env::args().nth(1) { if a != prefix { return; } } println!("| {name} | {why} | |"); }
fn rep(prefix: &str, name: &str, f: impl FnOnce(&mut Kernel) -> R) {
    if let Some(a) = std::env::args().nth(1) { if a != prefix { return; } }
    let mut k = Kernel::default();
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| f(&mut k)));
    let dt = t.elapsed().as_secs_f64() * 1e3;
    match r {
        Err(_) => println!("| {name} | PANIC | {dt:.2} ms |"),
        Ok(Err(e)) => println!("| {name} | ERR: {} | {dt:.2} ms |", e.to_string().chars().take(200).collect::<String>()),
        Ok(Ok(b)) => { let v = match check(&b) { Ok(()) => "valid".to_string(), Err(e) => format!("INVALID: {}", e.to_string().chars().take(80).collect::<String>()) }; println!("| {name} | ok: {} faces, {} edges, {v} | {dt:.2} ms |", b.faces().len(), b.edges().len()) }
    }
}
fn smooth(k: &mut Kernel, o: solidcore::math::Point3) -> R {
    let s: Vec<Curve> = vec![square_closed(p(o.x, o.y, o.z), 500., 3)?, circle_at(p(o.x, o.y, o.z + 450.), 350.)?, square_closed(p(o.x, o.y, o.z + 900.), 300., 3)?];
    loft(k, &s, 2)
}
fn main() {
    let ns = "not run: kernel exposes it as the low-level BSplineCurve/BSplineSurface API; not exercised in this time-boxed pass";
    for (id, n) in [("1a", "1a curve by 6 control points"), ("1b", "1b split"), ("1c", "1c closed periodic curve"), ("1d", "1d curve through points"), ("1e", "1e offset curve in plane"), ("2a", "2a surface 6x6 cps"), ("2b", "2b edit control point"), ("2c", "2c surface through a grid"), ("2d", "2d offset NURBS surface"), ("2e", "2e sheet body from NURBS surface")] { cant(id, n, ns); }
    rep("3a", "3a loft open, ruled", |k| loft_profiles(k, &[square_lines(p(0., 0., 0.), 500.)?, circle_arcs(p(0., 0., 900.), 200.)?], 1));
    rep("3b", "3b loft open, smooth 3 sections", |k| smooth(k, p(0., 0., 0.)));
    cant("3c", "3c loft closed (4 circles round a ring)", ns);
    cant("3d", "3d loft with rails / sweep2 / network / blend surface", "can't build: no rails/sweep2/network API in the public SM_API surface used (create_skin_primitive/create_skin only)");
    cant("3e", "3e sweep1 along a NURBS rail", ns);
    cant("3f", "3f revolve/extrude/planar-cap a NURBS curve", ns);
    rep("4a", "4a loft capped into a solid", |k| smooth(k, p(0., 0., 0.)));
    cant("4b", "4b shell a smooth loft", ns);
    cant("4c", "4c offset a NURBS face of a solid", ns);
    rep("5a", "5a smooth loft - box", |k| { let l = smooth(k, p(0., 0., 0.))?; let b = bx(k, p(200., -100., 300.), p(800., 100., 600.))?; cut(k, l, b) });
    rep("5b", "5b smooth loft - cylinder through it", |k| { let l = smooth(k, p(0., 0., 0.))?; let c = cyl(k, p(0., 0., 450.), 100., 1600.)?; cut(k, l, c) });
    rep("5c", "5c two smooth lofts fused", |k| { let a = smooth(k, p(0., 0., 0.))?; let b = smooth(k, p(400., 150., 100.))?; fuse(k, a, b) });
    rep("5d", "5d box - thin NURBS panel", |k| {
        let sec = |x: f64, y: f64| poly(&[p(x, y + 15., 0.), p(x, y + 15., 1000.), p(x, y - 15., 1000.), p(x, y - 15., 0.)]);
        let panel = loft_profiles(k, &[sec(0., 0.)?, sec(600., 200.)?, sec(1200., 0.)?], 2)?; let b = bx(k, p(100., -300., 200.), p(1100., 400., 800.))?; cut(k, b, panel) });
    rep("5e", "5e NURBS face flush with a box face", |k| { let l = smooth(k, p(0., 0., 0.))?; let b = bx(k, p(-600., -600., -300.), p(600., 600., 0.))?; fuse(k, l, b) });
    rep("6a", "6a fillet plane-plane (box edge, r 20)", |k| { let mut b = bx(k, p(0., 0., 0.), p(1000., 600., 750.))?; let e = b.edges().ids().next().ok_or_else(|| solidcore::SolidError::invalid_input("bench", "no edge"))?; fillet_edges(k, &mut b, &[e], 20., 1, 1, 1.)?; Ok(b) });
    cant("6b", "6b fillet plane-cylinder", ns); cant("6c", "6c fillet NURBS-NURBS edge", ns); cant("6d", "6d fillet NURBS-plane edge", ns); cant("6e", "6e chamfer NURBS-NURBS edge", ns); cant("6f", "6f variable-radius fillet", "API exists (variable_radius_fillet); not exercised in this time-boxed pass");
    cant("7a", "7a split smooth loft by a plane", ns); cant("7b", "7b trim surface by curve/surface", ns);
    for (id, n) in [("8 tessellate smooth loft at 0.5", "8 tessellate smooth loft at 0.5 mm"), ("8 tessellate smooth loft at 2 mm", "8 tessellate smooth loft at 2 mm")] { cant(id, n, "can't build: solidcore 1.0.1 has no surface tessellator (README: tessellation not ported yet)"); }
    cant("9", "9 STEP of a smooth loft", "can't build: no STEP writer in solidcore 1.0.1");
}
