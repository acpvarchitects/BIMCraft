//! Shaped walls (HBIM, option A: batter): each face of a wall may lean or curve up its height while the wall keeps its location
//! line, type, height, joins, openings and room bounding. A face's shape is an offset along its outward normal at each
//! height above the wall's base; only the variable layer (the core) takes up the change, its finish layers keeping their
//! thickness. The solid is the unshaped one (its prisms, caps and openings) deformed: a point on a face or a layer's
//! boundary moves square to the wall by that offset at its height, and a point on a cap slides along the cap, so a join
//! with a straight neighbour keeps sharing its mitre with no gap.

use serde::{Deserialize, Serialize};

use super::{Axis, Plane};
use crate::geom::{self, P2};
use crate::model::{Function, Id, Layer, MAX_COORD, Model, Wall};

/// The most points a face's profile may list.
pub const MAX_PROFILE: usize = 64;
/// The steepest batter, degrees from plumb either way.
pub const MAX_BATTER: f64 = 80.0;

/// A wall's two faces' shapes: none unless given, and a file holds none unless one is.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallShape {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exterior: Option<FaceShape>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interior: Option<FaceShape>,
}

impl WallShape {
    pub fn is_empty(&self) -> bool {
        self.exterior.is_none() && self.interior.is_none()
    }

    pub fn face(&self, face: Face) -> &Option<FaceShape> {
        match face {
            Face::Exterior => &self.exterior,
            Face::Interior => &self.interior,
        }
    }

    pub fn face_mut(&mut self, face: Face) -> &mut Option<FaceShape> {
        match face {
            Face::Exterior => &mut self.exterior,
            Face::Interior => &mut self.interior,
        }
    }
}

/// One of a wall's two faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Face {
    Exterior,
    Interior,
}

impl Face {
    pub fn name(self) -> &'static str {
        match self {
            Face::Exterior => "exterior",
            Face::Interior => "interior",
        }
    }
}

/// How a face departs from plumb. One kind for now; points and split lines on a face (flat triangles) come as another.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum FaceShape {
    Batter(Batter),
}

impl FaceShape {
    /// How far the face stands out (negative: in) along its outward normal `h` mm above the wall's base, of a wall
    /// `height` mm tall by its own constraints.
    pub fn offset(&self, h: f64, height: f64) -> f64 {
        match self {
            FaceShape::Batter(b) => b.offset(h, height),
        }
    }

    /// The heights over the base where its offset bends.
    pub fn knots(&self) -> Vec<f64> {
        match self {
            FaceShape::Batter(b) => b.profile.iter().map(|p| p[0]).collect(),
        }
    }
}

/// A battered face: by one of an `angle` from plumb (degrees, positive leaning out as it rises), an offset at the wall's
/// `top` (mm along the outward normal, in between in proportion to the height), or a `profile` of `[height, offset]`
/// points (mm over the base), straight between them from no offset at the base and plumb past the last: a scarp is one
/// point, a curved batter several.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profile: Vec<[f64; 2]>,
}

impl Batter {
    pub fn offset(&self, h: f64, height: f64) -> f64 {
        if let Some(a) = self.angle {
            return h * a.to_radians().tan();
        }
        if let Some(t) = self.top {
            return if height > 0.0 { t * h / height } else { 0.0 };
        }
        let mut prev = [0.0, 0.0];
        for p in &self.profile {
            if h <= p[0] {
                let run = p[0] - prev[0];
                return if run > 0.0 { prev[1] + (p[1] - prev[1]) * (h - prev[0]) / run } else { p[1] };
            }
            prev = *p;
        }
        prev[1]
    }

