#![allow(dead_code)]
use solidcore::boolean::api_brep::{boolean_difference, boolean_union};
use solidcore::curve::curve::Curve;
use solidcore::math::{Point3, Vec3};
use solidcore::nurbs::b_spline_curve::BSplineCurve;
use solidcore::nurbs::crv_fit::p1::ParamMethod;
use solidcore::primitive::api_curves::{create_arc, create_circle, create_line_segment};
use solidcore::primitive::api_primitives as prim;
use solidcore::sweep::api_brep::{create_linear_sweep, create_rotational_sweep, create_sweep_along_planar_path};
use solidcore::sweep::api_surfaces::create_skin_primitive;
use solidcore::{Brep, Kernel};

pub type R = solidcore::Result<Brep>;
pub fn p(x: f64, y: f64, z: f64) -> Point3 { Point3::new(x, y, z) }
pub fn line(a: Point3, b: Point3) -> solidcore::Result<Curve> { Ok(Curve::from(create_line_segment(a, b)?)) }
/// closed polygon in 3D as line segments
pub fn poly(pts: &[Point3]) -> solidcore::Result<Vec<Curve>> { (0..pts.len()).map(|i| line(pts[i], pts[(i + 1) % pts.len()])).collect() }
pub fn refs(c: &[Curve]) -> Vec<Option<&Curve>> { c.iter().map(Some).collect() }
pub fn crefs(c: &[Curve]) -> Vec<&Curve> { c.iter().collect() }
pub fn extrude(k: &mut Kernel, pts: &[Point3], dir: Vec3, d: f64) -> R { let c = poly(pts)?; create_linear_sweep(k, &refs(&c), dir, d, true) }
/// ring profile in the XZ plane (x r0..r1, z 0..h), revolved about Z through (cx,cy,cz)
pub fn ring(k: &mut Kernel, c: Point3, r0: f64, r1: f64, h: f64) -> R {
    let cs = poly(&[p(c.x + r0, c.y, c.z), p(c.x + r1, c.y, c.z), p(c.x + r1, c.y, c.z + h), p(c.x + r0, c.y, c.z + h)])?;
    create_rotational_sweep(k, &refs(&cs), c, Vec3::new(0.0, 0.0, 1.0), 360.0, true)
}
pub fn square_closed(c: Point3, h: f64, deg: usize) -> solidcore::Result<Curve> {
    let q = [p(c.x + h, c.y + h, c.z), p(c.x - h, c.y + h, c.z), p(c.x - h, c.y - h, c.z), p(c.x + h, c.y - h, c.z), p(c.x + h, c.y + h, c.z)];
    Ok(Curve::from(BSplineCurve::interpolate_points(&q, None, deg, None, None, true, ParamMethod::ChordLength)?))
}
pub fn circle_at(c: Point3, r: f64) -> solidcore::Result<Curve> {
    // create_circle is in XY at origin; move by building ellipse frame is not exposed: build via arcs of the origin and translate not available -> use interpolated closed cubic through 16 points
    let pts: Vec<Point3> = (0..=16).map(|j| { let t = std::f64::consts::TAU * (j % 16) as f64 / 16.0; p(c.x + r * t.cos(), c.y + r * t.sin(), c.z) }).collect();
    Ok(Curve::from(BSplineCurve::interpolate_points(&pts, None, 3, None, None, true, ParamMethod::ChordLength)?))
}
pub fn loft(k: &mut Kernel, secs: &[Curve], degree: usize) -> R {
    create_skin_primitive(k, &vec![1; secs.len()], &refs(secs), degree, true).map(|s| s.brep)
}
/// square as 4 lines (corners ccw from (+h,+h)) at centre c
pub fn square_lines(c: Point3, h: f64) -> solidcore::Result<Vec<Curve>> { poly(&[p(c.x + h, c.y + h, c.z), p(c.x - h, c.y + h, c.z), p(c.x - h, c.y - h, c.z), p(c.x + h, c.y - h, c.z)]) }
/// circle as 4 quarter arcs starting at 45 deg (matches square_lines edge by edge)
pub fn circle_arcs(c: Point3, r: f64) -> solidcore::Result<Vec<Curve>> { (0..4).map(|i| { let (mut a0, mut a1) = (45. + 90. * i as f64, 135. + 90. * i as f64); if a1 > 360. { a0 -= 360.; a1 -= 360.; } Ok(Curve::from(create_arc(c, r, a0, a1)?)) }).collect() }
pub fn loft_profiles(k: &mut Kernel, profiles: &[Vec<Curve>], degree: usize) -> R {
    let counts: Vec<usize> = profiles.iter().map(|x| x.len()).collect();
    let flat: Vec<&Curve> = profiles.iter().flatten().collect();
    let rf: Vec<Option<&Curve>> = flat.into_iter().map(Some).collect();
    create_skin_primitive(k, &counts, &rf, degree, true).map(|s| s.brep)
}
pub fn bx(k: &mut Kernel, a: Point3, b: Point3) -> R { prim::create_box(k, a, b.x - a.x, b.y - a.y, b.z - a.z) }
pub fn cyl(k: &mut Kernel, base: Point3, r: f64, h: f64) -> R { prim::create_cylinder(k, base, r, h) }
pub fn cut(k: &mut Kernel, a: Brep, b: Brep) -> R { boolean_difference(k, a, b) }
pub fn fuse(k: &mut Kernel, a: Brep, b: Brep) -> R { boolean_union(k, a, b) }
pub fn arc_path(c: Point3, r: f64, a0: f64, a1: f64) -> solidcore::Result<Curve> { Ok(Curve::from(create_arc(c, r, a0, a1)?)) }
pub fn _unused(_: &mut Kernel) { let _ = create_circle; let _ = create_sweep_along_planar_path; }

