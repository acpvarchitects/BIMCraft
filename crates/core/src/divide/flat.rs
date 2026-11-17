//! The lines of a cut in its frame's plane: checked and tidied, moved sideways for a gap, and raised into the solids
//! that cut.

use super::mesh::Soup;
use super::{Axes, DivideError, MAX_LINE_POINTS, MIN_SEGMENT, Mesh, REACH, Work, cap};
use crate::geom::{self, P2};
use crate::model::MAX_COORD;
use crate::view3d::{P3, add, mul};

/// Whether segments `p`–`q` and `a`–`b` meet anywhere, their ends included.
fn meet(p: P2, q: P2, a: P2, b: P2) -> bool {
    let turn = |a: P2, b: P2, c: P2| (b - a).cross(c - a);
    let (d1, d2, d3, d4) = (turn(p, q, a), turn(p, q, b), turn(a, b, p), turn(a, b, q));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return true;
    }
    let within = |s: P2, e: P2, x: P2| x.0 >= s.0.min(e.0) && x.0 <= s.0.max(e.0) && x.1 >= s.1.min(e.1) && x.1 <= s.1.max(e.1);
    (d1 == 0.0 && within(p, q, a)) || (d2 == 0.0 && within(p, q, b)) || (d3 == 0.0 && within(a, b, p)) || (d4 == 0.0 && within(a, b, q))
}

/// Whether a line, or a ring when `closed`, touches or crosses itself.
pub(crate) fn crosses_itself(points: &[P2], closed: bool) -> bool {
    let n = points.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    let seg = |k: usize| (points[k], points[(k + 1) % n]);
    (0..segs).any(|i| {
        (i + 2..segs).any(|j| {
            let wraps = closed && i == 0 && j == segs - 1;
            let ((p, q), (a, b)) = (seg(i), seg(j));
            !wraps && meet(p, q, a, b)
        })
    })
}

/// A line as it is cut along: finite, no more than [`MAX_LINE_POINTS`] points, points closer than [`MIN_SEGMENT`] made
/// one and those in line with their neighbours left out; a ring, when `closed`, counter-clockwise. Or what to fix.
pub(crate) fn clean(points: &[P2], closed: bool) -> Result<Vec<P2>, String> {
    if points.len() > MAX_LINE_POINTS {
        return Err(format!("it has {} points; a line or a profile takes {MAX_LINE_POINTS} at most", points.len()));
    }
    if points.iter().any(|p| !p.is_finite() || p.0.abs() > MAX_COORD || p.1.abs() > MAX_COORD) {
        return Err("one of its points isn't finite, or is beyond 1000 km".into());
    }
    let mut line: Vec<P2> = Vec::with_capacity(points.len());
    for &p in points {
        if line.last().is_none_or(|q| q.dist(p) >= MIN_SEGMENT) {
            line.push(p);
        }
    }
    if closed && line.len() > 1 && line.first().zip(line.last()).is_some_and(|(a, b)| a.dist(*b) < MIN_SEGMENT) {
        line.pop();
    }
    if closed && (line.len() < 3 || geom::area(&line).abs() < 1.0) {
        return Err("its outline has no area: give at least three points that aren't in a line".into());
    }
    // Points in line with their neighbours say nothing; a point the line turns back at makes a cut with no width.
    let mut k = 0;
    while line.len() >= 3 && k < line.len() {
        let n = line.len();
        if !closed && (k == 0 || k == n - 1) {
            k += 1;
            continue;
        }
        let (a, b, c) = (line[(k + n - 1) % n], line[k], line[(k + 1) % n]);
        let (d, e) = ((b - a).unit(), (c - b).unit());
        if d.dot(e) < -0.9998 {
            return Err(format!("it folds back on itself at ({:.0}, {:.0}): draw it without spikes", b.0, b.1));
        }
        if d.cross(e).abs() < 1e-9 {
            line.remove(k);
        } else {
            k += 1;
        }
    }
    if closed {
        if line.len() < 3 || geom::area(&line).abs() < 1.0 {
            return Err("its outline has no area: give at least three points that aren't in a line".into());
        }
        if geom::area(&line) < 0.0 {
            line.reverse();
        }
    } else if line.len() < 2 {
        return Err("a line needs two points apart".into());
    }
    if crosses_itself(&line, closed) {
        return Err("it crosses or touches itself: draw it as one line that doesn't".into());
    }
    Ok(line)
}

/// A line moved `d` mm to its left (to its right, when negative): each segment moved, the corners where they meet. A
/// counter-clockwise ring moved left shrinks.
pub(crate) fn shift(points: &[P2], closed: bool, d: f64) -> Vec<P2> {
    let n = points.len();
    let moved = |a: P2, b: P2| {
        let by = (b - a).unit().perp() * d;
        (a + by, b + by)
    };
    (0..n)
        .map(|k| {
            let (before, after) = ((k + n - 1) % n, (k + 1) % n);
            if !closed && k == 0 {
                return moved(points[k], points[after]).0;
            }
            if !closed && k == n - 1 {
                return moved(points[before], points[k]).1;
            }
            let ((a, b), (c, e)) = (moved(points[before], points[k]), moved(points[k], points[after]));
            geom::line_intersection(a, b, c, e).unwrap_or(b)
        })
        .collect()
}

