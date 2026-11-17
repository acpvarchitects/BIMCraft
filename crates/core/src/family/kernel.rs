//! GEO-1, step 2 (`docs/brep-plan.md`): what a family's forms are built through. [`FormKernel`] makes the solids of the
//! forms: an extrusion, a revolve, a sweep, the lofts of a blend and a swept blend, a solid less another, the box a solid
//! fills, its triangles, its edges and its faces. Its types are BIMCraft's own ([`Loops`], [`Path`], [`Sections`],
//! [`ExactEdge`], [`ExactFace`]): nothing outside an adapter names another library's.
//!
//! [`MeshKernel`] is the code that built them before, behind the trait, bit for bit (`family_kernel_tests`): a solid is its
//! triangles, arcs chorded at [`super::SEGMENTS`] a turn. [`Loops`] and [`Path`] keep the arcs and circles they were drawn
//! with exact beside the chorded regions the mesh is made of, for a kernel that builds them exactly.
//!
//! Where the trait doesn't fit the code yet (for the design note's review): a revolve's axis lies in its profile's plane, so
//! it takes the frame too; booleans spend one budget a whole family's build shares, so they take it ([`Budget`]); a blend and a swept blend are recipes ([`Sections`]: two rings mapped by a twist, and a swept blend's path),
//! not a list of placed sections; what a sweep or a revolve covers seen from above comes from how it was built, so a solid
//! keeps it for [`FormKernel::silhouette`]; nothing builds a fuse yet (a join cuts), so the mesh kernel has none.

use crate::csg;
use crate::geom::{P2, Region};
use crate::view3d::P3;

use super::Frame;
use super::forms;
use super::sketch::{FormCurve, FormLine};
use super::{Expr, sketch};

/// A curve of a sketch in its plane's axes, as drawn: a line, an arc (counter-clockwise from `from` to `to`, radians, `to`
/// greater; one drawn clockwise has them the other way), or a circle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve2 {
    Line { start: P2, end: P2 },
    Arc { centre: P2, radius: f64, from: f64, to: f64 },
    Circle { centre: P2, radius: f64 },
}

/// The curves of `lines` but their axis, each as drawn, read by `num`: an arc's centre and radius from its ends and its
/// turn. Each once, in the sketch's order; which loop each is in, the regions say.
pub fn curves<'a>(
    lines: impl Iterator<Item = &'a FormLine>,
    num: &mut dyn FnMut(&'a Expr) -> Result<f64, String>,
) -> Result<Vec<Curve2>, String> {
    let mut out = Vec::new();
    for l in lines.filter(|l| !l.axis) {
        out.push(match &l.curve {
            FormCurve::Circle { centre, radius } => Curve2::Circle { centre: P2(num(&centre[0])?, num(&centre[1])?), radius: num(radius)? },
            FormCurve::Line { start, end, turn } => {
                let (a, b) = (P2(num(&start[0])?, num(&start[1])?), P2(num(&end[0])?, num(&end[1])?));
                match turn {
                    None => Curve2::Line { start: a, end: b },
                    Some(t) => {
                        let theta = num(t)?.to_radians();
                        let chord = b - a;
                        let len = chord.length();
                        if theta.abs() < 1e-12 || len < 1e-12 {
                            Curve2::Line { start: a, end: b }
                        } else {
                            // The centre lies on the chord's perpendicular, to the left for a counter-clockwise turn.
                            let radius = len / (2.0 * (theta.abs() / 2.0).sin());
                            let rise = (len / 2.0) / (theta / 2.0).tan();
                            let centre = (a + b) * 0.5 + chord.unit().perp() * rise;
                            let from = (a - centre).1.atan2((a - centre).0);
                            let (from, to) = if theta > 0.0 { (from, from + theta) } else { (from + theta, from) };
                            Curve2::Arc { centre, radius, from, to }
                        }
                    }
                }
            }
        });
    }
    Ok(out)
}

