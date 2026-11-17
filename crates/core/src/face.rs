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
//! The coordinates on each kind of face are Place on Face's where it has them (`Model::face_at`, `Model::face_uv`):
//!
//! - **A wall's sides** (`exterior`, `interior`): `u` mm along the location line from its start, as a door's offset is,
//!   and `v` up from the wall's own base. On a curved wall a side is longer than its location line by its radius over the
//!   line's, which areas are measured by (developed), and a place in `(u, v)` stays square to the same point of the line
//!   when the wall is thickened, retyped or its location line moves, so an area over a window stays over it.
//! - **A wall's ends** (`start`, `end`): `u` across the wall from its exterior face, `v` up from its own base.
//! - **The top or bottom of anything** (a floor's, a ceiling's, a roof's, a column's, a wall's): `(u, v)` is the point in
//!   plan, in the model's axes; the face's height is found under each point (the slope, or the piece of a shaped floor,
//!   there), and an area is measured on the slope, not in plan. A command that moves, turns or mirrors the element takes
//!   what is drawn there with it, as it takes the components on it.
//! - **A side** (`{"side": i}`): `u` along edge `i` from its first point, `v` up from the lowest point of the face's bottom.
//!
//! Every surface here is flat or a plumb cylinder (a round column's side, whose `u` goes round and round), so the map is
//! exact and an area measured in `(u, v)` is the true one. Site toposolids, when they come, are faces of this kind too: a
//! top in plan with a slope under each point.

use std::f64::consts::{PI, TAU};

use serde::{Deserialize, Serialize};

use crate::geom::{self, P2};
use crate::hosting::{FaceRole, piece_at};
use crate::model::{Element, Floor, Guid, Id, Model, Roof};
use crate::view3d::{P3, dot, sub, unit};
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
    /// A wall's side: `u` along the location line `axis`, the side `off` mm to its left of it, `v` up from `z0`; the way it
    /// faces, left of the line (`out` +1) or right of it (-1); `k`, how much longer the side is than the line (1 on a
    /// straight wall, the side's radius over the line's on a curved one).
    Side { axis: Axis, off: f64, z0: f64, out: f64, k: f64 },
    /// A plumb cylinder: its axis in plan, its radius, the angle `u = 0` lies at from the centre, the height of `v = 0`; `u`
    /// is the arc length counter-clockwise, round and round.
    Round { centre: P2, radius: f64, a0: f64, z0: f64 },
    /// A top or a bottom: `(u, v)` the point in plan; its height and the way it faces from the piece under each point
    /// ([`piece_at`]), `holes` none of it (a ceiling's openings), `up` whether it faces up.
    Plan { pieces: Vec<(Vec<P2>, Plane)>, holes: Vec<Vec<P2>>, up: bool },
}

impl Surface {
    /// The plane under plan point `q`, on a top or a bottom.
    fn plane(pieces: &[(Vec<P2>, Plane)], holes: &[Vec<P2>], q: P2) -> Plane {
        piece_at(pieces, |p| &p.0, holes, q).map_or(Plane::flat(0.0), |(p, _)| p.1)
    }

    /// The point at `uv`.
    pub fn point(&self, uv: P2) -> P3 {
        match self {
            Surface::Plane { origin, u, v, .. } => std::array::from_fn(|i| origin[i] + u[i] * uv.0 + v[i] * uv.1),
            Surface::Side { axis, off, z0, .. } => {
                let q = along(axis, uv.0, *off);
                [q.0, q.1, z0 + uv.1]
            }
            Surface::Round { centre, radius, a0, z0 } => {
                let a = a0 + uv.0 / radius;
                [centre.0 + radius * a.cos(), centre.1 + radius * a.sin(), z0 + uv.1]
            }
            Surface::Plan { pieces, holes, .. } => [uv.0, uv.1, Surface::plane(pieces, holes, uv).at(uv)],
        }
    }

