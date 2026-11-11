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
/// How far to either side of a line, mm, the faces it parts are looked for.
const BESIDE: f64 = 1e-3;
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

/// One face a line leaves of a region, and the side of the line it lies on: its first, its second, or neither where the
/// line never touched it.
struct Face {
    region: Region,
    first: Option<bool>,
}

/// The faces a line leaves of one region, in the line's own frame; or that the line doesn't cross it.
fn faces(region: &Region, line: &[P2], closed: bool) -> Result<Vec<Face>, Conflict> {
    let around = |ring: &[P2]| geom::ring_edges(ring).collect::<Vec<_>>();
    let rims: Vec<(P2, P2)> = std::iter::once(&region.outer).chain(&region.holes).flat_map(|r| around(r)).collect();
    let mut segs = rims.clone();
    if closed {
        segs.extend(around(line));
    } else {
        segs.extend(line.windows(2).map(|w| (w[0], w[1])));
    }
    let all = geom::faces(&segs, NEAR);
    let inside = |p: P2| geom::contains(&region.outer, p) && !region.holes.iter().any(|h| geom::contains(h, p));
    // The face a point is in: the smallest that holds it.
    let face_at = |p: P2| {
        let holding = all.iter().enumerate().filter(|(_, f)| geom::area(f) > NEAR && geom::contains(f, p));
        holding.min_by(|x, y| geom::area(x.1).total_cmp(&geom::area(y.1))).map(|(i, _)| i)
    };
    // Which faces the line has on its left, and on its right: wherever a stretch of it runs inside the region, the face
    // just to either side of it. The same face on both sides was not cut through there: the line stops short in it, or
    // ends in an opening it closes round.
    let mut sides: Vec<(bool, bool)> = vec![(false, false); all.len()];
    if !closed {
        for s in line.windows(2) {
            let (from, to) = (s[0], s[1]);
            let mut stops = vec![0.0, 1.0];
            for &(p, q) in &rims {
                stops.extend(geom::crossing(from, to, p, q));
                stops.extend(
                    [p, q].iter().filter(|e| geom::dist_to_segment(**e, from, to) <= NEAR).map(|e| geom::param_on_segment(*e, from, to)),
                );
            }
            stops.sort_by(f64::total_cmp);
            for w in stops.windows(2) {
                let mid = from + (to - from) * ((w[0] + w[1]) / 2.0);
                if (w[1] - w[0]) * from.dist(to) <= 4.0 * BESIDE || !inside(mid) {
                    continue;
                }
                let off = (to - from).unit().perp() * BESIDE;
                let (left, right) =
                    (face_at(mid + off).filter(|_| inside(mid + off)), face_at(mid + off * -1.0).filter(|_| inside(mid + off * -1.0)));
                if left.is_some() && left == right {
                    return Err(Conflict::DoesNotCross);
                }
                if let Some(side) = left.and_then(|i| sides.get_mut(i)) {
                    side.0 = true;
                }
                if let Some(side) = right.and_then(|i| sides.get_mut(i)) {
                    side.1 = true;
                }
            }
        }
    }
    let mut out = Vec::new();
    for (i, face) in all.iter().enumerate().filter(|(_, f)| geom::area(f) > NEAR) {
        let Some(probe) = within(face).filter(|p| inside(*p)) else { continue };
        let Some(part) = geom::region_in(&all, probe, NEAR) else { continue };
        let first = if closed {
            Some(geom::contains(line, probe))
        } else {
            match sides.get(i).copied().unwrap_or_default() {
                (true, true) => return Err(Conflict::DoesNotCross),
                (true, false) => Some(true),
                (false, true) => Some(false),
                (false, false) => None,
            }
        };
        out.push(Face { region: part, first });
    }
    Ok(out)
}

/// A lump with the centre it is counted by.
type Placed = (P2, Vec<Region>);
/// The lumps on a line's first side, and on its second: each the regions it is made of.
type Sides = (Vec<Vec<Region>>, Vec<Vec<Region>>);

