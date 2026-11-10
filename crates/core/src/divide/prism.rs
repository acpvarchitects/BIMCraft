//! A part that is a prism, as a wall's layer is: a face's outline, with its openings, and a thickness behind it. Lines
//! drawn on its face that cut through it divide the outline in the plane, and each region is raised into its piece:
//! exact, and closed by construction. Whatever else a recipe asks goes the general way, on the prism's surface.

use serde::{Deserialize, Serialize};

use super::run::Shape;
use super::{
    Axes, Conflict, Cut, DivideError, Frame, MAX_CUTS, MAX_PIECES, MIN_VOLUME, Mesh, Outcome, Piece, PieceKey, Recipe, Stage, Step, THIN,
    Work, divide, flat, mesh,
};
use crate::geom::{self, P2, Region};
use crate::model::MAX_COORD;
use crate::view3d::dot;

/// How near, mm, points of the plane are one where an outline is divided.
const NEAR: f64 = 1e-6;
/// The most openings a prism has.
pub const MAX_HOLES: usize = 256;

/// A prism: `outline` and its `holes` (openings) in `frame`'s coordinates, on the part's face, and `thickness` mm of
/// solid behind it, against the frame's normal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prism {
    pub frame: Frame,
    pub outline: Vec<P2>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<P2>>,
    pub thickness: f64,
}

impl Prism {
    /// Its axes, its outline and its holes tidied, or what to fix.
    fn checked(&self) -> Result<(Axes, Region), DivideError> {
        let bad = |what: String| DivideError::Input(format!("the prism: {what}"));
        let axes = self.frame.axes().map_err(bad)?;
        if !(self.thickness.is_finite() && self.thickness >= THIN && self.thickness <= MAX_COORD) {
            return Err(bad("its thickness must be a length above zero".into()));
        }
        if self.holes.len() > MAX_HOLES {
            return Err(bad(format!("it has {} openings; it takes {MAX_HOLES} at most", self.holes.len())));
        }
        let outer = flat::clean(&self.outline, true).map_err(bad)?;
        let mut holes = Vec::with_capacity(self.holes.len());
        for h in &self.holes {
            let hole = flat::clean(h, true).map_err(bad)?;
            let apart =
                holes.iter().all(|o: &Vec<P2>| !hole.iter().any(|p| geom::contains(o, *p)) && !o.iter().any(|p| geom::contains(&hole, *p)));
            if !hole.iter().all(|p| geom::contains(&outer, *p)) || !apart {
                return Err(bad("an opening reaches outside its outline, or two openings overlap".into()));
            }
            holes.push(hole);
        }
        Ok((axes, Region { outer, holes }))
    }

    /// Its surface: closed, its triangles turned outward.
    pub fn mesh(&self) -> Result<Mesh, DivideError> {
        let (axes, region) = self.checked()?;
        raised(&axes, &region, self.thickness, &mut Work::new())
    }
}

/// A region of the face raised into a solid `thickness` deep behind it.
fn raised(axes: &Axes, region: &Region, thickness: f64, work: &mut Work) -> Result<Mesh, DivideError> {
    let (pts, tris) = flat::patch(axes, &region.outer, &region.holes, work)?;
    Ok(flat::slab(&pts, &tris, axes.n, 0.0, -thickness))
}

/// A region's area and the centre of it.
fn measure(region: &Region) -> (f64, P2) {
    let rings = std::iter::once(&region.outer).chain(&region.holes);
    let (mut whole, mut sum) = (0.0, P2(0.0, 0.0));
    for (k, ring) in rings.enumerate() {
        let a = if k == 0 { geom::area(ring).abs() } else { -geom::area(ring).abs() };
        whole += a;
        sum = sum + geom::centroid(ring) * a;
    }
    (whole, if whole.abs() > 1e-9 { sum * (1.0 / whole) } else { geom::centroid(&region.outer) })
}

/// A point inside a ring that runs counter-clockwise: just to the left of the middle of one of its edges. So it is
/// for a ring that walks an edge both ways, too: a line that ends in an opening joins the opening to the outline.
fn within(ring: &[P2]) -> Option<P2> {
    [1e-2, 1e-4].iter().find_map(|&off| {
        geom::ring_edges(ring)
            .filter(|(p, q)| p.dist(*q) > 4.0 * off)
            .map(|(p, q)| (p + q) * 0.5 + (q - p).unit().perp() * off)
            .find(|m| geom::contains(ring, *m))
    })
}