/// A profile: the regions the mesh is made of (each outline counter-clockwise, its holes clockwise, arcs chorded), and the
/// curves they were drawn with, exact.
#[derive(Clone, Debug, Default)]
pub struct Loops {
    pub regions: Vec<Region>,
    pub curves: Vec<Curve2>,
}

/// A sweep's path: the chorded one the mesh follows ([`forms::Path`]), and the curves it was drawn with, exact.
#[derive(Clone, Debug)]
pub struct Path {
    pub chorded: forms::Path,
    pub curves: Vec<Curve2>,
}

/// A loft's recipe: a Blend's two rings on `frame` at `from` and `to`, mapped corner to corner by `twist`; or a Swept
/// Blend's, carried along `path` on `host` from the first to the second.
#[derive(Clone, Debug)]
pub enum Sections {
    Blend { base: Vec<P2>, top: Vec<P2>, from: f64, to: f64, twist: i32, frame: Frame },
    Along { first: Vec<P2>, last: Vec<P2>, twist: i32, path: Path, host: Frame },
}

/// Why a kernel couldn't build what it was asked: said as the form's errors are.
#[derive(Clone, Debug, PartialEq)]
pub struct KernelError(pub String);

impl From<String> for KernelError {
    fn from(s: String) -> KernelError {
        KernelError(s)
    }
}

impl From<KernelError> for String {
    fn from(e: KernelError) -> String {
        e.0
    }
}

/// A solid's triangles, every one turned out, the first `caps` of them its caps (where it was built so).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub tris: Vec<[P3; 3]>,
    pub caps: usize,
}

/// An edge of a solid: a line, or (for a kernel that keeps them) an arc or a circle, in the family's axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExactEdge {
    Line(P3, P3),
    Arc { centre: P3, normal: P3, radius: f64, from: P3, to: P3 },
    Circle { centre: P3, normal: P3, radius: f64 },
}

/// A face of a solid: a plane and the triangles of the solid on it (the mesh kernel's coplanar groups), or (for a kernel
/// that keeps them) a curved surface, still to come.
#[derive(Clone, Debug, PartialEq)]
pub enum ExactFace {
    Plane { normal: P3, offset: f64, tris: Vec<[P3; 3]> },
}

/// What booleans may still cost, which one family's build spends together: today's [`csg::Meter`].
pub type Budget = csg::Meter;

/// What a solid covers seen from above: its outline (counter-clockwise) and its holes (clockwise).
pub type Silhouette = (Vec<P2>, Vec<Vec<P2>>);

/// What builds a family's forms. See the module's notes.
pub trait FormKernel: Send + Sync {
    type Solid: Send + Sync + Clone;
    /// `profile`, in `frame`'s plane, swept along its normal from `from` to `to`.
    fn extrude(&self, profile: &Loops, frame: &Frame, from: f64, to: f64) -> Result<Self::Solid, KernelError>;
    /// `profile`, in `frame`'s plane, turned about the line through `axis` (in that plane) from `start` to `end` degrees.
    fn revolve(&self, profile: &Loops, axis: [P2; 2], frame: &Frame, start: f64, end: f64) -> Result<Self::Solid, KernelError>;
    /// `profile`, in its profile plane's axes, carried along `path` on `host`.
    fn sweep(&self, profile: &Loops, path: &Path, host: &Frame) -> Result<Self::Solid, KernelError>;
    /// A blend's or a swept blend's solid.
    fn loft(&self, sections: &Sections) -> Result<Self::Solid, KernelError>;
    /// `body` less `tool`, spending `budget`.
    fn cut(&self, body: &Self::Solid, tool: &Self::Solid, budget: &mut Budget) -> Result<Self::Solid, KernelError>;
    /// `a` and `b` as one, spending `budget`.
    fn fuse(&self, a: &Self::Solid, b: &Self::Solid, budget: &mut Budget) -> Result<Self::Solid, KernelError>;
    /// What it covers seen from above, where it knows that (none: whoever asks works it out from its triangles).
    fn silhouette(&self, s: &Self::Solid) -> Option<Silhouette>;
    /// Its least and greatest corners; none for a solid of nothing.
    fn bounds(&self, s: &Self::Solid) -> Option<[P3; 2]>;
    /// Its triangles, no chord longer off its surface than `chord` mm where it is curved.
    fn tessellate(&self, s: &Self::Solid, chord: f64) -> Mesh;
    fn edges(&self, s: &Self::Solid) -> Vec<ExactEdge>;
    fn faces(&self, s: &Self::Solid) -> Vec<ExactFace>;
}