#[derive(Clone, Copy, PartialEq)]
pub struct Params { pub w: f64, pub d: f64, pub t: f64, pub h: f64, pub r_in: f64, pub r_out: f64, pub ring_h: f64, pub loft_a: f64, pub loft_r: f64, pub hole_r: f64, pub void: u8 }
pub fn params(i: usize, void: u8) -> Params {
    let f = (i % 997) as f64;
    Params { w: 1200.0 + f * 0.37, d: 600.0 + (i % 13) as f64 * 3.0, t: 200.0 + (i % 7) as f64, h: 50.0 + (i % 11) as f64,
        r_in: 40.0 + (i % 5) as f64, r_out: 80.0 + (i % 9) as f64, ring_h: 150.0 + (i % 17) as f64,
        loft_a: 300.0 + (i % 19) as f64, loft_r: 60.0 + (i % 23) as f64, hole_r: 30.0 + (i % 29) as f64 * 0.5, void }
}
/// the panel: L slab h thick, a ring revolved, a ruled loft square->circle; void 1 = cylinder cut, 2 = hole in profile
pub fn panel(pr: &Params) -> solidcore::Result<Vec<Brep>> {
    let mut k = Kernel::default();
    let k = &mut k;
    let l = [p(0., 0., 0.), p(pr.w, 0., 0.), p(pr.w, pr.t, 0.), p(pr.t, pr.t, 0.), p(pr.t, pr.d, 0.), p(0., pr.d, 0.)];
    let mut slab = if pr.void == 2 {
        let mut c = poly(&l)?;
        c.push(circle_at(p(pr.w * 0.5, pr.t * 0.5, 0.0), pr.hole_r)?);
        create_linear_sweep(k, &refs(&c), Vec3::new(0., 0., 1.), pr.h, true)?
    } else { extrude(k, &l, Vec3::new(0., 0., 1.), pr.h)? };
    let rg = ring(k, p(pr.w * 0.25, pr.t * 0.5, pr.h), pr.r_in, pr.r_out, pr.ring_h)?;
    let base = p(pr.w * 0.75, pr.t * 0.5, pr.h);
    let lf = loft_profiles(k, &[square_lines(base, pr.loft_a * 0.5)?, circle_arcs(p(base.x, base.y, base.z + 400.0), pr.loft_r)?], 1)?;
    if pr.void == 1 {
        let c = cyl(k, p(pr.w * 0.5, pr.t * 0.5, -10.0), pr.hole_r, pr.h + 20.0)?;
        slab = cut(k, slab, c)?;
    }
    Ok(vec![slab, rg, lf])
}