    /// Why it can't be, if it can't: one of its three ways given, each finite and in range.
    pub fn check(&self) -> Result<(), String> {
        let given = usize::from(self.angle.is_some()) + usize::from(self.top.is_some()) + usize::from(!self.profile.is_empty());
        if given != 1 {
            return Err("a batter takes one of `angle`, `top` or `profile`".into());
        }
        if let Some(a) = self.angle
            && !(a.is_finite() && a.abs() <= MAX_BATTER)
        {
            return Err(format!("batter angle {a}° isn't between -{MAX_BATTER}° and {MAX_BATTER}° from plumb"));
        }
        if let Some(t) = self.top
            && !(t.is_finite() && t.abs() <= MAX_COORD)
        {
            return Err(format!("batter top offset {t} isn't a length in mm"));
        }
        if self.profile.len() > MAX_PROFILE {
            return Err(format!("a batter profile has {} points, more than {MAX_PROFILE}", self.profile.len()));
        }
        let mut below = 0.0;
        for p in &self.profile {
            if !(p.iter().all(|x| x.is_finite() && x.abs() <= MAX_COORD) && p[0] > below) {
                return Err(format!(
                    "batter profile point [{}, {}]: heights must rise from above 0, each over the last, offsets be lengths in mm",
                    p[0], p[1]
                ));
            }
            below = p[0];
        }
        Ok(())
    }
}

/// Which band of a wall's boundaries takes up a shaped face's change: the thickest structure layer, else the thickest
/// layer, else (no layers) the whole wall.
pub fn variable_layer(layers: &[Layer]) -> usize {
    let thickest = |pick: &dyn Fn(&Layer) -> bool| {
        layers
            .iter()
            .enumerate()
            .filter(|(_, l)| pick(l))
            .max_by(|(i, a), (j, b)| a.thickness.total_cmp(&b.thickness).then(j.cmp(i)))
            .map(|(k, _)| k)
    };
    thickest(&|l| l.function == Function::Structure).or_else(|| thickest(&|_| true)).unwrap_or(0)
}

/// A shaped wall's deformation: where a point of its unshaped solid goes at a height.
pub(crate) struct Shaper {
    axis: Axis,
    base: f64,
    height: f64,
    exterior: Option<FaceShape>,
    interior: Option<FaceShape>,
    /// Its faces' and layers' boundaries, mm to the left of its location line, exterior first.
    bounds: Vec<f64>,
    /// The band of `bounds` that stretches.
    var: usize,
    /// Its caps, start and end: points on them slide along them.
    caps: [Vec<P2>; 2],
}

impl Shaper {
    /// Wall `w`'s, if it is shaped; its caps not yet known (a point slides square to it).
    pub(crate) fn of(m: &Model, w: &Wall) -> Option<Shaper> {
        if w.shape.is_empty() {
            return None;
        }
        let layers = m.type_layers("wall", &w.type_name);
        let base = m.own_base(w);
        Some(Shaper {
            axis: Axis::of(w),
            base,
            height: m.own_top(w) - base,
            exterior: w.shape.exterior.clone(),
            interior: w.shape.interior.clone(),
            bounds: super::boundaries(m, w),
            var: if layers.is_empty() { 0 } else { variable_layer(layers) },
            caps: [Vec::new(), Vec::new()],
        })
    }

    /// Wall `id`'s, with its caps, if it is shaped.
    pub(crate) fn of_id(m: &Model, id: Id, index: Option<&super::Index>) -> Option<Shaper> {
        let mut s = Shaper::of(m, m.wall(id)?)?;
        s.caps = [super::cap_in(m, id, false, index), super::cap_in(m, id, true, index)];
        Some(s)
    }

    /// How far the exterior and the interior face stand out at height `z`, each along its own outward normal.
    pub(crate) fn shifts(&self, z: f64) -> (f64, f64) {
        let h = z - self.base;
        let at = |f: &Option<FaceShape>| f.as_ref().map_or(0.0, |f| f.offset(h, self.height));
        (at(&self.exterior), at(&self.interior))
    }

    /// The heights where either face's offset bends, absolute.
    pub(crate) fn knots(&self) -> Vec<f64> {
        let mut out: Vec<f64> =
            [&self.exterior, &self.interior].into_iter().flatten().flat_map(FaceShape::knots).map(|h| self.base + h).collect();
        out.sort_by(f64::total_cmp);
        out.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        out
    }

    /// Where the line `off` mm to the left of the location line lies at height `z`.
    pub(crate) fn offset(&self, off: f64, z: f64) -> f64 {
        let (de, di) = self.shifts(z);
        let (Some(&a), Some(&b)) = (self.bounds.get(self.var), self.bounds.get(self.var + 1)) else { return off + de };
        if off >= a - 1e-9 {
            off + de
        } else if off <= b + 1e-9 {
            off - di
        } else {
            let t = (a - off) / (a - b);
            off + de * (1.0 - t) - di * t
        }
    }

