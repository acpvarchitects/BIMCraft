//! A flat region filled with triangles: the section a plane cuts through a solid, a profile's end, a prism's face. The
//! region is given by its edges, each with the region on its left, between corners that are numbers: the triangles use
//! those corners and no others, and every edge given is an edge of exactly one of them, so a surface closed along the
//! region's outline stays closed whatever the numbers are. Holes are joined to their outline by a bridge walked both
//! ways, and the one ring that makes is cut ear by ear.

use std::f64::consts::PI;

use super::{DivideError, Work};
use crate::geom::P2;

fn at(pts: &[P2], i: u32) -> P2 {
    pts.get(i as usize).copied().unwrap_or_default()
}

/// A ring's area, about its first corner: positive counter-clockwise.
fn ring_area(pts: &[P2], ring: &[u32]) -> f64 {
    let o = ring.first().map_or(P2(0.0, 0.0), |&i| at(pts, i));
    ring.windows(2).map(|w| (at(pts, w[0]) - o).cross(at(pts, w[1]) - o)).sum::<f64>() / 2.0
}

/// Whether `p` is in the ring: even-odd.
fn in_ring(pts: &[P2], ring: &[u32], p: P2) -> bool {
    let n = ring.len();
    (0..n)
        .filter(|&k| {
            let (a, b) = (at(pts, ring[k]), at(pts, ring[(k + 1) % n]));
            (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1)
        })
        .count()
        % 2
        == 1
}

/// Edges, each from one corner to the next with the region on its left, chained into rings: at a corner where several
/// leave, the one turning furthest left, so two rings that touch at a corner stay two. None where a chain has no way on:
/// the edges weren't the outline of a region.
pub(crate) fn rings(pts: &[P2], edges: &[(u32, u32)]) -> Option<Vec<Vec<u32>>> {
    let mut edges = edges.to_vec();
    edges.sort_unstable();
    let mut used = vec![false; edges.len()];
    let mut out = Vec::new();
    for start in 0..edges.len() {
        if used.get(start).copied().unwrap_or(true) {
            continue;
        }
        let first = edges.get(start)?.0;
        let mut ring = Vec::new();
        let mut e = start;
        loop {
            *used.get_mut(e)? = true;
            let &(a, b) = edges.get(e)?;
            ring.push(a);
            if b == first {
                break;
            }
            let d_in = at(pts, b) - at(pts, a);
            let from = edges.partition_point(|x| x.0 < b);
            let mut best: Option<(f64, usize)> = None;
            for k in from..edges.len() {
                let Some(&(s, to)) = edges.get(k) else { break };
                if s != b {
                    break;
                }
                if used.get(k).copied().unwrap_or(true) {
                    continue;
                }
                let d_out = at(pts, to) - at(pts, b);
                let mut turn = d_in.cross(d_out).atan2(d_in.dot(d_out));
                if turn >= PI - 1e-12 {
                    turn = -PI;
                }
                if best.is_none_or(|(t, _)| turn > t) {
                    best = Some((turn, k));
                }
            }
            e = best?.1;
        }
        out.push(ring);
    }
    Some(out)
}

