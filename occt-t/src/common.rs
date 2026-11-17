#![allow(dead_code)]
pub use glam::{dvec3, DVec3};
use opencascade::primitives::{Edge, Face, Shape, Solid, Wire};
use std::f64::consts::TAU;

pub type R = Result<Shape, String>;
pub fn e<T: std::fmt::Debug>(x: T) -> String { format!("{x:?}") }
pub fn p(x: f64, y: f64, z: f64) -> DVec3 { dvec3(x, y, z) }
pub fn poly_wire(pts: &[DVec3]) -> Result<Wire, String> { Wire::from_ordered_points(pts.iter().copied()).map_err(e) }
pub fn poly_face(pts: &[DVec3]) -> Result<Face, String> { Ok(Face::from_wire(&poly_wire(pts)?)) }
pub fn extrude(pts: &[DVec3], d: DVec3) -> R { Ok(poly_face(pts)?.extrude(d).into()) }
pub fn ring(c: DVec3, r0: f64, r1: f64, h: f64) -> R {
    let f = poly_face(&[p(c.x + r0, c.y, c.z), p(c.x + r1, c.y, c.z), p(c.x + r1, c.y, c.z + h), p(c.x + r0, c.y, c.z + h)])?;
    Ok(f.revolve(c, dvec3(0., 0., 1.), None).into())
}
pub fn circle_wire(c: DVec3, n: DVec3, r: f64) -> Wire { Wire::from_edges([&Edge::circle(c, n, r)]) }
pub fn loft(wires: &[Wire]) -> R { Ok(Solid::loft(wires.iter()).into()) }
pub fn bx(a: DVec3, b: DVec3) -> Shape { Shape::box_from_corners(a, b) }
pub fn cyl(base: DVec3, r: f64, h: f64) -> Shape { Shape::cylinder(base, r, dvec3(0., 0., 1.), h) }
pub fn cut(a: &Shape, b: &Shape) -> R { Ok(a.subtract(b).shape) }
pub fn fuse(a: &Shape, b: &Shape) -> R { Ok(a.union(b).shape) }
pub fn tris(s: &Shape, chord: f64) -> Result<usize, String> { Ok(s.mesh_with_tolerance(chord).map_err(e)?.indices.len() / 3) }
pub fn nfaces(s: &Shape) -> usize { s.faces().count() }
pub fn _t() { let _ = TAU; }

#[derive(Clone, Copy)]
pub struct Params { pub w: f64, pub d: f64, pub t: f64, pub h: f64, pub r_in: f64, pub r_out: f64, pub ring_h: f64, pub loft_a: f64, pub loft_r: f64, pub hole_r: f64, pub void: u8 }
pub fn params(i: usize, void: u8) -> Params {
    let f = (i % 997) as f64;
    Params { w: 1200.0 + f * 0.37, d: 600.0 + (i % 13) as f64 * 3.0, t: 200.0 + (i % 7) as f64, h: 50.0 + (i % 11) as f64,
        r_in: 40.0 + (i % 5) as f64, r_out: 80.0 + (i % 9) as f64, ring_h: 150.0 + (i % 17) as f64,
        loft_a: 300.0 + (i % 19) as f64, loft_r: 60.0 + (i % 23) as f64, hole_r: 30.0 + (i % 29) as f64 * 0.5, void }
}
pub fn square_wire(c: DVec3, h: f64) -> Result<Wire, String> { poly_wire(&[p(c.x + h, c.y + h, c.z), p(c.x - h, c.y + h, c.z), p(c.x - h, c.y - h, c.z), p(c.x + h, c.y - h, c.z)]) }
pub fn panel(pr: &Params) -> Result<Vec<Shape>, String> {
    let l = [p(0., 0., 0.), p(pr.w, 0., 0.), p(pr.w, pr.t, 0.), p(pr.t, pr.t, 0.), p(pr.t, pr.d, 0.), p(0., pr.d, 0.)];
    let mut slab: Shape = if pr.void == 2 {
        let outer = poly_wire(&l)?;
        let hole = circle_wire(p(pr.w * 0.5, pr.t * 0.5, 0.), dvec3(0., 0., 1.), pr.hole_r);
        Face::from_wire_with_holes(&outer, &[hole]).extrude(dvec3(0., 0., pr.h)).into()
    } else { extrude(&l, dvec3(0., 0., pr.h))? };
    let rg = ring(p(pr.w * 0.25, pr.t * 0.5, pr.h), pr.r_in, pr.r_out, pr.ring_h)?;
    let base = p(pr.w * 0.75, pr.t * 0.5, pr.h);
    let lf = loft(&[square_wire(base, pr.loft_a * 0.5)?, circle_wire(p(base.x, base.y, base.z + 400.), dvec3(0., 0., 1.), pr.loft_r)])?;
    if pr.void == 1 { slab = cut(&slab, &cyl(p(pr.w * 0.5, pr.t * 0.5, -10.), pr.hole_r, pr.h + 20.))?; }
    Ok(vec![slab, rg, lf])
}
