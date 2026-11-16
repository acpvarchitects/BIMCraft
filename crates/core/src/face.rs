//! Face coordinates: every face of a wall, a floor, a ceiling, a roof or a column has its own 2D coordinates, `(u, v)` in
//! millimetres, worked out from the element as it stands each time they are asked for. What is drawn or recorded on a face
//! (a survey area, a drawn stone, later a division or a shape) is held in them, so it follows its element when it is
//! moved, lengthened, thickened, retyped or curved, with nothing in 3D stored to go stale.
//!
//! A face is named by a [`FaceId`], by what it is to its element rather than by its triangles, so the name lasts through
//! joins, openings and type changes: a wall's `exterior` and `interior` sides, its `start` and `end` and its `top` and
//! `bottom`; a floor's, a ceiling's, a roof's or a column's `top` and `bottom`, and `{"side": i}`, the upright face along
//! edge `i` of its outline in plan (a round column's one curved side is side 0).
//!
//! [`Model::face_chart`] resolves one into a [`Chart`]: the map from `(u, v)` to a point in space and the way the face
//! looks there, the map back, the face's outline in `(u, v)` and the openings the model cuts in it, and what an outline drawn
//! in `(u, v)` measures on it ([`Chart::measure`]).
//!
//! The coordinates on each kind of face:
//!
//! - **A wall's sides** (`exterior`, `interior`): `u` along the face from the point square to the location line's start,
//!   the way the line runs, and `v` up from the wall's own base; on a curved wall `u` is the arc length along that face's
//!   own curve, so `(u, v)` is the side laid out flat (developed) and an area in `(u, v)` is its true area. On a straight
//!   wall `u` is the same as a door's offset along the wall, so a hole's place reads straight across.
//! - **A wall's ends** (`start`, `end`): `u` across the wall from its exterior face, `v` up from its own base.
//! - **The top or bottom of anything** (a floor's, a ceiling's, a roof's, a column's, a wall's): `(u, v)` in plan, from
//!   the element's own origin along its own direction (a slab's first boundary point and first edge, a column's point and
//!   angle, a wall's start and direction), so it moves and turns with the element; the face's height is found under each
//!   point, on the slope or the piece of a shaped floor there, and an area is measured on the slope, not in plan.
//! - **A side** (`{"side": i}`): `u` along edge `i` from its first point, `v` up from the lowest point of the face's bottom.
//!
//! Every surface here is flat or a plumb cylinder, so the map is exact and an area measured in `(u, v)` is the true one.
//! Site toposolids, when they come, are faces of this kind too: a top in plan with a slope under each point.

use serde::{Deserialize, Serialize};

use crate::geom::{self, P2};
use crate::hosting::FaceRole;
use crate::model::{Element, Guid, Model, heading};
use crate::view3d::{P3, cross, dot, unit};
use crate::walls::{self, Axis, Plane};

/// Which face of its element: see the module's notes. In commands and files a word (`"exterior"`) or, for a side,
/// `{"side": i}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaceId {
    /// A wall's side to the left of its location line, start to end, as Place on Face and paint name it.
    Exterior,
    /// A wall's other side.
    Interior,
    /// A wall's end at its start.
    Start,
    /// A wall's end at its end.
    End,
    Top,
    Bottom,
    /// The upright face along edge `i` of a floor's, a ceiling's, a roof's or a column's outline in plan, from point `i` to
    /// the next.
    Side(u32),
}

impl FaceId {
    /// As listings and messages show it: `exterior`, `side 2`.
    pub fn name(self) -> String {
        match self {
            FaceId::Exterior => "exterior".into(),
            FaceId::Interior => "interior".into(),
            FaceId::Start => "start".into(),
            FaceId::End => "end".into(),
            FaceId::Top => "top".into(),
            FaceId::Bottom => "bottom".into(),
            FaceId::Side(i) => format!("side {i}"),
        }
    }

    /// Whether an element of `category` has a face of this kind at all (a side's number is the element's own to have).
    pub fn of(self, category: &str) -> bool {
        match category {
            "wall" => !matches!(self, FaceId::Side(_)),
            "floor" | "ceiling" | "roof" | "column" | "structuralcolumn" | "footing" => {
                matches!(self, FaceId::Top | FaceId::Bottom | FaceId::Side(_))
            }
            _ => false,
        }
    }