/// The region the `rings` bound, in triangles, each counter-clockwise: a ring that runs counter-clockwise is an
/// outline, one that runs clockwise a hole in the smallest outline round it.
pub(crate) fn fill(pts: &[P2], rings: &[Vec<u32>], work: &mut Work) -> Result<Vec<[u32; 3]>, DivideError> {
    let areas: Vec<f64> = rings.iter().map(|r| ring_area(pts, r)).collect();
    let mut holes: Vec<Vec<usize>> = vec![Vec::new(); rings.len()];
    let mut out = Vec::new();
    for (k, ring) in rings.iter().enumerate() {
        let a = areas.get(k).copied().unwrap_or(0.0);
        if ring.len() < 3 {
            continue;
        }
        if a > 1e-12 {
            continue;
        }
        // A hole goes to its outline; one with none, or a ring with no area, is fanned as it runs.
        let probe = (at(pts, ring[0]) + at(pts, ring[1])) * 0.5;
        let mut home: Option<usize> = None;
        for (j, other) in rings.iter().enumerate() {
            let size = areas.get(j).copied().unwrap_or(0.0);
            work.spend(other.len())?;
            if a < -1e-12 && size > 1e-12 && in_ring(pts, other, probe) && home.and_then(|h| areas.get(h)).is_none_or(|&best| size < best) {
                home = Some(j);
            }
        }
        match home.and_then(|h| holes.get_mut(h)) {
            Some(list) => list.push(k),
            None => out.extend((1..ring.len() - 1).map(|i| [ring[0], ring[i], ring[i + 1]])),
        }
    }
    for (k, ring) in rings.iter().enumerate() {
        if ring.len() < 3 || areas.get(k).copied().unwrap_or(0.0) <= 1e-12 {
            continue;
        }
        let inner: Vec<&Vec<u32>> = holes.get(k).into_iter().flatten().filter_map(|&h| rings.get(h)).collect();
        let whole = bridge(pts, ring, &inner, work)?;
        ears(pts, &whole, &mut out, work)?;
    }
    // A triangle on a bridge walked there and straight back has a corner twice: its two edges match each other.
    out.retain(|t| t[0] != t[1] && t[1] != t[2] && t[2] != t[0]);
    Ok(out)
}

/// `a`, `b`, `c` turning left: positive.
fn turn(a: P2, b: P2, c: P2) -> f64 {
    (b - a).cross(c - a)
}

/// Whether segments `p`–`q` and `a`–`b` meet anywhere, their ends included.
fn meet(p: P2, q: P2, a: P2, b: P2) -> bool {
    let (d1, d2, d3, d4) = (turn(p, q, a), turn(p, q, b), turn(a, b, p), turn(a, b, q));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return true;
    }
    let within = |s: P2, e: P2, x: P2| x.0 >= s.0.min(e.0) && x.0 <= s.0.max(e.0) && x.1 >= s.1.min(e.1) && x.1 <= s.1.max(e.1);
    (d1 == 0.0 && within(p, q, a)) || (d2 == 0.0 && within(p, q, b)) || (d3 == 0.0 && within(a, b, p)) || (d4 == 0.0 && within(a, b, q))
}

/// An outline and its holes as one ring: each hole, from the one reaching furthest right, joined at its rightmost
/// corner to the nearest corner of the ring so far that sees it, by an edge walked there and back.
fn bridge(pts: &[P2], outline: &[u32], holes: &[&Vec<u32>], work: &mut Work) -> Result<Vec<u32>, DivideError> {
    let mut whole = outline.to_vec();
    let mut order: Vec<(usize, usize)> = holes
        .iter()
        .enumerate()
        .filter_map(|(h, hole)| {
            let k = (0..hole.len()).max_by(|&i, &j| {
                let (p, q) = (at(pts, hole[i]), at(pts, hole[j]));
                p.0.total_cmp(&q.0).then(q.1.total_cmp(&p.1)).then(j.cmp(&i))
            })?;
            Some((h, k))
        })
        .collect();
    order.sort_by(|x, y| {
        let (p, q) = (at(pts, holes[x.0][x.1]), at(pts, holes[y.0][y.1]));
        q.0.total_cmp(&p.0).then(p.1.total_cmp(&q.1)).then(x.0.cmp(&y.0))
    });
    for (n, &(h, k)) in order.iter().enumerate() {
        let hole = holes[h];
        let (hv, hp) = (hole[k], at(pts, hole[k]));
        let mut near: Vec<(f64, usize)> = whole.iter().enumerate().map(|(j, &v)| ((at(pts, v) - hp).length(), j)).collect();
        near.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        let m = whole.len();
        let mut seen: Option<usize> = None;
        for &(_, j) in &near {
            let (v, p) = (whole[j], at(pts, whole[j]));
            if v == hv {
                seen = Some(j);
                break;
            }
            // The way to the hole must leave this corner into the region: between the edge on and the edge back.
            let (to, back, d) = (at(pts, whole[(j + 1) % m]) - p, at(pts, whole[(j + m - 1) % m]) - p, hp - p);
            let inside =
                if to.cross(back) > 0.0 { to.cross(d) > 0.0 && d.cross(back) > 0.0 } else { !(back.cross(d) >= 0.0 && d.cross(to) >= 0.0) };
            if !inside {
                continue;
            }
            work.spend(m + order.iter().skip(n).map(|&(o, _)| holes[o].len()).sum::<usize>())?;
            let blocked = |ring: &[u32]| {
                let len = ring.len();
                (0..len).any(|i| {
                    let (a, b) = (ring[i], ring[(i + 1) % len]);
                    a != v && b != v && a != hv && b != hv && meet(hp, p, at(pts, a), at(pts, b))
                })
            };
            if !blocked(&whole) && !order.iter().skip(n).any(|&(o, _)| blocked(holes[o])) {
                seen = Some(j);
                break;
            }
        }
        // A hole nothing sees is joined to the nearest corner all the same: the surface stays closed.
        let j = seen.or(near.first().map(|x| x.1)).unwrap_or(0);
        let mut next = Vec::with_capacity(m + hole.len() + 2);
        next.extend_from_slice(&whole[..=j]);
        if whole[j] == hv {
            next.extend(hole[k + 1..].iter().chain(&hole[..k]));
            next.push(hv);
        } else {
            next.extend(hole[k..].iter().chain(&hole[..k]));
            next.push(hv);
            next.push(whole[j]);
        }
        next.extend_from_slice(&whole[j + 1..]);
        whole = next;
    }
    Ok(whole)
}