    /// The way the face looks at `uv`: a unit vector, out of the element.
    pub fn normal(&self, uv: P2) -> P3 {
        match self {
            Surface::Plane { normal, .. } => *normal,
            Surface::Side { axis, out, .. } => {
                let n = tangent(axis, uv.0).perp() * *out;
                [n.0, n.1, 0.0]
            }
            Surface::Round { radius, a0, .. } => {
                let a = a0 + uv.0 / radius;
                [a.cos(), a.sin(), 0.0]
            }
            Surface::Plan { pieces, holes, up } => {
                let g = Surface::plane(pieces, holes, uv).g;
                let n = unit([-g.0, -g.1, 1.0]);
                if *up { n } else { n.map(|c| -c) }
            }
        }
    }

    /// The place of point `p` on it: on a plane, the point square to it; on a wall's side, how far along the location line
    /// the nearest point is (carried on past its ends) and its height; round a column, along it; on a top or a bottom, the
    /// point in plan.
    pub fn uv(&self, p: P3) -> P2 {
        match self {
            Surface::Plane { origin, u, v, .. } => {
                let d = sub(p, *origin);
                P2(dot(d, *u), dot(d, *v))
            }
            Surface::Side { axis, z0, .. } => P2(station(axis, P2(p[0], p[1])), p[2] - z0),
            Surface::Round { centre, radius, a0, z0 } => {
                let a = (p[1] - centre.1).atan2(p[0] - centre.0);
                P2((a - a0).rem_euclid(TAU) * radius, p[2] - z0)
            }
            Surface::Plan { .. } => P2(p[0], p[1]),
        }
    }

    /// How much an area in `(u, v)` at `q` is on the face: on a slope 1 over its cosine, on a curved wall's side `k`, else 1.
    fn stretch(&self, q: P2) -> f64 {
        match self {
            Surface::Plan { pieces, holes, .. } => {
                let g = Surface::plane(pieces, holes, q).g;
                (1.0 + g.dot(g)).sqrt()
            }
            Surface::Side { k, .. } => *k,
            _ => 1.0,
        }
    }
}

/// The point `s` mm along `axis` (carried on along its tangent, or round its circle, past its ends), `off` mm to its left.
fn along(axis: &Axis, s: f64, off: f64) -> P2 {
    match axis.arc() {
        Some(_) => axis.at(s, off),
        None => {
            let t = axis.tangent(0.0);
            axis.start + t * s + t.perp() * off
        }
    }
}

fn tangent(axis: &Axis, s: f64) -> P2 {
    axis.tangent(s)
}

/// How far along `axis` the point square to `q` is: on a straight one carried on past its ends; on an arc, round from its
/// start the way it turns, counted from its middle so a point past either end goes to the nearer.
fn station(axis: &Axis, q: P2) -> f64 {
    match axis.arc() {
        Some((c, r, a0, sweep)) => {
            let mid = sweep.abs() / 2.0;
            let t = (sweep.signum() * ((q.1 - c.1).atan2(q.0 - c.0) - a0) - mid + PI).rem_euclid(TAU) - PI + mid;
            t * r
        }
        None => (q - axis.start).dot(axis.tangent(0.0)),
    }
}

/// A face as its element stands: its surface, its outline in `(u, v)` and the openings the model cuts in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub element: Guid,
    pub face: FaceId,
    pub surface: Surface,
    /// The face's outline in `(u, v)`, openings not taken out: rings side by side (a roof's top is a ring for each slope).
    pub outline: Vec<Vec<P2>>,
    /// What the model cuts out of it, in `(u, v)`: a wall's doors and windows (their voids' shapes, where their families
    /// have voids) and what components on its faces cut; a slab's openings, the shafts through it and what components on
    /// it cut; a hollow column's hole.
    pub openings: Vec<Vec<P2>>,
    /// The face, openings and all, as convex pieces, each with how much its area in `(u, v)` is on the face: what
    /// [`Chart::measure`] reads.
    pieces: Vec<(Vec<P2>, f64)>,
    /// How far `u` goes before it comes round again, on a face that goes all the way round (a round column's side).
    period: Option<f64>,
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