/// Whether `moved`, a line moved sideways, is still a line like `line`: every segment runs the way it did, and it
/// doesn't cross itself.
pub(crate) fn sound(line: &[P2], moved: &[P2], closed: bool) -> bool {
    let n = line.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    moved.len() == n
        && moved.iter().all(|p| p.is_finite())
        && (0..segs).all(|k| (line[(k + 1) % n] - line[k]).dot(moved[(k + 1) % n] - moved[k]) > 0.0)
        && !crosses_itself(moved, closed)
}

/// A counter-clockwise ring drawn in by `by` mm; none where that closes it up.
pub(crate) fn shrunk(ring: &[P2], by: f64) -> Option<Vec<P2>> {
    if by <= 0.0 {
        return Some(ring.to_vec());
    }
    Some(shift(ring, true, by)).filter(|r| sound(ring, r, true) && geom::area(r) >= 1.0)
}

/// An open line drawn [`REACH`] past both its ends.
pub(crate) fn reaching(line: &[P2]) -> Vec<P2> {
    let mut out = line.to_vec();
    let n = out.len();
    if n >= 2 {
        out[0] = line[0] - (line[1] - line[0]).unit() * REACH;
        out[n - 1] = line[n - 1] + (line[n - 1] - line[n - 2]).unit() * REACH;
    }
    out
}

/// The strip from `right` mm on the right of an open line to `left` mm on its left, as a counter-clockwise ring; none
/// where the line turns too sharply to be moved that far.
pub(crate) fn strip(line: &[P2], left: f64, right: f64) -> Option<Vec<P2>> {
    let (l, r) = (shift(line, false, left), shift(line, false, -right));
    let fine = |moved: &[P2], by: f64| by.abs() < 1e-12 || sound(line, moved, false);
    if !fine(&l, left) || !fine(&r, right) {
        return None;
    }
    let ring: Vec<P2> = r.into_iter().chain(l.into_iter().rev()).collect();
    (!crosses_itself(&ring, true) && geom::area(&ring) > 0.0).then_some(ring)
}

/// The solid between a flat patch of triangles moved `up` mm and `down` mm along `n`, the way they face: the patch
/// twice, and a wall along each edge of it that only one triangle has.
pub(crate) fn slab(pts: &[P3], tris: &[[u32; 3]], n: P3, up: f64, down: f64) -> Mesh {
    let at = |i: u32, by: f64| add(pts.get(i as usize).copied().unwrap_or_default(), mul(n, by));
    let mut out = Vec::with_capacity(tris.len() * 2 + 64);
    for t in tris {
        out.push([at(t[0], up), at(t[1], up), at(t[2], up)]);
        out.push([at(t[0], down), at(t[2], down), at(t[1], down)]);
    }
    let edges = Soup { pts: pts.to_vec(), tris: tris.to_vec() }.open_edges();
    for (a, b) in edges {
        out.push([at(a, up), at(a, down), at(b, down)]);
        out.push([at(a, up), at(b, down), at(b, up)]);
    }
    out
}

/// A region of a frame's plane, its outline counter-clockwise and its holes clockwise, as corners in space and the
/// triangles that fill it, facing the way the frame's normal points.
pub(crate) fn patch(axes: &Axes, outline: &[P2], holes: &[Vec<P2>], work: &mut Work) -> Result<(Vec<P3>, Vec<[u32; 3]>), DivideError> {
    let mut flat: Vec<P2> = Vec::new();
    let mut rings: Vec<Vec<u32>> = Vec::new();
    // A point met twice (where a ring touches itself, or another) is one corner, so the filling knows it for the same.
    let mut known: std::collections::HashMap<(u64, u64), u32> = std::collections::HashMap::new();
    for (k, ring) in std::iter::once(outline).chain(holes.iter().map(Vec::as_slice)).enumerate() {
        let mut numbered: Vec<u32> = Vec::with_capacity(ring.len());
        for p in ring {
            let next = flat.len() as u32;
            let i = *known.entry((p.0.to_bits(), p.1.to_bits())).or_insert(next);
            if i == next {
                flat.push(*p);
            }
            numbered.push(i);
        }
        // Whichever way it was drawn: an outline counter-clockwise, a hole clockwise.
        if (geom::area(ring) > 0.0) != (k == 0) {
            numbered.reverse();
        }
        numbered.dedup();
        rings.push(numbered);
    }
    let tris = cap::fill(&flat, &rings, work)?;
    Ok((flat.iter().map(|q| axes.point(*q, 0.0)).collect(), tris))
}

/// The prism over a counter-clockwise `ring` of a frame's plane, from `lo` to `hi` mm along its normal.
pub(crate) fn prism(axes: &Axes, ring: &[P2], lo: f64, hi: f64, work: &mut Work) -> Result<Mesh, DivideError> {
    let (pts, tris) = patch(axes, ring, &[], work)?;
    Ok(slab(&pts, &tris, axes.n, hi, lo))
}
