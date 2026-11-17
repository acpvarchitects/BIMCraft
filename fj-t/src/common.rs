#![allow(dead_code)]
use fj_core::algorithms::approx::Tolerance;
use fj_core::algorithms::triangulate::Triangulate;
use fj_core::objects::{Cycle, Region, Sketch, Solid};
use fj_core::operations::build::{BuildCycle, BuildRegion, BuildSketch};
use fj_core::operations::holes::{AddHole, HoleLocation};
use fj_core::operations::insert::Insert;
use fj_core::operations::reverse::Reverse;
use fj_core::operations::sweep::SweepSketch;
use fj_core::operations::update::{UpdateRegion, UpdateSketch};
use fj_core::Core;

/// polygon (ccw, in XY), optional circular hole in the profile, swept along +z by h
pub fn extrude(core: &mut Core, pts: &[[f64; 2]], hole: Option<([f64; 2], f64)>, h: f64) -> Solid {
    let mut region = Region::polygon(pts.iter().copied(), core);
    if let Some((c, r)) = hole {
        let cyc = Cycle::circle(c, r, core).reverse(core);
        region = region.add_interiors([cyc], core);
    }
    let sketch = Sketch::empty().add_regions([region], core);
    let surface = core.layers.objects.surfaces.xy_plane();
    sketch.sweep_sketch(surface, [0., 0., h], core)
}
/// through hole by the hole operation on a swept slab (not a boolean)
pub fn extrude_with_through_hole(core: &mut Core, pts: &[[f64; 2]], c: [f64; 2], r: f64, h: f64) -> Solid {
    let solid = extrude(core, pts, None, h);
    let shell = solid.shells().only().clone();
    let faces: Vec<_> = shell.faces().iter().cloned().collect();
    let (entry, exit) = (&faces[0], faces.last().unwrap());
    let shell2 = shell.add_through_hole([HoleLocation { face: entry, position: c.into() }, HoleLocation { face: exit, position: c.into() }], r, core);
    Solid::new([shell2.insert(core)])
}
pub fn check(core: &mut Core, s: Solid) -> (String, Solid) {
    let s2 = s.insert(core);
    let v = match core.layers.validation.take_errors() { Ok(()) => "valid".to_string(), Err(e) => format!("INVALID: {} errors: {}", e.0.len(), format!("{e}").chars().take(100).collect::<String>().replace('\n', " ")) };
    ((v), (*s2).clone())
}
pub fn mesh_tris(core: &mut Core, s: &Solid, chord: f64) -> usize {
    let tol = Tolerance::from_scalar(chord).unwrap();
    (s, tol).triangulate(core).triangles().count()
}

#[derive(Clone, Copy)]
pub struct Params { pub w: f64, pub d: f64, pub t: f64, pub h: f64, pub hole_r: f64, pub void: u8 }
pub fn params(i: usize, void: u8) -> Params {
    let f = (i % 997) as f64;
    Params { w: 1200.0 + f * 0.37, d: 600.0 + (i % 13) as f64 * 3.0, t: 200.0 + (i % 7) as f64, h: 50.0 + (i % 11) as f64, hole_r: 30.0 + (i % 29) as f64 * 0.5, void }
}
pub fn panel(core: &mut Core, p: &Params) -> Solid {
    let l = [[0., 0.], [p.w, 0.], [p.w, p.t], [p.t, p.t], [p.t, p.d], [0., p.d]];
    match p.void {
        1 => extrude_with_through_hole(core, &l, [p.w * 0.5, p.t * 0.5], p.hole_r, p.h),
        2 => extrude(core, &l, Some(([p.w * 0.5, p.t * 0.5], p.hole_r)), p.h),
        _ => extrude(core, &l, None, p.h),
    }
}