    /// The categories that have faces with coordinates.
    pub const HOSTS: [&'static str; 7] = ["wall", "floor", "ceiling", "roof", "column", "structuralcolumn", "footing"];

    /// The role Place on Face and paint name it by, where they have one.
    pub fn role(self) -> Option<FaceRole> {
        match self {
            FaceId::Exterior => Some(FaceRole::Exterior),
            FaceId::Interior => Some(FaceRole::Interior),
            FaceId::Top => Some(FaceRole::Top),
            FaceId::Bottom => Some(FaceRole::Bottom),
            _ => None,
        }
    }
}

impl From<FaceRole> for FaceId {
    fn from(r: FaceRole) -> FaceId {
        match r {
            FaceRole::Exterior => FaceId::Exterior,
            FaceRole::Interior => FaceId::Interior,
            FaceRole::Top => FaceId::Top,
            FaceRole::Bottom => FaceId::Bottom,
        }
    }
}

/// The surface a face lies on, and how `(u, v)` is laid on it.
#[derive(Clone, Debug, PartialEq)]
pub enum Surface {
    /// A plane: the point at `(0, 0)`, the unit directions of `u` and `v`, and the way it faces.
    Plane { origin: P3, u: P3, v: P3, normal: P3 },
    /// A plumb cylinder: its axis in plan, its radius, the angle `u = 0` lies at from the centre, which way `u` turns
    /// (+1 counter-clockwise, -1 clockwise), the height of `v = 0`, whether it faces away from its axis, and the angle
    /// (from `u = 0`, the way it turns) of the face's middle, about which a point is read back.
    Cylinder { centre: P2, radius: f64, a0: f64, turn: f64, z0: f64, outward: bool, middle: f64 },
    /// A top or a bottom: `(u, v)` in plan from `origin` along `x` and square to it (counter-clockwise); its height and the
    /// way it faces from the piece under each point (in plan, model axes), the nearest one where none is.
    Plan { origin: P2, x: P2, pieces: Vec<(Vec<P2>, Plane)>, up: bool },
}

impl Surface {
    /// The point at `uv`.
    pub fn point(&self, uv: P2) -> P3 {
        match self {
            Surface::Plane { origin, u, v, .. } => std::array::from_fn(|i| origin[i] + u[i] * uv.0 + v[i] * uv.1),
            Surface::Cylinder { centre, radius, a0, turn, z0, .. } => {
                let a = a0 + turn * uv.0 / radius;
                [centre.0 + radius * a.cos(), centre.1 + radius * a.sin(), z0 + uv.1]
            }
            Surface::Plan { origin, x, pieces, .. } => {
                let q = *origin + *x * uv.0 + x.perp() * uv.1;
                [q.0, q.1, piece_under(pieces, q).map_or(0.0, |p| p.at(q))]
            }
        }
    }

    /// The way the face looks at `uv`: a unit vector, out of the element.
    pub fn normal(&self, uv: P2) -> P3 {
        match self {
            Surface::Plane { normal, .. } => *normal,
            Surface::Cylinder { radius, a0, turn, outward, .. } => {
                let a = a0 + turn * uv.0 / radius;
                let s = if *outward { 1.0 } else { -1.0 };
                [s * a.cos(), s * a.sin(), 0.0]
            }
            Surface::Plan { origin, x, pieces, up } => {
                let q = *origin + *x * uv.0 + x.perp() * uv.1;
                let g = piece_under(pieces, q).map_or(P2(0.0, 0.0), |p| p.g);
                let n = unit([-g.0, -g.1, 1.0]);
                if *up { n } else { n.map(|c| -c) }
            }
        }
    }

    /// The place of point `p` on it: on a plane, the point square to it; on a cylinder, along it round its axis; on a top
    /// or a bottom, the point in plan.
    pub fn uv(&self, p: P3) -> P2 {
        match self {
            Surface::Plane { origin, u, v, .. } => {
                let d = [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]];
                P2(dot(d, *u), dot(d, *v))
            }
            Surface::Cylinder { centre, radius, a0, turn, z0, middle, .. } => {
                let a = (p[1] - centre.1).atan2(p[0] - centre.0);
                // Turned from u = 0 the way it turns, counted from its middle so the back of the cylinder splits evenly.
                let t = (turn * (a - a0) - middle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI + middle;
                P2(t * radius, p[2] - z0)
            }
            Surface::Plan { origin, x, .. } => {
                let d = P2(p[0], p[1]) - *origin;
                P2(d.dot(*x), d.dot(x.perp()))
            }
        }
    }