    /// Where the point `p` of the unshaped wall in plan lies at height `z`: moved square to the wall, or along its cap.
    pub(crate) fn point(&self, p: P2, z: f64) -> P2 {
        let (s, off) = self.axis.project(p);
        let to = self.offset(off, z);
        if (to - off).abs() < 1e-9 {
            return p;
        }
        for cap in &self.caps {
            let n = cap.len();
            if n < 2 || !cap.windows(2).any(|w| geom::dist_to_segment(p, w[0], w[1]) <= 1e-6) {
                continue;
            }
            // A corner inside the cap is the node its neighbours share: it stays.
            if cap[1..n - 1].iter().any(|c| c.dist(p) <= 1e-6) {
                return p;
            }
            let (a, t) = (self.axis.at(s, to), self.axis.tangent(s));
            let mut best: Option<P2> = None;
            for (i, w) in cap.windows(2).enumerate() {
                let (c, d) = (w[0], w[1]);
                let Some(x) = geom::line_intersection(a, a + t, c, d) else { continue };
                let len2 = (d - c).dot(d - c);
                let along = if len2 > 0.0 { (x - c).dot(d - c) / len2 } else { 0.0 };
                // The cap's first and last edges run on past its corners: a face leaning out goes past them.
                if (along >= -1e-9 || i == 0) && (along <= 1.0 + 1e-9 || i == n - 2) && best.is_none_or(|b| x.dist(p) < b.dist(p)) {
                    best = Some(x);
                }
            }
            if let Some(x) = best {
                return x;
            }
        }
        self.axis.at(s, to)
    }

    /// A ring or a line in plan as it lies at height `z`.
    pub(crate) fn ring(&self, ring: &[P2], z: f64) -> Vec<P2> {
        ring.iter().map(|p| self.point(*p, z)).collect()
    }