/// The mesh kernel's solid: its triangles and caps, and what it covers seen from above where it was built knowing that (a
/// sweep on a level plane: the band; a revolve: its outline).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshSolid {
    pub mesh: Mesh,
    pub plan: Option<forms::Plan>,
}

/// Today's code behind the trait: see the module's notes.
#[derive(Clone, Copy, Debug, Default)]
pub struct MeshKernel;

impl FormKernel for MeshKernel {
    type Solid = MeshSolid;

    fn extrude(&self, profile: &Loops, frame: &Frame, from: f64, to: f64) -> Result<MeshSolid, KernelError> {
        let (tris, caps) = super::sweep(&profile.regions, from, to, frame)?;
        Ok(MeshSolid { mesh: Mesh { tris, caps }, plan: None })
    }

    fn revolve(&self, profile: &Loops, axis: [P2; 2], frame: &Frame, start: f64, end: f64) -> Result<MeshSolid, KernelError> {
        let (tris, outline) = super::revolve(&profile.regions, axis, (start, end), frame)?;
        let caps = tris.len();
        Ok(MeshSolid { mesh: Mesh { tris, caps }, plan: Some((outline, Vec::new())) })
    }

    fn sweep(&self, profile: &Loops, path: &Path, host: &Frame) -> Result<MeshSolid, KernelError> {
        let (tris, plan) = forms::sweep(&profile.regions, &path.chorded, host)?;
        Ok(MeshSolid { mesh: Mesh { caps: tris.len(), tris }, plan })
    }

    fn loft(&self, sections: &Sections) -> Result<MeshSolid, KernelError> {
        let (tris, plan) = match sections {
            Sections::Blend { base, top, from, to, twist, frame } => (forms::blend(base, top, (*from, *to), *twist, frame)?, None),
            Sections::Along { first, last, twist, path, host } => forms::swept_blend(first, last, *twist, &path.chorded, host)?,
        };
        Ok(MeshSolid { mesh: Mesh { caps: tris.len(), tris }, plan })
    }

    fn cut(&self, body: &MeshSolid, tool: &MeshSolid, budget: &mut Budget) -> Result<MeshSolid, KernelError> {
        let tris = csg::subtract(&body.mesh.tris, &tool.mesh.tris, budget)?;
        Ok(MeshSolid { mesh: Mesh { caps: tris.len(), tris }, plan: body.plan.clone() })
    }

    fn fuse(&self, _: &MeshSolid, _: &MeshSolid, _: &mut Budget) -> Result<MeshSolid, KernelError> {
        Err(KernelError("the mesh kernel joins no solids: a join cuts one by the other".into()))
    }

    fn silhouette(&self, s: &MeshSolid) -> Option<Silhouette> {
        s.plan.clone()
    }

    fn bounds(&self, s: &MeshSolid) -> Option<[P3; 2]> {
        let mut pts = s.mesh.tris.iter().flatten();
        let first = *pts.next()?;
        Some(pts.fold([first, first], |[lo, hi], p| [std::array::from_fn(|k| lo[k].min(p[k])), std::array::from_fn(|k| hi[k].max(p[k]))]))
    }

    fn tessellate(&self, s: &MeshSolid, _chord: f64) -> Mesh {
        s.mesh.clone()
    }