    /// How much an area in plan under `q` is on the face: 1 on a plane or a cylinder; on a slope, 1 over its cosine.
    fn stretch(&self, q: P2) -> f64 {
        match self {
            Surface::Plan { origin, x, pieces, .. } => {
                let at = *origin + *x * q.0 + x.perp() * q.1;
                piece_under(pieces, at).map_or(1.0, |p| (1.0 + p.g.dot(p.g)).sqrt())
            }
            _ => 1.0,
        }
    }

    /// Model point `q` in plan in `(u, v)`, for a [`Surface::Plan`]; itself for the others.
    fn plan_uv(&self, q: P2) -> P2 {
        match self {
            Surface::Plan { origin, x, .. } => {
                let d = q - *origin;
                P2(d.dot(*x), d.dot(x.perp()))
            }
            _ => q,
        }
    }
}

/// The piece whose ring holds `q` (or has it on its edge), else the nearest.
fn piece_under(pieces: &[(Vec<P2>, Plane)], q: P2) -> Option<&Plane> {
    let away = |r: &[P2]| geom::ring_edges(r).map(|(a, b)| geom::dist_to_segment(q, a, b)).fold(f64::INFINITY, f64::min);
    pieces
        .iter()
        .find(|(r, _)| geom::contains(r, q))
        .or_else(|| pieces.iter().min_by(|a, b| away(&a.0).total_cmp(&away(&b.0))))
        .map(|(_, p)| p)
}

/// A face as its element stands: its surface, its outline in `(u, v)` and the openings the model cuts in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub element: Guid,
    pub face: FaceId,
    pub surface: Surface,
    /// The face's outline in `(u, v)`, openings not taken out: rings side by side (a roof's top is a ring for each slope).
    pub outline: Vec<Vec<P2>>,
    /// What the model cuts out of it, in `(u, v)`: a wall's doors and windows, a slab's openings.
    pub openings: Vec<Vec<P2>>,
    /// The outline as convex pieces, each with how much its area in `(u, v)` is on the face (1, or 1 over a slope's
    /// cosine): what [`Chart::measure`] reads.
    pieces: Vec<(Vec<P2>, f64)>,
}

/// What an outline drawn in a face's `(u, v)` measures on it ([`Chart::measure`]), mm² on the true surface unless said.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measure {
    /// The part of it on the face, openings and all.
    pub area: f64,
    /// The part of that an opening of the face takes out.
    pub cut: f64,
    /// How much of it lies off the face, mm² in `(u, v)`.
    pub outside: f64,
}

impl Measure {
    /// What is left on the face: its area less what openings take out.
    pub fn net(&self) -> f64 {
        (self.area - self.cut).max(0.0)
    }
}

impl Chart {
    pub fn point(&self, uv: P2) -> P3 {
        self.surface.point(uv)
    }

    pub fn normal(&self, uv: P2) -> P3 {
        self.surface.normal(uv)
    }

    pub fn uv(&self, p: P3) -> P2 {
        self.surface.uv(p)
    }

    /// The face's own axes at `uv`: the point, the way `u` grows there (a unit vector along the face), and the normal. A
    /// frame as a cut drawn on a face takes it (`origin`, `x`, `normal`).
    pub fn frame(&self, uv: P2) -> (P3, P3, P3) {
        let (p, q) = (self.point(uv), self.point(P2(uv.0 + 1.0, uv.1)));
        let along = unit([q[0] - p[0], q[1] - p[1], q[2] - p[2]]);
        let n = self.normal(uv);
        // Square to the normal: on a slope `u` runs level in plan, which the face tilts away from.
        let x = unit(cross(cross(n, along), n));
        (p, x, n)
    }

    /// The face's whole area, mm² on the true surface, openings not taken out.
    pub fn area(&self) -> f64 {
        self.pieces.iter().map(|(r, k)| geom::area(r).abs() * k).sum()
    }

    /// What the outline `outline`, with `holes` in it, drawn in `(u, v)`, measures on the face ([`Measure`]).
    pub fn measure(&self, outline: &[P2], holes: &[Vec<P2>]) -> Measure {
        let own = geom::area(outline).abs() - holes.iter().map(|h| geom::area(h).abs()).sum::<f64>();
        let within = |convex: &[&[P2]]| overlap(outline, convex) - holes.iter().map(|h| overlap(h, convex)).sum::<f64>();
        let (mut area, mut flat) = (0.0, 0.0);
        for (p, k) in &self.pieces {
            let a = within(&[p]).max(0.0);
            flat += a;
            area += a * k;
        }
        let mut cut = 0.0;
        for o in self.openings.iter().flat_map(|o| walls::convex_pieces(o)) {
            for (p, k) in &self.pieces {
                cut += within(&[&o, p]).max(0.0) * k;
            }
        }
        Measure { area, cut: cut.min(area), outside: (own - flat).max(0.0) }
    }
}

