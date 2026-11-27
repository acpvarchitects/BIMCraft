//! Divide Parts drawn on a face, as Revit's sketch of a division: the lines a front end has drawn on a part's face made into
//! the cuts `divide_parts` takes, and a division's cuts on that face drawn again as lines for Edit Division. Lines are
//! chained end to end within [`TOL`]: a chain that comes back to its start, or a closed line on its own, is an outline
//! (`Cut::Sketch` closed); one that doesn't is a line across the face (open). A division edited keeps its cuts in their
//! order, an unchanged line in its place, so the pieces they make keep their keys, and with them their ids and values.

use crate::curves::Curve;
use crate::divide::{Axes, Cut, Division, Frame, Recipe};
use crate::geom::P2;
use crate::model::TOL;
use crate::view3d::dot;

/// How near, in mm, a point of a cut drawn again is to the one it was: the same line.
const SAME: f64 = 1e-3;

/// What of a division a sketch on one face draws again: the lines of its cuts drawn on that face, in the face's own
/// coordinates, with the joint and the depth they were cut with; the division's other cuts are kept as they are.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sketched {
    pub lines: Vec<Curve>,
    pub gap: f64,
    pub depth: Option<f64>,
}

/// The lines of `recipe`'s cuts drawn on `face`'s plane, seen from the same side, in `face`'s coordinates.
pub fn sketched(recipe: &Recipe, face: &Frame) -> Sketched {
    let Ok(f) = face.axes() else { return Sketched::default() };
    let mut out = Sketched::default();
    for d in &recipe.cuts {
        if let Some((points, closed, depth)) = on_face(d, &f) {
            out.lines.push(Curve::Poly { points, arcs: Vec::new(), closed });
            out.gap = d.gap;
            out.depth = depth.or(out.depth);
        }
    }
    out
}

/// A cut drawn on face `f` as its points in `f`'s coordinates, whether it is closed, and how deep: a sketch of every piece
/// it meets (not of one piece only) in `f`'s plane, its normal the same way.
fn on_face(d: &Division, f: &Axes) -> Option<(Vec<P2>, bool, Option<f64>)> {
    let Cut::Sketch { frame, points, closed, depth } = &d.cut else { return None };
    if d.of.is_some() {
        return None;
    }
    let a = frame.axes().ok()?;
    if dot(a.n, f.n) < 1.0 - 1e-9 || f.local(a.o).1.abs() > SAME {
        return None;
    }
    Some((points.iter().map(|p| f.local(a.point(*p, 0.0)).0).collect(), *closed, *depth))
}

/// Whether `a` and `b` are the same line: the same points in turn, either way, and round from any of them if closed.
fn same(a: &[P2], b: &[P2], closed: bool) -> bool {
    let n = a.len();
    if n != b.len() || n == 0 {
        return false;
    }
    let near = |i: usize, j: usize| a[i].dist(b[j]) <= SAME;
    let starts: Vec<usize> = if closed { (0..n).collect() } else { vec![0] };
    starts.into_iter().any(|s| (0..n).all(|i| near(i, (s + i) % n)) || (0..n).all(|i| near(i, (s + n - i) % n)))
        || (!closed && (0..n).all(|i| near(i, n - 1 - i)))
}

/// `lines` chained end to end within [`TOL`]: each chain's points, and whether it closes. Refused where three ends meet.
fn chained(lines: &[Curve]) -> Result<Vec<(Vec<P2>, bool)>, String> {
    let mut out = Vec::new();
    let mut open: Vec<Vec<P2>> = Vec::new();
    for l in lines {
        let mut p = l.points();
        p.dedup_by(|b, a| a.dist(*b) <= SAME);
        if p.len() < 2 {
            continue;
        }
        let ends_meet = p.len() > 3 && p[0].dist(p[p.len() - 1]) <= TOL;
        if ends_meet || matches!(l, Curve::Poly { closed: true, .. }) {
            if ends_meet {
                p.pop();
            }
            out.push((p, true));
        } else {
            open.push(p);
        }
    }
    let ends: Vec<P2> = open.iter().flat_map(|p| [p[0], p[p.len() - 1]]).collect();
    if let Some(e) = ends.iter().find(|e| ends.iter().filter(|q| q.dist(**e) <= TOL).count() > 2) {
        return Err(format!(
            "three or more lines meet at ({:.0}, {:.0}): each outline closes on itself and each line runs on its own, so draw them apart",
            e.0, e.1
        ));
    }
    let meets = |p: &[P2], at: P2| p[0].dist(at) <= TOL || p[p.len() - 1].dist(at) <= TOL;
    while let Some(mut c) = open.pop() {
        loop {
            let tail = c[c.len() - 1];
            if let Some(i) = open.iter().position(|p| meets(p, tail)) {
                let mut p = open.remove(i);
                if p[0].dist(tail) > TOL {
                    p.reverse();
                }
                c.extend(p.into_iter().skip(1));
                continue;
            }
            let head = c[0];
            if let Some(i) = open.iter().position(|p| meets(p, head)) {
                let mut p = open.remove(i);
                if p[p.len() - 1].dist(head) > TOL {
                    p.reverse();
                }
                p.pop();
                p.extend(c);
                c = p;
                continue;
            }
            break;
        }
        let closed = c.len() > 3 && c[0].dist(c[c.len() - 1]) <= TOL;
        if closed {
            c.pop();
        }
        out.push((c, closed));
    }
    Ok(out)
}