/// A ring, counter-clockwise, cut into triangles ear by ear. An ear is a corner turning left whose triangle holds no
/// other corner of the ring, on its edges included: so a corner in line with its neighbours is never cut off as a
/// triangle with no area, but is taken by a triangle that has one. A ring with no ear left (it crosses itself) gives
/// up its most left-turning corner: its triangles may overlap, its edges still match.
fn ears(pts: &[P2], ring: &[u32], out: &mut Vec<[u32; 3]>, work: &mut Work) -> Result<(), DivideError> {
    let n = ring.len();
    if n < 3 {
        return Ok(());
    }
    let mut prev: Vec<usize> = (0..n).map(|i| (i + n - 1) % n).collect();
    let mut next: Vec<usize> = (0..n).map(|i| (i + 1) % n).collect();
    let p = |i: usize| at(pts, ring[i]);
    let mut left = n;
    let mut cursor = 0;
    while left > 3 {
        let mut found: Option<usize> = None;
        let mut widest: (f64, usize) = (f64::NEG_INFINITY, cursor);
        let mut k = cursor;
        for _ in 0..left {
            let (a, c) = (prev[k], next[k]);
            let (pa, pb, pc) = (p(a), p(k), p(c));
            let bend = turn(pa, pb, pc);
            if bend > widest.0 {
                widest = (bend, k);
            }
            // In line with its neighbours, to a ten-millionth of a turn, a corner is no ear: its triangle has no area.
            if bend > 1e-7 * (pb - pa).length() * (pc - pb).length() {
                work.spend(left)?;
                let mut m = next[c];
                let mut clear = true;
                while m != a {
                    let (v, q) = (ring[m], p(m));
                    if v != ring[a]
                        && v != ring[k]
                        && v != ring[c]
                        && turn(pa, pb, q) >= 0.0
                        && turn(pb, pc, q) >= 0.0
                        && turn(pc, pa, q) >= 0.0
                    {
                        clear = false;
                        break;
                    }
                    m = next[m];
                }
                if clear {
                    found = Some(k);
                    break;
                }
            }
            k = next[k];
        }
        let k = found.unwrap_or(widest.1);
        let (a, c) = (prev[k], next[k]);
        out.push([ring[a], ring[k], ring[c]]);
        next[a] = c;
        prev[c] = a;
        left -= 1;
        cursor = next[c];
    }
    let a = cursor;
    out.push([ring[a], ring[next[a]], ring[next[next[a]]]]);
    Ok(())
}