    /// The prism `ring` from `bottom` up to `top` as closed triangles, outward: each corner's edge bent at the knots.
    pub(crate) fn mesh(&self, ring: &[P2], bottom: Plane, top: Plane) -> Vec<[[f64; 3]; 3]> {
        let mut ring: Vec<P2> = ring.to_vec();
        ring.dedup_by(|a, b| a.dist(*b) < 1e-9);
        if ring.len() > 1 && ring.first().zip(ring.last()).is_some_and(|(a, b)| a.dist(*b) < 1e-9) {
            ring.pop();
        }
        if ring.len() < 3 {
            return Vec::new();
        }
        if geom::area(&ring) < 0.0 {
            ring.reverse();
        }
        let knots = self.knots();
        let lift = |p: P2, z: f64| {
            let q = self.point(p, z);
            [q.0, q.1, z]
        };
        let columns: Vec<Vec<[f64; 3]>> = ring
            .iter()
            .map(|p| {
                let (z0, z1) = (bottom.at(*p), top.at(*p));
                let mut zs = vec![z0];
                zs.extend(knots.iter().copied().filter(|k| *k > z0 + 1e-6 && *k < z1 - 1e-6));
                if z1 > z0 + 1e-6 {
                    zs.push(z1);
                }
                zs.into_iter().map(|z| lift(*p, z)).collect()
            })
            .collect();
        let mut out = Vec::new();
        let mut push = |t: [[f64; 3]; 3]| {
            let (u, v) = (sub(t[1], t[0]), sub(t[2], t[0]));
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            if n.iter().map(|x| x * x).sum::<f64>() > 1e-12 {
                out.push(t);
            }
        };
        let n = ring.len();
        for i in 0..n {
            let (a, b) = (&columns[i], &columns[(i + 1) % n]);
            let (mut x, mut y) = (0, 0);
            while x + 1 < a.len() || y + 1 < b.len() {
                let up_a = x + 1 < a.len() && (y + 1 >= b.len() || a[x + 1][2] <= b[y + 1][2]);
                if up_a {
                    push([a[x], b[y], a[x + 1]]);
                    x += 1;
                } else {
                    push([a[x], b[y], b[y + 1]]);
                    y += 1;
                }
            }
        }
        for t in geom::triangulate(&ring) {
            let [i, j, k] = t;
            let (Some(ci), Some(cj), Some(ck)) = (columns.get(i), columns.get(j), columns.get(k)) else { continue };
            let ccw = (ring[j] - ring[i]).cross(ring[k] - ring[i]) > 0.0;
            let (lo, hi) = (|c: &Vec<[f64; 3]>| c[0], |c: &Vec<[f64; 3]>| c[c.len() - 1]);
            if ccw {
                push([hi(ci), hi(cj), hi(ck)]);
                push([lo(ci), lo(ck), lo(cj)]);
            } else {
                push([hi(ci), hi(ck), hi(cj)]);
                push([lo(ci), lo(cj), lo(ck)]);
            }
        }
        out
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The volume closed triangles hold, mm³ (outward, positive).
pub fn volume(tris: &[[[f64; 3]; 3]]) -> f64 {
    tris.iter()
        .map(|[a, b, c]| {
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0
        })
        .sum()
}

/// Wall `w`'s thickness up its height, thinnest, thickest and on average (mm): its type's for an unshaped wall.
pub fn thickness_range(m: &Model, w: &Wall) -> (f64, f64, f64) {
    let t = m.thickness(w);
    let Some(s) = Shaper::of(m, w) else { return (t, t, t) };
    let (lo, hi) = (m.base_z(w), m.top_z(w));
    let at = |z: f64| {
        let (de, di) = s.shifts(z);
        t + de + di
    };
    let mut zs = vec![lo];
    zs.extend(s.knots().into_iter().filter(|k| *k > lo && *k < hi));
    zs.push(hi);
    let (mut min, mut max, mut area) = (f64::INFINITY, f64::NEG_INFINITY, 0.0);
    for pair in zs.windows(2) {
        let (a, b) = (at(pair[0]), at(pair[1]));
        min = min.min(a).min(b);
        max = max.max(a).max(b);
        area += (a + b) / 2.0 * (pair[1] - pair[0]);
    }
    let avg = if hi > lo { area / (hi - lo) } else { t };
    (min, max, avg)
}

/// How far either face of wall `w` stands out or in at most, mm, up its height: 0 unshaped.
pub fn reach(m: &Model, w: &Wall) -> f64 {
    let Some(s) = Shaper::of(m, w) else { return 0.0 };
    let (lo, hi) = (m.base_z(w), m.top_z(w));
    let mut zs = vec![lo, hi];
    zs.extend(s.knots().into_iter().filter(|k| *k > lo && *k < hi));
    zs.into_iter().map(|z| s.shifts(z)).map(|(de, di)| de.abs().max(di.abs())).fold(0.0, f64::max)
}

/// Why wall `w`'s shape can't be, if it can't: each face's batter in range, and its variable layer no thinner than nothing
/// anywhere up its height (its faces never crossing), openings included, which lie within it.
pub(crate) fn check(m: &Model, who: &str, w: &Wall) -> Result<(), String> {
    for face in [Face::Exterior, Face::Interior] {
        if let Some(FaceShape::Batter(b)) = w.shape.face(face) {
            b.check().map_err(|e| format!("{who}: its {} face: {e}", face.name()))?;
        }
    }
    let Some(s) = Shaper::of(m, w) else { return Ok(()) };
    let layers = m.type_layers("wall", &w.type_name);
    let (a, b) = (s.bounds.get(s.var).copied().unwrap_or(0.0), s.bounds.get(s.var + 1).copied().unwrap_or(0.0));
    let thick = a - b;
    let (lo, hi) = (m.base_z(w), m.top_z(w));
    let mut zs = vec![lo, hi];
    zs.extend(s.knots().into_iter().filter(|k| *k > lo && *k < hi));
    for z in zs {
        let (de, di) = s.shifts(z);
        let left = thick + de + di;
        if left < -1e-6 {
            let what = match layers.get(s.var) {
                Some(l) => format!("its layer {} ({} mm)", s.var + 1, l.thickness),
                None => format!("it ({thick} mm)"),
            };
            return Err(format!(
                "{who}: its faces' batter leaves {what} {:.1} mm thick {:.0} mm over its base: lessen the batter or thicken the wall",
                left,
                z - s.base
            ));
        }
    }
    Ok(())
}