/// `recipe` with its cuts drawn on `face` replaced by `lines`, chained into cuts, each `gap` wide and `depth` deep: a
/// chain the same as a cut it had keeps that cut's place, the others go after the rest. The cuts drawn elsewhere, and the
/// offsets, are kept. Refused when lines branch, or the face is no plane.
pub fn redivided(recipe: &Recipe, face: &Frame, lines: &[Curve], gap: f64, depth: Option<f64>) -> Result<Recipe, String> {
    let f = face.axes().map_err(|e| format!("the face to divide on: {e}"))?;
    let mut fresh: Vec<Option<(Vec<P2>, bool)>> = chained(lines)?.into_iter().map(Some).collect();
    let depth_of = |closed: bool| if closed { depth } else { None };
    let mut cuts = Vec::new();
    for d in &recipe.cuts {
        let Some((points, closed, _)) = on_face(d, &f) else {
            cuts.push(d.clone());
            continue;
        };
        let kept = fresh.iter_mut().find(|c| c.as_ref().is_some_and(|(q, cl)| *cl == closed && same(q, &points, closed)));
        if let Some(slot) = kept {
            *slot = None;
            let mut d = d.clone();
            d.gap = gap;
            if let Cut::Sketch { depth: was, .. } = &mut d.cut {
                *was = depth_of(closed);
            }
            cuts.push(d);
        }
    }
    for (points, closed) in fresh.into_iter().flatten() {
        cuts.push(Division { cut: Cut::Sketch { frame: *face, points, closed, depth: depth_of(closed) }, gap, of: None });
    }
    Ok(Recipe { cuts, offsets: recipe.offsets.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wall's south face: x east, y up, the normal pointing south.
    fn south() -> Frame {
        Frame { origin: [0.0, -150.0, 0.0], x: [1.0, 0.0, 0.0], normal: [0.0, -1.0, 0.0] }
    }

    fn points(cut: &Division) -> (Vec<P2>, bool, Option<f64>) {
        match &cut.cut {
            Cut::Sketch { points, closed, depth, .. } => (points.clone(), *closed, *depth),
            other => panic!("not a sketch: {other:?}"),
        }
    }

    fn rect(a: P2, b: P2) -> Curve {
        Curve::Poly { points: vec![a, P2(b.0, a.1), b, P2(a.0, b.1)], arcs: Vec::new(), closed: true }
    }

    #[test]
    fn lines_drawn_end_to_end_are_chained_into_one_cut_closed_where_they_come_back_and_open_where_they_dont() {
        let lines = [
            // An outline drawn as four lines, out of order and one of them backwards.
            Curve::line(P2(1000.0, 0.0), P2(1000.0, 500.0)),
            Curve::line(P2(0.0, 0.0), P2(1000.0, 0.0)),
            Curve::line(P2(0.0, 500.0), P2(1000.0, 500.0)),
            Curve::line(P2(0.0, 500.0), P2(0.0, 0.0)),
            // A line across the face in two lines, its joint 0.4 mm off.
            Curve::line(P2(-100.0, 1500.0), P2(800.0, 1400.0)),
            Curve::line(P2(800.4, 1400.0), P2(3000.0, 1600.0)),
            // A closed rectangle on its own.
            rect(P2(2000.0, 200.0), P2(2500.0, 900.0)),
        ];
        let r = redivided(&Recipe::default(), &south(), &lines, 20.0, Some(25.0)).expect("divided");
        assert_eq!(r.cuts.len(), 3, "{r:?}");
        assert!(r.offsets.is_empty());
        for cut in &r.cuts {
            assert_eq!(cut.gap, 20.0);
            assert_eq!(cut.of, None);
            let Cut::Sketch { frame, .. } = &cut.cut else { panic!() };
            assert_eq!(*frame, south());
        }
        let shapes: Vec<_> = r.cuts.iter().map(points).collect();
        let outline = shapes.iter().find(|(p, closed, _)| *closed && p.len() == 4 && p.contains(&P2(0.0, 0.0))).expect("the outline");
        assert_eq!(outline.2, Some(25.0), "an outline is cut that deep");
        for corner in [P2(0.0, 0.0), P2(1000.0, 0.0), P2(1000.0, 500.0), P2(0.0, 500.0)] {
            assert!(outline.0.contains(&corner), "{outline:?}");
        }
        let across = shapes.iter().find(|(_, closed, _)| !closed).expect("the line across");
        assert_eq!(across.0.len(), 3, "the joint is one point: {across:?}");
        assert_eq!(across.2, None, "a line across cuts through");
        let ends = [across.0[0], across.0[2]];
        assert!(ends.contains(&P2(-100.0, 1500.0)) && ends.contains(&P2(3000.0, 1600.0)), "{across:?}");
        assert!(shapes.iter().any(|(p, closed, _)| *closed && p.contains(&P2(2500.0, 900.0))));
    }

    #[test]
    fn a_circle_is_an_outline_of_its_points() {
        let r = redivided(&Recipe::default(), &south(), &[Curve::Circle { centre: P2(500.0, 500.0), radius: 200.0 }], 0.0, None)
            .expect("divided");
        let (p, closed, _) = points(&r.cuts[0]);
        assert!(closed && p.len() > 8, "{p:?}");
        assert!(p.iter().all(|q| (q.dist(P2(500.0, 500.0)) - 200.0).abs() < 1.0));
        assert_ne!(p.first(), p.last(), "a closed outline doesn't repeat its first point");
    }

    #[test]
    fn lines_that_branch_or_a_face_that_is_no_plane_are_refused() {
        let branching = [
            Curve::line(P2(0.0, 0.0), P2(1000.0, 0.0)),
            Curve::line(P2(1000.0, 0.0), P2(2000.0, 0.0)),
            Curve::line(P2(1000.0, 0.0), P2(1000.0, 900.0)),
        ];
        let e = redivided(&Recipe::default(), &south(), &branching, 0.0, None).expect_err("branches");
        assert!(e.contains("1000") && e.contains("three"), "{e}");
        let flat = Frame { normal: [0.0, 0.0, 0.0], ..south() };
        assert!(redivided(&Recipe::default(), &flat, &[Curve::line(P2(0.0, 0.0), P2(9.0, 0.0))], 0.0, None).is_err());
        let none = redivided(&Recipe::default(), &south(), &[], 0.0, None).expect("no lines");
        assert!(none.cuts.is_empty(), "no lines, no cuts: the part made whole");
    }

    #[test]
    fn a_division_is_drawn_again_on_its_face_and_edited_keeping_its_cuts_places() {
        let plane = Division { cut: Cut::Plane { point: [0.0, 0.0, 700.0], normal: [0.0, 0.0, 1.0] }, gap: 0.0, of: None };
        let first = rect(P2(0.0, 0.0), P2(1000.0, 500.0));
        let second = rect(P2(2000.0, 0.0), P2(2500.0, 500.0));
        let drawn = redivided(&Recipe { cuts: vec![plane.clone()], offsets: Vec::new() }, &south(), &[first.clone(), second], 10.0, None)
            .expect("divided");
        assert_eq!(drawn.cuts.len(), 3);
        assert_eq!(drawn.cuts[0], plane, "the cut drawn elsewhere stays first");

        // The same face seen through a frame of its own: another origin on the plane and another x.
        let other = Frame { origin: [500.0, -150.0, 300.0], x: [-1.0, 0.0, 0.0], normal: [0.0, -1.0, 0.0] };
        let back = sketched(&drawn, &other);
        assert_eq!((back.gap, back.depth), (10.0, None));
        assert_eq!(back.lines.len(), 2, "{back:?}");
        let corners: Vec<P2> = back.lines.iter().flat_map(Curve::points).collect();
        // Its x runs west and its y down (normal × x), so (0, 0) of the south frame is (500, 300) of it.
        assert!(corners.iter().any(|q| q.dist(P2(500.0, 300.0)) < 1e-6), "{corners:?}");
        assert!(back.lines.iter().all(|l| matches!(l, Curve::Poly { closed: true, .. })));

        // Not on that face: another plane, or the same plane seen from behind.
        let behind = Frame { normal: [0.0, 1.0, 0.0], ..south() };
        assert!(sketched(&drawn, &behind).lines.is_empty());
        let north = Frame { origin: [0.0, 150.0, 0.0], ..south() };
        assert!(sketched(&drawn, &north).lines.is_empty());

        // The first outline kept, the second gone, a new one drawn: the kept one keeps its place, the new one goes last.
        let third = rect(P2(3000.0, 0.0), P2(3500.0, 500.0));
        let kept: Vec<Curve> = sketched(&drawn, &south()).lines.into_iter().filter(|l| l.points().contains(&P2(0.0, 0.0))).collect();
        assert_eq!(kept.len(), 1);
        let edited = redivided(&drawn, &south(), &[third, kept[0].clone()], 10.0, None).expect("edited");
        assert_eq!(edited.cuts.len(), 3);
        assert_eq!(edited.cuts[0], plane);
        assert_eq!(edited.cuts[1], drawn.cuts[1], "the unchanged outline is cut 1 still, so its pieces keep their keys");
        assert!(points(&edited.cuts[2]).0.contains(&P2(3500.0, 500.0)));
    }
}