/// The area of `ring` inside every one of the convex rings `convex` (Sutherland-Hodgman, exact for a convex clip).
fn overlap(ring: &[P2], convex: &[&[P2]]) -> f64 {
    let mut kept = ring.to_vec();
    for c in convex {
        let ccw = geom::area(c) >= 0.0;
        for (a, b) in geom::ring_edges(c).filter(|(a, b)| a.dist(*b) > 1e-9) {
            let left = |q: P2| if ccw { (b - a).cross(q - a) } else { (a - b).cross(q - b) };
            kept = geom::clip(&kept, left);
            if kept.len() < 3 {
                return 0.0;
            }
        }
    }
    geom::area(&kept).abs()
}

/// A face's surface, its outline and its openings in `(u, v)`, before its pieces are measured.
type Laid = (Surface, Vec<Vec<P2>>, Vec<Vec<P2>>);

/// A plain rectangle in `(u, v)`.
fn rect(u0: f64, u1: f64, v0: f64, v1: f64) -> Vec<P2> {
    vec![P2(u0, v0), P2(u1, v0), P2(u1, v1), P2(u0, v1)]
}

fn finite(p: P3) -> bool {
    p.iter().all(|c| c.is_finite())
}

impl Model {
    /// Face `face` of element `element` as it stands now ([`Chart`]); none where the element is gone or has no such face
    /// (a side past its outline's edges, a wall's side curved to nothing).
    pub fn face_chart(&self, element: Guid, face: FaceId) -> Option<Chart> {
        let e = self.elements.get(element)?;
        if !face.of(e.category()) {
            return None;
        }
        let (surface, outline, openings) = match e {
            Element::Wall(w) => self.wall_face(element, w, face)?,
            Element::Floor(_) | Element::Ceiling(_) | Element::Roof(_) => self.slab_face(e, face)?,
            Element::Column(_) | Element::StructuralColumn(_) | Element::Footing(_) => self.column_face(e, face)?,
            _ => return None,
        };
        if outline.iter().flatten().chain(openings.iter().flatten()).any(|q| !q.is_finite()) {
            return None;
        }
        let mut pieces: Vec<(Vec<P2>, f64)> = Vec::new();
        for ring in &outline {
            for p in walls::convex_pieces(ring) {
                let k = surface.stretch(geom::centroid(&p));
                pieces.push((p, k));
            }
        }
        if matches!(surface, Surface::Plan { .. }) {
            // A slab's pieces leave its openings out already: put them back, so its outline is all of the face.
            for o in &openings {
                for p in walls::convex_pieces(o) {
                    let k = surface.stretch(geom::centroid(&p));
                    pieces.push((p, k));
                }
            }
        }
        Some(Chart { element, face, surface, outline, openings, pieces })
    }

    /// The faces element `element` has, as [`Model::face_chart`] resolves them.
    pub fn faces_of(&self, element: Guid) -> Vec<FaceId> {
        let Some(e) = self.elements.get(element) else { return Vec::new() };
        let sides = match e {
            Element::Wall(_) => 0,
            Element::Floor(f) => f.boundary.len(),
            Element::Ceiling(c) => c.boundary.len(),
            Element::Roof(r) => r.boundary.len(),
            _ if e.column().is_some() => self.column_outline(e).len(),
            _ => return Vec::new(),
        };
        let fixed: &[FaceId] = if matches!(e, Element::Wall(_)) {
            &[FaceId::Exterior, FaceId::Interior, FaceId::Start, FaceId::End, FaceId::Top, FaceId::Bottom]
        } else {
            &[FaceId::Top, FaceId::Bottom]
        };
        let sides = (0..sides).filter_map(|i| u32::try_from(i).ok()).map(FaceId::Side);
        fixed.iter().copied().chain(sides).filter(|f| self.face_chart(element, *f).is_some()).collect()
    }

