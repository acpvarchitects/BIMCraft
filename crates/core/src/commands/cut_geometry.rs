//! Cut Geometry and Uncut Geometry in the project (T23): a component whose family is set to Cut with Voids When Loaded, placed
//! where its voids that cut a host (`cuts` `both` or `host`, as a face-hosted one's) pass through a wall, is named in the wall's
//! `void_cuts`, and from then on its voids cut the wall wherever it stands: in plan, in section, in 3D and as an IFC opening.
//! The tie is kept while the wall and the component stand, and does nothing while the family's flag is off or the voids are
//! gone or out of reach; Uncut takes it off.

use super::*;
use crate::view3d::P3;

/// The wall and the component a Cut or an Uncut names, by id or GUID, as `target` and `cutter`.
fn pair(c: &str, m: &Model, p: &Value) -> Result<(Id, Id), String> {
    only(c, p, &["target", "cutter"])?;
    let get = |key: &str| match p.get(key) {
        v if absent(v) => {
            Err(bad(c, format!("`{key}` is required: {}", if key == "target" { "the wall to cut" } else { "the component that cuts it" })))
        }
        Some(v) => element_id(c, m, v),
        None => Err(bad(c, format!("`{key}` is required"))),
    };
    let (target, cutter) = (get("target")?, get("cutter")?);
    if m.wall(target).is_none() {
        return Err(bad(
            c,
            format!("{} is a {}, not a wall: Cut Geometry cuts a wall with a component's voids", m.shown(target), kind(m, target)),
        ));
    }
    if !matches!(m.elements.get(cutter), Some(Element::Component(_))) {
        return Err(bad(
            c,
            format!("{} is a {}, not a component: the cutter is a placed family with voids", m.shown(cutter), kind(m, cutter)),
        ));
    }
    Ok((target, cutter))
}

fn kind(m: &Model, id: Id) -> &'static str {
    m.elements.get(id).map_or("nothing", Element::category)
}

/// Whether any of `voids` passes into wall `id` and takes some of it. First cheaply: their outline in plan (a hull) overlaps
/// the wall's, and their heights its own; then on the solids themselves ([`crate::view3d::takes_from_wall`]), since a concave
/// or tilted void's hull may span the wall while the void misses it.
fn reaches(m: &Model, id: Id, voids: &[Vec<[P3; 3]>]) -> bool {
    let Some(w) = m.wall(id) else { return false };
    let outline = crate::walls::outline(m, id);
    let (base, top) = crate::walls::heights(m, w);
    let near: Vec<Vec<[P3; 3]>> = voids
        .iter()
        .filter(|v| {
            let pts: Vec<P3> = v.iter().flatten().copied().collect();
            let (lo, hi) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p[2]), hi.max(p[2])));
            if hi <= base + 1e-6 || lo >= top - 1e-6 {
                return false;
            }
            let hull = crate::geom::hull(pts.iter().map(|p| P2(p[0], p[1])).collect());
            hull.len() >= 3 && overlap(&hull, &outline)
        })
        .cloned()
        .collect();
    !near.is_empty() && crate::view3d::takes_from_wall(m, id, &near)
}

/// Whether rings `a` and `b` share some area: a corner of one inside the other, or two of their sides crossing.
fn overlap(a: &[P2], b: &[P2]) -> bool {
    if b.len() < 3 {
        return false;
    }
    a.iter().any(|p| crate::geom::contains(b, *p))
        || b.iter().any(|p| crate::geom::contains(a, *p))
        || crate::geom::ring_edges(a).any(|(p, q)| crate::geom::ring_edges(b).any(|(r, s)| crate::geom::crossing(p, q, r, s).is_some()))
}

/// Cut Geometry: wall `target` is cut by component `cutter`'s voids from now on.
pub(super) fn cut_geometry(s: &mut Session, p: &Value) -> R {
    let c = "cut_geometry";
    let m = &s.model;
    let (target, cutter) = pair(c, m, p)?;
    let Some(Element::Component(comp)) = m.elements.get(cutter) else { return Err(bad(c, "the cutter is a component")) };
    let (wall, by) = (m.shown(target), m.shown(cutter));
    let Some(f) = m.families.get(&comp.family) else { return Err(bad(c, format!("{by}'s family `{}` isn't loaded", comp.family))) };
    if !f.cut_with_voids_when_loaded {
        return Err(bad(
            c,
            format!(
                "family `{}` isn't set to Cut with Voids When Loaded: turn it on in its options ({{\"name\": \"{}\", \"options\": {{\"cut_with_voids_when_loaded\": true}}}} to `family`), then cut",
                comp.family, comp.family
            ),
        ));
    }
    if let Some(w) = m.wall(target)
        && w.void_cuts.contains(&m.guid(cutter))
    {
        return Err(bad(c, format!("{by} already cuts {wall}")));
    }
    if m.hosted_voids(target).iter().any(|(id, _)| *id == cutter) {
        return Err(bad(c, format!("{by} stands on a face of {wall} and already cuts it as its host")));
    }
    let voids = m.cut_voids(comp);
    if voids.is_empty() {
        return Err(bad(
            c,
            format!(
                "{by} has no void to cut with: family `{}` needs a void form that cuts a host (`cuts` `both` or `host`; one that cuts `forms` only cuts the family's own)",
                comp.family
            ),
        ));
    }
    if !reaches(m, target, &voids) {
        return Err(bad(c, format!("{by}'s voids don't pass into {wall}: move it so they meet the wall, then cut")));
    }
    let g = m.guid(cutter);
    if let Some(Element::Wall(w)) = s.model_mut().elements.get_mut(target) {
        w.void_cuts.push(g);
    }
    Ok(json!({"id": target, "cutter": cutter}))
}

/// Uncut Geometry: wall `target` is no longer cut by component `cutter`.
pub(super) fn uncut_geometry(s: &mut Session, p: &Value) -> R {
    let c = "uncut_geometry";
    let m = &s.model;
    let (target, cutter) = pair(c, m, p)?;
    let g = m.guid(cutter);
    if !m.wall(target).is_some_and(|w| w.void_cuts.contains(&g)) {
        return Err(bad(c, format!("{} doesn't cut {} by Cut Geometry: there is nothing to uncut", m.shown(cutter), m.shown(target))));
    }
    if let Some(Element::Wall(w)) = s.model_mut().elements.get_mut(target) {
        w.void_cuts.retain(|x| *x != g);
    }
    Ok(json!({"id": target, "cutter": cutter}))
}