    fn edges(&self, s: &MeshSolid) -> Vec<ExactEdge> {
        sketch::creases(&s.mesh.tris).into_iter().map(|(a, b)| ExactEdge::Line(a, b)).collect()
    }

    fn faces(&self, s: &MeshSolid) -> Vec<ExactFace> {
        use crate::view3d::{cross, dot};
        let mut out: Vec<ExactFace> = Vec::new();
        for t in &s.mesh.tris {
            let n = cross(std::array::from_fn(|i| t[1][i] - t[0][i]), std::array::from_fn(|i| t[2][i] - t[0][i]));
            let len = dot(n, n).sqrt();
            if len <= 1e-12 {
                continue;
            }
            let normal = n.map(|x| x / len);
            let offset = dot(normal, t[0]);
            let same = |f: &ExactFace| {
                let ExactFace::Plane { normal: m, offset: d, .. } = f;
                dot(*m, normal) > 1.0 - 1e-9 && (d - offset).abs() < 1e-6
            };
            match out.iter_mut().find(|f| same(f)) {
                Some(ExactFace::Plane { tris, .. }) => tris.push(*t),
                None => out.push(ExactFace::Plane { normal, offset, tris: vec![*t] }),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Loops {
        let outer = vec![P2(0.0, 0.0), P2(100.0, 0.0), P2(100.0, 100.0), P2(0.0, 100.0)];
        Loops { regions: vec![Region { outer, holes: Vec::new() }], curves: Vec::new() }
    }

    #[test]
    fn the_mesh_kernel_gives_a_boxs_bounds_six_faces_and_twelve_edges() {
        let k = MeshKernel;
        let mut budget = Budget::default();
        let b = k.extrude(&square(), &Frame::LEVEL, 0.0, 50.0).unwrap();
        assert_eq!(k.bounds(&b), Some([[0.0, 0.0, 0.0], [100.0, 100.0, 50.0]]));
        assert_eq!(k.faces(&b).len(), 6);
        assert_eq!(k.edges(&b).len(), 12);
        assert_eq!(k.tessellate(&b, 0.5), b.mesh);
        assert!(k.fuse(&b, &b, &mut budget).is_err(), "no fuse yet");
        assert_eq!(k.silhouette(&b), None, "an extrusion's is worked out from its profile");
        // A box less a smaller one through it: a hole, so more faces.
        let small = Loops {
            regions: vec![Region { outer: vec![P2(25.0, 25.0), P2(75.0, 25.0), P2(75.0, 75.0), P2(25.0, 75.0)], holes: Vec::new() }],
            curves: Vec::new(),
        };
        let tool = k.extrude(&small, &Frame::LEVEL, -10.0, 60.0).unwrap();
        let cut = k.cut(&b, &tool, &mut budget).unwrap();
        assert!(k.faces(&cut).len() > 6);
    }

    #[test]
    fn an_arc_is_read_exactly_from_its_ends_and_turn() {
        let lines: Vec<FormLine> = serde_json::from_value(serde_json::json!([
            {"start": [100, 0], "end": [0, 100], "turn": 90},
            {"start": [0, 0], "end": [10, 0]},
            {"centre": [5, 5], "radius": 2}
        ]))
        .unwrap();
        let mut num = |e: &Expr| super::super::number(e, &Default::default(), &mut Default::default());
        let c = curves(lines.iter(), &mut num).unwrap();
        let Curve2::Arc { centre, radius, from, to } = c[0] else { panic!("{:?} is an arc", c[0]) };
        assert!(centre.dist(P2(0.0, 0.0)) < 1e-9 && (radius - 100.0).abs() < 1e-9, "{centre:?} {radius}");
        assert!(from.abs() < 1e-9 && (to - std::f64::consts::FRAC_PI_2).abs() < 1e-9, "{from} {to}");
        assert_eq!(c[1], Curve2::Line { start: P2(0.0, 0.0), end: P2(10.0, 0.0) });
        assert_eq!(c[2], Curve2::Circle { centre: P2(5.0, 5.0), radius: 2.0 });
    }
}