    fn wall_face(&self, element: Guid, w: &crate::model::Wall, face: FaceId) -> Option<Laid> {
        let axis = Axis::of(w);
        let len = axis.length();
        let (ext, int) = walls::faces(self, w);
        let z0 = self.own_base(w);
        let up: P3 = [0.0, 0.0, 1.0];
        // Where along the location line it is sampled: its stations, more where what it is attached to may bend.
        let stations = if w.attach_base.is_some() || w.attach_top.is_some() {
            (0..=32).map(|i| len * f64::from(i) / 32.0).collect()
        } else {
            axis.stations(0.0, len)
        };
        match face {
            FaceId::Exterior | FaceId::Interior => {
                let exterior = face == FaceId::Exterior;
                let off = if exterior { ext } else { int };
                // How much longer the face is than the location line: 1 when straight, its radius over the line's on an arc.
                let (surface, k) = match axis.arc() {
                    Some((c, r, a0, sweep)) => {
                        let turn = sweep.signum();
                        let radius = r - turn * off;
                        if radius.is_nan() || radius <= 1e-6 {
                            return None;
                        }
                        // The exterior is to the left, which is toward the centre turning left.
                        let outward = if exterior { turn < 0.0 } else { turn > 0.0 };
                        let middle = sweep.abs() / 2.0;
                        (Surface::Cylinder { centre: c, radius, a0, turn, z0, outward, middle }, radius / r)
                    }
                    None => {
                        let t = axis.tangent(0.0);
                        let o = axis.at(0.0, off);
                        let left = t.perp();
                        let n = if exterior { left } else { left * -1.0 };
                        (Surface::Plane { origin: [o.0, o.1, z0], u: [t.0, t.1, 0.0], v: up, normal: [n.0, n.1, 0.0] }, 1.0)
                    }
                };
                let mut ring: Vec<P2> = stations.iter().map(|s| P2(s * k, walls::base_at(self, w, *s) - z0)).collect();
                ring.extend(stations.iter().rev().map(|s| P2(s * k, walls::top_at(self, w, *s) - z0)));
                let openings = self
                    .elements
                    .values()
                    .filter_map(|e| match e {
                        Element::Door(o) | Element::Window(o) if o.host == element => walls::hole(self, w, e, o),
                        _ => None,
                    })
                    .map(|h| rect(h.s0 * k, h.s1 * k, h.bottom - z0, h.top - z0))
                    .collect();
                Some((surface, vec![ring], openings))
            }
            FaceId::Start | FaceId::End => {
                let s = if face == FaceId::Start { 0.0 } else { len };
                let t = axis.tangent(s);
                let o = axis.at(s, ext);
                let across = t.perp() * -1.0;
                let n = if face == FaceId::Start { t * -1.0 } else { t };
                let surface = Surface::Plane { origin: [o.0, o.1, z0], u: [across.0, across.1, 0.0], v: up, normal: [n.0, n.1, 0.0] };
                let ring = rect(0.0, ext - int, walls::base_at(self, w, s) - z0, walls::top_at(self, w, s) - z0);
                Some((surface, vec![ring], Vec::new()))
            }
            FaceId::Top | FaceId::Bottom => {
                let top = face == FaceId::Top;
                let mut band: Vec<P2> = stations.iter().map(|s| axis.at(*s, ext)).collect();
                band.extend(stations.iter().rev().map(|s| axis.at(*s, int)));
                let z = if top { self.top_z(w) } else { self.base_z(w) };
                let surface =
                    Surface::Plan { origin: w.start, x: axis.tangent(0.0), pieces: vec![(band.clone(), Plane::flat(z))], up: top };
                let ring = band.iter().map(|q| surface.plan_uv(*q)).collect();
                Some((surface, vec![ring], Vec::new()))
            }
            FaceId::Side(_) => None,
        }
    }