/// What a line makes of a piece's regions, in the line's own frame: the lumps on its first side and on its second, each
/// in the order their keys count them, or why it leaves the piece whole. Faces of one side that touch at a corner are
/// one lump, as their solids' surfaces would share that corner; a face the line never touched goes with what it
/// touches, or by itself to the side of the line it lies on.
fn split(regions: &[Region], shape: &Shape, thickness: f64, work: &mut Work) -> Result<Result<Sides, Conflict>, DivideError> {
    let (line, closed): (Vec<P2>, bool) = match shape {
        Shape::Line { line, .. } => (flat::reaching(line), false),
        Shape::Ring { ring, .. } => (ring.clone(), true),
        Shape::Plane { .. } => return Ok(Err(Conflict::Misses)),
    };
    let mut all: Vec<Face> = Vec::new();
    for region in regions {
        match faces(region, &line, closed) {
            Ok(found) => all.extend(found),
            Err(conflict) => return Ok(Err(conflict)),
        }
    }
    // The faces that touch: every corner with its face, in order across the plane, each joined to those beside it.
    let mut corners: Vec<(P2, usize)> = all
        .iter()
        .enumerate()
        .flat_map(|(i, f)| std::iter::once(&f.region.outer).chain(&f.region.holes).flatten().map(move |p| (*p, i)))
        .collect();
    corners.sort_by(|a, b| a.0.0.total_cmp(&b.0.0).then(a.0.1.total_cmp(&b.0.1)).then(a.1.cmp(&b.1)));
    let mut root: Vec<usize> = (0..all.len()).collect();
    fn find(root: &mut [usize], mut x: usize) -> usize {
        while let Some(&up) = root.get(x).filter(|&&up| up != x) {
            x = up;
        }
        x
    }
    for (n, &(p, i)) in corners.iter().enumerate() {
        for &(q, j) in corners.iter().skip(n + 1).take_while(|(q, _)| q.0 - p.0 <= NEAR) {
            work.spend(1)?;
            let sides = (all.get(i).and_then(|f| f.first), all.get(j).and_then(|f| f.first));
            let apart = matches!(sides, (Some(a), Some(b)) if a != b);
            if i != j && !apart && p.dist(q) <= NEAR {
                let (a, b) = (find(&mut root, i), find(&mut root, j));
                if let Some(slot) = root.get_mut(a.max(b)) {
                    *slot = a.min(b);
                }
            }
        }
    }
    let mut lumps: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    for i in 0..all.len() {
        lumps.entry(find(&mut root, i)).or_default().push(i);
    }
    let (mut a, mut b): (Vec<Placed>, Vec<Placed>) = (Vec::new(), Vec::new());
    for members in lumps.values() {
        let of = |i: &usize| all.get(*i);
        let on = |side: bool| members.iter().filter_map(of).any(|f| f.first == Some(side));
        let (mut held, mut rim, mut sum) = (0.0, 0.0, P2(0.0, 0.0));
        for f in members.iter().filter_map(of) {
            let (area, centre) = measure(&f.region);
            held += area;
            sum = sum + centre * area;
            rim += geom::perimeter(&f.region.outer) + f.region.holes.iter().map(|h| geom::perimeter(h)).sum::<f64>();
        }
        let centre = if held > 0.0 { sum * (1.0 / held) } else { sum };
        let first = match (on(true), on(false)) {
            // A face the line never touched, touching both sides of it at corners: the line runs through a pinch.
            (true, true) => return Ok(Err(Conflict::DoesNotCross)),
            (true, false) => true,
            (false, true) => false,
            (false, false) => {
                let nearest = line
                    .windows(2)
                    .min_by(|s, t| geom::dist_to_segment(centre, s[0], s[1]).total_cmp(&geom::dist_to_segment(centre, t[0], t[1])));
                nearest.is_none_or(|s| (s[1] - s[0]).cross(centre - s[0]) >= 0.0)
            }
        };
        if held * thickness < MIN_VOLUME || 2.0 * held * thickness < THIN * (2.0 * held + rim * thickness) {
            continue;
        }
        (if first { &mut a } else { &mut b }).push((centre, members.iter().filter_map(of).map(|f| f.region.clone()).collect()));
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
        return Ok(Err(none[0]));
    }
    if b.is_empty() {
        return Ok(Err(none[1]));
    }
    Ok(Ok((a.into_iter().map(|x| x.1).collect(), b.into_iter().map(|x| x.1).collect())))
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
    let mut parts: Vec<(PieceKey, Vec<Region>)> = vec![(PieceKey::default(), vec![whole])];
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
        for (key, regions) in &parts {
            if d.of.as_ref().is_some_and(|of| !key.is_of(of)) {
                next.push((key.clone(), regions.clone()));
                continue;
            }
            met = true;
            work.spend(regions.iter().map(|r| r.outer.len() + r.holes.iter().map(Vec::len).sum::<usize>()).sum())?;
            let seen: Vec<Region> =
                regions.iter().map(|r| Region { outer: there(&r.outer), holes: r.holes.iter().map(|h| there(h)).collect() }).collect();
            match split(&seen, &shape, prism.thickness, &mut work)? {
                Ok((a, b)) => {
                    any = true;
                    for (side, lumps) in [(sides[0], a), (sides[1], b)] {
                        for (n, lump) in lumps.into_iter().enumerate() {
                            let mut key = key.clone();
                            key.0.push(Step { cut: k as u32, side, lump: n as u32 });
                            let here = lump
                                .iter()
                                .map(|r| Region { outer: back(&r.outer), holes: r.holes.iter().map(|h| back(h)).collect() })
                                .collect();
                            next.push((key, here));
                        }
                    }
                }
                Err(Conflict::DoesNotCross) => return Err(DivideError::Conflict { at: Stage::Cut(k), conflict: Conflict::DoesNotCross }),
                Err(conflict) => {
                    if conflict == Conflict::HoldsAll {
                        why = conflict;
                    }
                    next.push((key.clone(), regions.clone()));
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
    for (key, regions) in parts {
        let mut surface = Mesh::new();
        for region in &regions {
            surface.extend(raised(&axes, region, prism.thickness, &mut work)?);
        }
        pieces.push(Piece { key, volume: mesh::volume(&surface), closed: mesh::is_closed(&surface), mesh: surface });
    }
    let removed = held - pieces.iter().map(|p| p.volume).sum::<f64>();
    Ok(Outcome { pieces, removed, conflicts: Vec::new() })
}