/// A region with the centre it is counted by.
type Placed = (P2, Region);

/// What a line makes of one region, in the line's own frame: the regions on its first side and on its second, each in
/// the order their keys count them, or why it leaves the region whole.
fn split(region: &Region, shape: &Shape, thickness: f64) -> Result<(Vec<Region>, Vec<Region>), Conflict> {
    let (line, closed): (Vec<P2>, bool) = match shape {
        Shape::Line { line, .. } => (flat::reaching(line), false),
        Shape::Ring { ring, .. } => (ring.clone(), true),
        Shape::Plane { .. } => return Err(Conflict::Misses),
    };
    let around = |ring: &[P2]| geom::ring_edges(ring).collect::<Vec<_>>();
    let mut segs: Vec<(P2, P2)> = std::iter::once(&region.outer).chain(&region.holes).flat_map(|r| around(r)).collect();
    if closed {
        segs.extend(around(&line));
    } else {
        segs.extend(line.windows(2).map(|w| (w[0], w[1])));
    }
    let all = geom::faces(&segs, NEAR);
    let inside = |p: P2| geom::contains(&region.outer, p) && !region.holes.iter().any(|h| geom::contains(h, p));
    let (mut a, mut b): (Vec<Placed>, Vec<Placed>) = (Vec::new(), Vec::new());
    for face in all.iter().filter(|f| geom::area(f) > NEAR) {
        let Some(probe) = within(face).filter(|p| inside(*p)) else { continue };
        let Some(part) = geom::region_in(&all, probe, NEAR) else { continue };
        let (held, centre) = measure(&part);
        let around = geom::perimeter(&part.outer) + part.holes.iter().map(|h| geom::perimeter(h)).sum::<f64>();
        if held * thickness < MIN_VOLUME || 2.0 * held * thickness < THIN * (2.0 * held + around * thickness) {
            continue;
        }
        let first = if closed {
            geom::contains(&line, probe)
        } else {
            // The face is on the left of each of its edges: one that runs the way the line does has the line's left.
            let (mut left, mut right) = (false, false);
            for (p, q) in geom::ring_edges(face) {
                let mid = (p + q) * 0.5;
                for s in line.windows(2) {
                    if geom::dist_to_segment(mid, s[0], s[1]) <= 1e-5 && geom::dist_to_segment(p, s[0], s[1]) <= 1e-5 {
                        *(if (q - p).dot(s[1] - s[0]) > 0.0 { &mut left } else { &mut right }) = true;
                    }
                }
            }
            match (left, right) {
                (true, true) => return Err(Conflict::DoesNotCross),
                (true, false) => true,
                (false, true) => false,
                (false, false) => {
                    let nearest = line
                        .windows(2)
                        .min_by(|s, t| geom::dist_to_segment(probe, s[0], s[1]).total_cmp(&geom::dist_to_segment(probe, t[0], t[1])));
                    nearest.is_none_or(|s| (s[1] - s[0]).cross(probe - s[0]) >= 0.0)
                }
            }
        };
        (if first { &mut a } else { &mut b }).push((centre, part));
    }
    let order = |v: &mut Vec<Placed>| {
        v.sort_by(|(p, _), (q, _)| {
            (p.0.round().total_cmp(&q.0.round()))
                .then(p.1.round().total_cmp(&q.1.round()))
                .then(p.0.total_cmp(&q.0))
                .then(p.1.total_cmp(&q.1))
        });
    };
    order(&mut a);
    order(&mut b);
    let none = if closed { [Conflict::Misses, Conflict::HoldsAll] } else { [Conflict::Misses; 2] };
    if a.is_empty() {
        return Err(none[0]);
    }
    if b.is_empty() {
        return Err(none[1]);
    }
    Ok((a.into_iter().map(|x| x.1).collect(), b.into_iter().map(|x| x.1).collect()))
}