/// The axes of a face at a point ([`Chart::frame`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: P3,
    /// The way `u` grows there, square to the normal (on a slope, level in plan as `u` is).
    pub x: P3,
    /// The way `v` grows there, square to the normal and to `x`. Not always `normal × x`: on a wall's exterior, its end
    /// and a bottom, where `u` runs right to left seen from outside, `x × y` is the inward normal.
    pub y: P3,
    pub normal: P3,
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

    /// The face's own axes at `uv`: where it is, which way `u` and `v` grow, and the normal ([`Frame`]): what a cut drawn
    /// on a face takes (`origin`, `x`, `normal`, and `y` for the side of `x` that `v` is on).
    pub fn frame(&self, uv: P2) -> Frame {
        let p = self.point(uv);
        let n = self.normal(uv);
        let flat = |d: P3| unit(sub(d, n.map(|c| c * dot(d, n))));
        let x = flat(sub(self.point(P2(uv.0 + 1.0, uv.1)), p));
        let y = flat(sub(self.point(P2(uv.0, uv.1 + 1.0)), p));
        // `y` square to `x` too, on `v`'s side of it.
        let y = unit(sub(y, x.map(|c| c * dot(y, x))));
        Frame { origin: p, x, y, normal: n }
    }

    /// The face's whole area, mm² on the true surface, openings not taken out.
    pub fn area(&self) -> f64 {
        self.pieces.iter().map(|(r, k)| geom::area(r).abs() * k).sum()
    }

    /// How far round `u` comes back to where it began, on a face that goes all the way round.
    pub fn period(&self) -> Option<f64> {
        self.period
    }

    /// What the outline `outline`, with `holes` in it, drawn in `(u, v)`, measures on the face ([`Measure`]). On a face
    /// that goes round, an outline across where `u` starts again measures as it lies.
    pub fn measure(&self, outline: &[P2], holes: &[Vec<P2>]) -> Measure {
        let own = geom::area(outline).abs() - holes.iter().map(|h| geom::area(h).abs()).sum::<f64>();
        // The copies of the outline a period apart that may lie over the face.
        let shifts: Vec<f64> = match self.period.filter(|p| *p > 0.0) {
            Some(p) => {
                let (lo, hi) = outline.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), q| (lo.min(q.0), hi.max(q.0)));
                let (a, b) = ((lo / p).floor(), (hi / p).floor());
                if !(a.is_finite() && b.is_finite()) || b - a > 64.0 {
                    vec![0.0]
                } else {
                    (0..=(b - a) as i64).map(|i| -(a + i as f64) * p).collect()
                }
            }
            None => vec![0.0],
        };
        let moved = |ring: &[P2], d: f64| -> Vec<P2> { ring.iter().map(|q| P2(q.0 + d, q.1)).collect() };
        let within = |convex: &[&[P2]]| {
            shifts
                .iter()
                .map(|d| overlap(&moved(outline, *d), convex) - holes.iter().map(|h| overlap(&moved(h, *d), convex)).sum::<f64>())
                .sum::<f64>()
        };
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

    /// Ring `ring` in `(u, v)` in space: each edge cut where it crosses from one plane of the face to another (a roof's
    /// ridge or valley) and, on a curved face, every `step` mm, so the points follow the face.
    pub fn lift(&self, ring: &[P2], step: f64) -> Vec<P3> {
        let mut out = Vec::new();
        for (a, b) in geom::ring_edges(ring) {
            let mut ts: Vec<f64> = vec![0.0];
            match &self.surface {
                Surface::Side { axis, .. } if axis.arc().is_some() => ts.extend(steps(a.dist(b), step)),
                Surface::Round { .. } => ts.extend(steps(a.dist(b), step)),
                Surface::Plan { pieces, .. } if pieces.len() > 1 => {
                    for (r, _) in pieces {
                        ts.extend(geom::ring_edges(r).filter_map(|(c, d)| geom::crossing(a, b, c, d)));
                    }
                }
                _ => {}
            }
            ts.sort_by(f64::total_cmp);
            ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
            out.extend(ts.into_iter().map(|t| self.point(a + (b - a) * t)));
        }
        out
    }
}