    fn slab_face(&self, e: &Element, face: FaceId) -> Option<Laid> {
        let pieces = self.slab_pieces(e);
        let (boundary, holes): (&[P2], &[Vec<P2>]) = match e {
            Element::Floor(f) => (&f.boundary, &f.openings),
            Element::Ceiling(c) => (&c.boundary, &c.openings),
            Element::Roof(r) => (&r.boundary, &r.openings),
            _ => return None,
        };
        let first = boundary.first().or_else(|| pieces.first().and_then(|p| p.0.first())).copied()?;
        let x = boundary.get(1).map_or(P2(1.0, 0.0), |b| (*b - first).unit());
        let x = if x.length() > 0.5 { x } else { P2(1.0, 0.0) };
        match face {
            FaceId::Top | FaceId::Bottom => {
                let top = face == FaceId::Top;
                let planes: Vec<(Vec<P2>, Plane)> =
                    pieces.iter().map(|(r, under, over)| (r.clone(), if top { *over } else { *under })).collect();
                let surface = Surface::Plan { origin: first, x, pieces: planes, up: top };
                let to_uv = |r: &Vec<P2>| r.iter().map(|q| surface.plan_uv(*q)).collect::<Vec<P2>>();
                let outline = pieces.iter().map(|(r, ..)| to_uv(r)).collect();
                let openings = holes.iter().map(to_uv).collect();
                Some((surface, outline, openings))
            }
            FaceId::Side(i) => {
                let i = usize::try_from(i).ok()?;
                let (a, b) = (*boundary.get(i)?, *boundary.get((i + 1) % boundary.len())?);
                let height = |q: P2, top: bool| {
                    let near = |r: &[P2]| geom::ring_edges(r).map(|(p, s)| geom::dist_to_segment(q, p, s)).fold(f64::INFINITY, f64::min);
                    let piece = pieces.iter().min_by(|m, n| near(&m.0).total_cmp(&near(&n.0)))?;
                    Some(if top { piece.2.at(q) } else { piece.1.at(q) })
                };
                let (ba, bb, ta, tb) = (height(a, false)?, height(b, false)?, height(a, true)?, height(b, true)?);
                side_face(a, b, geom::area(boundary) >= 0.0, [ba, bb, ta, tb])
            }
            _ => None,
        }
    }

    fn column_face(&self, e: &Element, face: FaceId) -> Option<Laid> {
        let col = e.column()?;
        let t = self.column_type(e)?;
        let outline = self.column_outline(e);
        let (lo, hi) = walls::column_heights(self, e, col);
        match face {
            FaceId::Top | FaceId::Bottom => {
                let top = face == FaceId::Top;
                let z = if top { hi } else { lo };
                let surface =
                    Surface::Plan { origin: col.at, x: heading(col.angle), pieces: vec![(outline.clone(), Plane::flat(z))], up: top };
                let ring = outline.iter().map(|q| surface.plan_uv(*q)).collect();
                let holes = self.column_holes(e).iter().map(|h| h.iter().map(|q| surface.plan_uv(*q)).collect()).collect();
                Some((surface, vec![ring], holes))
            }
            // A round column's side is one curved face, round from its angle counter-clockwise.
            FaceId::Side(0) if t.round && t.profile.is_none() => {
                let radius = t.width / 2.0;
                if radius.is_nan() || radius <= 1e-6 {
                    return None;
                }
                let tau = std::f64::consts::TAU;
                let surface = Surface::Cylinder {
                    centre: col.at,
                    radius,
                    a0: col.angle.to_radians(),
                    turn: 1.0,
                    z0: lo,
                    outward: true,
                    middle: std::f64::consts::PI,
                };
                Some((surface, vec![rect(0.0, tau * radius, 0.0, hi - lo)], Vec::new()))
            }
            FaceId::Side(_) if t.round && t.profile.is_none() => None,
            FaceId::Side(i) => {
                let i = usize::try_from(i).ok()?;
                let (a, b) = (*outline.get(i)?, *outline.get((i + 1) % outline.len())?);
                side_face(a, b, geom::area(&outline) >= 0.0, [lo, lo, hi, hi])
            }
            _ => None,
        }
    }
}

/// The upright face along the edge from `a` to `b` of a ring turning counter-clockwise (`ccw`) or not, its bottom and top
/// at `a` and at `b` given as `[bottom a, bottom b, top a, top b]`.
fn side_face(a: P2, b: P2, ccw: bool, [ba, bb, ta, tb]: [f64; 4]) -> Option<Laid> {
    let len = a.dist(b);
    if len.is_nan() || len <= 1e-6 {
        return None;
    }
    let u = (b - a).unit();
    // Out of the ring: to the right of a counter-clockwise ring's edge.
    let out = if ccw { u.perp() * -1.0 } else { u.perp() };
    let z0 = ba.min(bb);
    let surface = Surface::Plane { origin: [a.0, a.1, z0], u: [u.0, u.1, 0.0], v: [0.0, 0.0, 1.0], normal: [out.0, out.1, 0.0] };
    let ring = vec![P2(0.0, ba - z0), P2(len, bb - z0), P2(len, tb - z0), P2(0.0, ta - z0)];
    finite(surface.point(P2(0.0, 0.0))).then_some((surface, vec![ring], Vec::new()))
}