/// [`divide`] for a prism. A recipe of lines drawn on its face, or on a plane level with it, that cut through it with
/// no gap is done in the plane: the outline is divided and each region raised, exact and closed. Any other recipe is
/// done by [`divide`] on the prism's surface. The pieces have the same keys either way.
pub fn divide_prism(prism: &Prism, recipe: &Recipe) -> Result<Outcome, DivideError> {
    let (axes, whole) = prism.checked()?;
    let level = |frame: &Frame| frame.axes().is_ok_and(|a| dot(a.n, axes.n).abs() > 1.0 - 1e-9);
    let in_plane = recipe.offsets.is_empty()
        && recipe.cuts.iter().all(|d| d.gap == 0.0 && matches!(&d.cut, Cut::Sketch { frame, depth: None, .. } if level(frame)));
    if !in_plane {
        return divide(&prism.mesh()?, recipe);
    }
    if recipe.cuts.len() > MAX_CUTS {
        return Err(DivideError::Input(format!("the recipe has {} cuts; it takes {MAX_CUTS} at most", recipe.cuts.len())));
    }
    let mut work = Work::new();
    let held = measure(&whole).0 * prism.thickness;
    let mut parts: Vec<(PieceKey, Region)> = vec![(PieceKey::default(), whole)];
    for (k, d) in recipe.cuts.iter().enumerate() {
        let shape = Shape::of(&d.cut).map_err(|e| DivideError::Input(format!("cut {k}: {e}")))?;
        if d.of.as_ref().is_some_and(|key| key.0.len() > MAX_CUTS) {
            return Err(DivideError::Input(format!("cut {k}: the piece it names has more steps than a recipe has cuts")));
        }
        // Between the face's coordinates and the line's: the same plane, seen from either side.
        let theirs = *shape.axes();
        let there = |r: &[P2]| r.iter().map(|q| theirs.local(axes.point(*q, 0.0)).0).collect::<Vec<P2>>();
        let back = |r: &[P2]| r.iter().map(|q| axes.local(theirs.point(*q, 0.0)).0).collect::<Vec<P2>>();
        let sides = d.cut.sides();
        let mut next = Vec::with_capacity(parts.len() + 1);
        let (mut met, mut any, mut why) = (false, false, Conflict::Misses);
        for (key, region) in &parts {
            if d.of.as_ref().is_some_and(|of| !key.is_of(of)) {
                next.push((key.clone(), region.clone()));
                continue;
            }
            met = true;
            work.spend(region.outer.len() + region.holes.iter().map(Vec::len).sum::<usize>())?;
            let seen = Region { outer: there(&region.outer), holes: region.holes.iter().map(|h| there(h)).collect() };
            match split(&seen, &shape, prism.thickness) {
                Ok((a, b)) => {
                    any = true;
                    for (side, regions) in [(sides[0], a), (sides[1], b)] {
                        for (n, r) in regions.into_iter().enumerate() {
                            let mut key = key.clone();
                            key.0.push(Step { cut: k as u32, side, lump: n as u32 });
                            next.push((key, Region { outer: back(&r.outer), holes: r.holes.iter().map(|h| back(h)).collect() }));
                        }
                    }
                }
                Err(conflict) => {
                    if conflict == Conflict::DoesNotCross || conflict == Conflict::HoldsAll {
                        why = conflict;
                    }
                    if conflict == Conflict::DoesNotCross {
                        return Err(DivideError::Conflict { at: Stage::Cut(k), conflict });
                    }
                    next.push((key.clone(), region.clone()));
                }
            }
        }
        if !met || !any {
            return Err(DivideError::Conflict { at: Stage::Cut(k), conflict: if met { why } else { Conflict::NoSuchPiece } });
        }
        parts = next;
        if parts.len() > MAX_PIECES {
            return Err(DivideError::TooMuch(format!("the recipe makes more than {MAX_PIECES} pieces: give it fewer cuts")));
        }
    }
    let mut pieces = Vec::with_capacity(parts.len());
    for (key, region) in parts {
        let surface = raised(&axes, &region, prism.thickness, &mut work)?;
        pieces.push(Piece { key, volume: mesh::volume(&surface), closed: mesh::is_closed(&surface), mesh: surface });
    }
    let removed = held - pieces.iter().map(|p| p.volume).sum::<f64>();
    Ok(Outcome { pieces, removed, conflicts: Vec::new() })
}