/// The parameters strictly between 0 and 1 that cut a length `len` every `step`.
fn steps(len: f64, step: f64) -> Vec<f64> {
    let n = if step > 0.0 { ((len / step).ceil() as usize).clamp(1, 256) } else { 1 };
    (1..n).map(|i| i as f64 / n as f64).collect()
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

/// A face as laid out, before its pieces are measured: its surface, its outline and its openings in `(u, v)`, whether
/// its outline leaves its openings out already (a floor's and a roof's pieces do), and its period.
struct Laid {
    surface: Surface,
    outline: Vec<Vec<P2>>,
    openings: Vec<Vec<P2>>,
    openings_left_out: bool,
    period: Option<f64>,
}

impl Laid {
    fn new(surface: Surface, outline: Vec<Vec<P2>>, openings: Vec<Vec<P2>>) -> Laid {
        Laid { surface, outline, openings, openings_left_out: false, period: None }
    }
}

/// A plain rectangle in `(u, v)`.
fn rect(u0: f64, u1: f64, v0: f64, v1: f64) -> Vec<P2> {
    vec![P2(u0, v0), P2(u1, v0), P2(u1, v1), P2(u0, v1)]
}

/// The points of a void (a closed surface of triangles in space) seen square on `surface`, as one ring in `(u, v)`
/// (their hull), when the void reaches the face: some of it on the face or behind it, some on it or in front.
fn void_on(surface: &Surface, tris: &[[P3; 3]]) -> Option<Vec<P2>> {
    let mut pts = Vec::new();
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in tris.iter().flatten() {
        let uv = surface.uv(*p);
        let d = dot(sub(*p, surface.point(uv)), surface.normal(uv));
        (lo, hi) = (lo.min(d), hi.max(d));
        pts.push(uv);
    }
    let reaches = lo <= 1e-6 && hi >= -1e-6;
    let ring = geom::hull(pts);
    (reaches && ring.len() >= 3 && geom::area(&ring).abs() > 1.0).then_some(ring)
}

/// How finely a round column's top is laid out: enough sides that a polygon misses the circle by a negligible share.
const ROUND: usize = 512;

impl Model {
    /// Face `face` of element `element` as it stands now ([`Chart`]); none where the element is gone or has no such face
    /// (a side past its outline's edges, a wall's side curved to nothing).
    pub fn face_chart(&self, element: Guid, face: FaceId) -> Option<Chart> {
        let e = self.elements.get(element)?;
        if !face.of(e.category()) {
            return None;
        }
        let id = self.elements.id(element)?;
        let laid = match e {
            Element::Wall(w) => self.wall_face(id, element, w, face)?,
            Element::Floor(_) | Element::Ceiling(_) | Element::Roof(_) => self.slab_face(id, e, face)?,
            Element::Column(_) | Element::StructuralColumn(_) | Element::Footing(_) => self.column_face(e, face)?,
            _ => return None,
        };
        let Laid { surface, outline, openings, openings_left_out, period } = laid;
        if outline.iter().flatten().chain(openings.iter().flatten()).any(|q| !q.is_finite()) {
            return None;
        }
        let mut pieces: Vec<(Vec<P2>, f64)> = Vec::new();
        let mut add = |ring: &[P2]| {
            for p in walls::convex_pieces(ring) {
                let k = surface.stretch(geom::centroid(&p));
                pieces.push((p, k));
            }
        };
        outline.iter().for_each(|r| add(r));
        // Its pieces leave its own openings out already (a floor's, a roof's): put them back, so its pieces are all of the
        // face, and its outline with them.
        let mut outline = outline;
        if let (true, Element::Floor(Floor { openings: own, .. }) | Element::Roof(Roof { openings: own, .. })) = (openings_left_out, e) {
            own.iter().for_each(|r| add(r));
            outline.extend(own.iter().cloned());
        }
        Some(Chart { element, face, surface, outline, openings, pieces, period })
    }

    /// The faces element `element` has, as [`Model::face_chart`] resolves them: a wall's ends only where no other wall is
    /// joined to it there, hiding them.
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
        let fixed: Vec<FaceId> = match (e, self.elements.id(element)) {
            (Element::Wall(_), Some(id)) => {
                let mut f = vec![FaceId::Exterior, FaceId::Interior];
                if !walls::joined_at(self, id, false, None) {
                    f.push(FaceId::Start);
                }
                if !walls::joined_at(self, id, true, None) {
                    f.push(FaceId::End);
                }
                f.extend([FaceId::Top, FaceId::Bottom]);
                f
            }
            _ => vec![FaceId::Top, FaceId::Bottom],
        };
        let sides = (0..sides).filter_map(|i| u32::try_from(i).ok()).map(FaceId::Side);
        fixed.into_iter().chain(sides).filter(|f| self.face_chart(element, *f).is_some()).collect()
    }

    /// The doors and windows in wall or slab `element` and the components on its faces that cut it, each as the solid it
    /// cuts (a door's or a window's own voids, where its family has them) or, for a door or a window without, its hole in
    /// the wall (`(s0, s1, bottom, top)`, along the location line and up). Found through the model's lookup of what points
    /// at what, not by looking at every element.
    #[allow(clippy::type_complexity)]
    fn cutters(&self, id: Id, element: Guid) -> (Vec<Vec<[P3; 3]>>, Vec<(f64, f64, f64, f64)>) {
        let lookup = self.lookup();
        let mut ids: Vec<Id> = lookup.referrers(&element).collect();
        ids.sort_unstable();
        ids.dedup();
        let (mut voids, mut holes) = (Vec::new(), Vec::new());
        let wall = self.wall(id);
        for r in ids {
            let Some(e @ (Element::Door(o) | Element::Window(o))) = self.elements.get(r) else { continue };
            if o.host != element {
                continue;
            }
            let own = self.opening_voids(e);
            if !own.is_empty() {
                voids.extend(own);
            } else if let Some(h) = wall.and_then(|w| walls::hole(self, w, e, o)) {
                holes.push((h.s0, h.s1, h.bottom, h.top));
            }
        }
        voids.extend(self.hosted_voids(id).into_iter().flat_map(|(_, v)| v));
        (voids, holes)
    }

    fn wall_face(&self, id: Id, element: Guid, w: &crate::model::Wall, face: FaceId) -> Option<Laid> {
        let axis = Axis::of(w);
        let len = axis.length();
        let (ext, int) = walls::faces(self, w);
        let z0 = self.own_base(w);
        let up: P3 = [0.0, 0.0, 1.0];
        let attached = w.attach_base.is_some() || w.attach_top.is_some();
        // Where along the location line it is sampled: its stations, more where what it is attached to may bend.
        let stations = |s0: f64, s1: f64| -> Vec<f64> {
            if attached { (0..=32).map(|i| s0 + (s1 - s0) * f64::from(i) / 32.0).collect() } else { axis.stations(s0, s1) }
        };
        match face {
            FaceId::Exterior | FaceId::Interior => {
                let exterior = face == FaceId::Exterior;
                let off = if exterior { ext } else { int };
                let k = match axis.arc() {
                    Some((_, r, _, sweep)) => {
                        let radius = r - sweep.signum() * off;
                        if radius.is_nan() || radius <= 1e-6 {
                            return None;
                        }
                        radius / r
                    }
                    None => 1.0,
                };
                let surface = Surface::Side { axis, off, z0, out: if exterior { 1.0 } else { -1.0 }, k };
                let at = stations(0.0, len);
                let mut ring: Vec<P2> = at.iter().map(|s| P2(*s, walls::base_at_offset(self, w, *s, off) - z0)).collect();
                ring.extend(at.iter().rev().map(|s| P2(*s, walls::top_at_offset(self, w, *s, off) - z0)));
                let (voids, holes) = self.cutters(id, element);
                let mut openings: Vec<Vec<P2>> = holes.into_iter().map(|(s0, s1, b, t)| rect(s0, s1, b - z0, t - z0)).collect();
                openings.extend(voids.iter().filter_map(|v| void_on(&surface, v)));
                Some(Laid::new(surface, vec![ring], openings))
            }
            FaceId::Start | FaceId::End => {
                let at_end = face == FaceId::End;
                let s = if at_end { len } else { 0.0 };
                let t = axis.tangent(s);
                let o = axis.at(s, ext);
                let across = t.perp() * -1.0;
                let n = if at_end { t } else { t * -1.0 };
                let surface = Surface::Plane { origin: [o.0, o.1, z0], u: [across.0, across.1, 0.0], v: up, normal: [n.0, n.1, 0.0] };
                let width = ext - int;
                let cuts: Vec<f64> = if attached { (0..=8).map(|i| width * f64::from(i) / 8.0).collect() } else { vec![0.0, width] };
                let mut ring: Vec<P2> = cuts.iter().map(|d| P2(*d, walls::base_at_offset(self, w, s, ext - d) - z0)).collect();
                ring.extend(cuts.iter().rev().map(|d| P2(*d, walls::top_at_offset(self, w, s, ext - d) - z0)));
                Some(Laid::new(surface, vec![ring], Vec::new()))
            }
            FaceId::Top | FaceId::Bottom => {
                let top = face == FaceId::Top;
                let at = stations(0.0, len);
                let mut band: Vec<P2> = at.iter().map(|s| axis.at(*s, ext)).collect();
                band.extend(at.iter().rev().map(|s| axis.at(*s, int)));
                let pieces = walls::wall_cap(self, w, band, top);
                let outline = pieces.iter().map(|(r, _)| r.clone()).collect();
                Some(Laid::new(Surface::Plan { pieces, holes: Vec::new(), up: top }, outline, Vec::new()))
            }
            FaceId::Side(_) => None,
        }
    }

    fn slab_face(&self, id: Id, e: &Element, face: FaceId) -> Option<Laid> {
        let pieces = self.slab_pieces(e);
        let (boundary, own): (&[P2], &[Vec<P2>]) = match e {
            Element::Floor(f) => (&f.boundary, &f.openings),
            Element::Ceiling(c) => (&c.boundary, &c.openings),
            Element::Roof(r) => (&r.boundary, &r.openings),
            _ => return None,
        };
        match face {
            FaceId::Top | FaceId::Bottom => {
                let top = face == FaceId::Top;
                let planes: Vec<(Vec<P2>, Plane)> =
                    pieces.iter().map(|(r, under, over)| (r.clone(), if top { *over } else { *under })).collect();
                // A ceiling's one piece is all of its boundary; a floor's and a roof's leave their openings out.
                let ceiling = matches!(e, Element::Ceiling(_));
                let holes = if ceiling { own.to_vec() } else { Vec::new() };
                let surface = Surface::Plan { pieces: planes, holes, up: top };
                let outline: Vec<Vec<P2>> = pieces.iter().map(|(r, ..)| r.clone()).collect();
                let mut openings: Vec<Vec<P2>> = own.to_vec();
                // The shafts through it, which its pieces don't leave out.
                let mut shafts: Vec<Id> = Vec::new();
                for (r, under, over) in &pieces {
                    let rise = r.iter().map(|q| over.at(*q) - under.at(*q)).fold(0.0, f64::max);
                    shafts.extend(self.less_shafts(&under.lift(r), rise).1);
                }
                shafts.sort_unstable();
                shafts.dedup();
                openings.extend(shafts.iter().filter_map(|s| match self.elements.get(*s) {
                    Some(Element::Shaft(sh)) => Some(sh.boundary.clone()),
                    _ => None,
                }));
                let (voids, _) = self.cutters(id, self.guid(id));
                openings.extend(voids.iter().filter_map(|v| void_on(&surface, v)));
                // Its own openings are what a floor's or a roof's pieces leave out (the shafts and voids are cut from them
                // here): put back, so its pieces are all of the face.
                Some(Laid { openings_left_out: !ceiling, ..Laid::new(surface, outline, openings) })
            }
            FaceId::Side(i) => {
                let i = usize::try_from(i).ok()?;
                let (a, b) = (*boundary.get(i)?, *boundary.get((i + 1) % boundary.len())?);
                let height = |q: P2, top: bool| {
                    let (piece, _) = piece_at(&pieces, |p| &p.0, &[], q)?;
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
        let round = t.round && t.profile.is_none();
        let outline = if round {
            (0..ROUND).map(|i| col.at + crate::model::heading(360.0 * i as f64 / ROUND as f64) * (t.width / 2.0)).collect()
        } else {
            self.column_outline(e)
        };
        match face {
            FaceId::Top | FaceId::Bottom => {
                let top = face == FaceId::Top;
                let pieces = walls::column_cap(self, col, outline, top);
                let rings = pieces.iter().map(|(r, _)| r.clone()).collect();
                // A hollow section's hole is no part of its top: as an opening, it measures what an outline over it takes.
                let holes = self.column_holes(e);
                Some(Laid::new(Surface::Plan { pieces, holes: holes.clone(), up: top }, rings, holes))
            }
            // A round column's side is one curved face, round from its angle counter-clockwise.
            FaceId::Side(0) if round => {
                let radius = t.width / 2.0;
                if radius.is_nan() || radius <= 1e-6 {
                    return None;
                }
                let a0 = col.angle.to_radians();
                let surface = Surface::Round { centre: col.at, radius, a0, z0: 0.0 };
                let n = 64;
                let at: Vec<f64> = (0..=n).map(|i| TAU * radius * f64::from(i) / f64::from(n)).collect();
                let edge = |u: f64| {
                    let a = a0 + u / radius;
                    walls::column_base_top_at(self, col, col.at + P2(a.cos(), a.sin()) * radius)
                };
                let z0 = at.iter().map(|u| edge(*u).0).fold(f64::INFINITY, f64::min);
                let mut ring: Vec<P2> = at.iter().map(|u| P2(*u, edge(*u).0 - z0)).collect();
                ring.extend(at.iter().rev().map(|u| P2(*u, edge(*u).1 - z0)));
                let surface = match surface {
                    Surface::Round { centre, radius, a0, .. } => Surface::Round { centre, radius, a0, z0 },
                    s => s,
                };
                Some(Laid { period: Some(TAU * radius), ..Laid::new(surface, vec![ring], Vec::new()) })
            }
            FaceId::Side(_) if round => None,
            FaceId::Side(i) => {
                let i = usize::try_from(i).ok()?;
                let (a, b) = (*outline.get(i)?, *outline.get((i + 1) % outline.len())?);
                let (ba, ta) = walls::column_base_top_at(self, col, a);
                let (bb, tb) = walls::column_base_top_at(self, col, b);
                side_face(a, b, geom::area(&outline) >= 0.0, [ba, bb, ta, tb])
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
    if !z0.is_finite() {
        return None;
    }
    let surface = Surface::Plane { origin: [a.0, a.1, z0], u: [u.0, u.1, 0.0], v: [0.0, 0.0, 1.0], normal: [out.0, out.1, 0.0] };
    let ring = vec![P2(0.0, ba - z0), P2(len, bb - z0), P2(len, tb - z0), P2(0.0, ta - z0)];
    Some(Laid::new(surface, vec![ring], Vec::new()))
}
