#![allow(dead_code)]
pub use truck_meshalgo::prelude::*;
pub use truck_modeling::*;
use std::f64::consts::TAU;

pub type R = std::result::Result<Solid, String>;
pub fn p(x: f64, y: f64, z: f64) -> Point3 { Point3::new(x, y, z) }
pub fn poly_wire(pts: &[Point3]) -> Wire {
    let vs: Vec<Vertex> = pts.iter().map(|q| builder::vertex(*q)).collect();
    let n = vs.len();
    (0..n).map(|i| builder::line(&vs[i], &vs[(i + 1) % n])).collect()
}
pub fn poly_face(pts: &[Point3]) -> std::result::Result<Face, String> { builder::try_attach_plane(&[poly_wire(pts)]).map_err(|e| format!("{e:?}")) }
pub fn circle_wire(c: Point3, r: f64) -> Wire {
    let v = builder::vertex(p(c.x + r, c.y, c.z));
    builder::rsweep(&v, c, Vector3::unit_z(), Rad(TAU))
}
pub fn cuboid(a: Point3, b: Point3) -> Solid {
    let v = builder::vertex(a);
    let e = builder::tsweep(&v, Vector3::new(b.x - a.x, 0., 0.));
    let f = builder::tsweep(&e, Vector3::new(0., b.y - a.y, 0.));
    builder::tsweep(&f, Vector3::new(0., 0., b.z - a.z))
}
pub fn cylinder(base: Point3, r: f64, h: f64) -> std::result::Result<Solid, String> {
    let f = builder::try_attach_plane(&[circle_wire(base, r)]).map_err(|e| format!("{e:?}"))?;
    Ok(builder::tsweep(&f, Vector3::new(0., 0., h)))
}
pub fn extrude(pts: &[Point3], d: Vector3) -> R { Ok(builder::tsweep(&poly_face(pts)?, d)) }
pub fn ring(c: Point3, r0: f64, r1: f64, h: f64, _div: usize) -> R {
    let f = poly_face(&[p(c.x + r0, c.y, c.z), p(c.x + r1, c.y, c.z), p(c.x + r1, c.y, c.z + h), p(c.x + r0, c.y, c.z + h)])?;
    Ok(builder::rsweep(&f, c, Vector3::unit_z(), Rad(TAU)))
}
/// ruled loft square(a) -> circle(r) as 4 arcs, capped
pub fn loft_sq_circle(base: Point3, a: f64, r: f64, dz: f64) -> R {
    let sq = [p(base.x + a, base.y + a, base.z), p(base.x - a, base.y + a, base.z), p(base.x - a, base.y - a, base.z), p(base.x + a, base.y - a, base.z)];
    let w0 = poly_wire(&sq);
    let c = std::f64::consts::FRAC_1_SQRT_2 * r;
    let z = base.z + dz;
    let cv: Vec<Vertex> = [p(base.x + c, base.y + c, z), p(base.x - c, base.y + c, z), p(base.x - c, base.y - c, z), p(base.x + c, base.y - c, z)].iter().map(|q| builder::vertex(*q)).collect();
    let mids = [p(base.x, base.y + r, z), p(base.x - r, base.y, z), p(base.x, base.y - r, z), p(base.x + r, base.y, z)];
    let w1: Wire = (0..4).map(|i| builder::circle_arc(&cv[i], &cv[(i + 1) % 4], mids[i])).collect();
    let mut side = builder::try_wire_homotopy(&w0, &w1).map_err(|e| format!("{e:?}"))?;
    let mut bottom = builder::try_attach_plane(&[w0]).map_err(|e| format!("{e:?}"))?;
    bottom.invert();
    let top = builder::try_attach_plane(&[w1]).map_err(|e| format!("{e:?}"))?;
    side.push(bottom); side.push(top);
    Solid::try_new(vec![side]).map_err(|e| format!("{e:?}"))
}
pub fn cut(a: &Solid, b: &Solid) -> R { let mut nb = b.clone(); nb.not(); truck_shapeops::and(a, &nb, 0.05).ok_or_else(|| "boolean returned None".to_string()) }
pub fn fuse(a: &Solid, b: &Solid) -> R { truck_shapeops::or(a, b, 0.05).ok_or_else(|| "boolean returned None".to_string()) }

pub fn tris(s: &Solid, chord: f64) -> usize {
    let pm = s.triangulation(chord).to_polygon();
    pm.tri_faces().len() + 2 * pm.quad_faces().len() + pm.other_faces().iter().map(|f| f.len().saturating_sub(2)).sum::<usize>()
}

#[derive(Clone, Copy)]
pub struct Params { pub w: f64, pub d: f64, pub t: f64, pub h: f64, pub r_in: f64, pub r_out: f64, pub ring_h: f64, pub loft_a: f64, pub loft_r: f64, pub hole_r: f64, pub void: u8 }
pub fn params(i: usize, void: u8) -> Params {
    let f = (i % 997) as f64;
    Params { w: 1200.0 + f * 0.37, d: 600.0 + (i % 13) as f64 * 3.0, t: 200.0 + (i % 7) as f64, h: 50.0 + (i % 11) as f64,
        r_in: 40.0 + (i % 5) as f64, r_out: 80.0 + (i % 9) as f64, ring_h: 150.0 + (i % 17) as f64,
        loft_a: 300.0 + (i % 19) as f64, loft_r: 60.0 + (i % 23) as f64, hole_r: 30.0 + (i % 29) as f64 * 0.5, void }
}
pub fn panel(pr: &Params) -> std::result::Result<Vec<Solid>, String> {
    let l = [p(0., 0., 0.), p(pr.w, 0., 0.), p(pr.w, pr.t, 0.), p(pr.t, pr.t, 0.), p(pr.t, pr.d, 0.), p(0., pr.d, 0.)];
    let mut slab = if pr.void == 2 {
        let outer = poly_wire(&l);
        let hole = circle_wire(p(pr.w * 0.5, pr.t * 0.5, 0.), pr.hole_r).inverse();
        let f = builder::try_attach_plane(&[outer, hole]).map_err(|e| format!("{e:?}"))?;
        builder::tsweep(&f, Vector3::new(0., 0., pr.h))
    } else { extrude(&l, Vector3::new(0., 0., pr.h))? };
    let rg = ring(p(pr.w * 0.25, pr.t * 0.5, pr.h), pr.r_in, pr.r_out, pr.ring_h, 4)?;
    let lf = loft_sq_circle(p(pr.w * 0.75, pr.t * 0.5, pr.h), pr.loft_a * 0.5, pr.loft_r, 400.)?;
    if pr.void == 1 {
        let c = cylinder(p(pr.w * 0.5, pr.t * 0.5, -10.), pr.hole_r, pr.h + 20.)?;
        slab = cut(&slab, &c)?;
    }
    Ok(vec![slab, rg, lf])
}
