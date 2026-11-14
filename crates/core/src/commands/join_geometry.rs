//! Join Geometry, Switch Join Order and Unjoin Geometry in the project (T23): two of the walls, floors, roofs and ceilings that
//! meet (overlap, or share a face) are joined, and the first cuts the second, which names it in its `joined_by` and loses what
//! the first's own solids take of it, in section and in 3D ([`crate::view3d::join_voids`]). A pair an edit parts stays joined,
//! cutting nothing, and `Model::warnings` says so. Joins may go round in a cycle (each is cut by the other's own solids, so
//! nothing recurses); a pair is joined one way at a time.

use super::*;

/// The two elements a join names, by id or GUID, as `first` (the one that cuts) and `second` (the one it cuts): each a wall,
/// a floor, a roof or a ceiling, and not one element twice.
fn pair(c: &str, m: &Model, p: &Value) -> Result<(Id, Id), String> {
    only(c, p, &["first", "second"])?;
    let get = |key: &str| match p.get(key) {
        v if absent(v) => {
            Err(bad(c, format!("`{key}` is required: {}", if key == "first" { "the element that cuts" } else { "the element it cuts" })))
        }
        Some(v) => element_id(c, m, v),
        None => Err(bad(c, format!("`{key}` is required"))),
    };
    let (first, second) = (get("first")?, get("second")?);
    for id in [first, second] {
        if !m.elements.get(id).is_some_and(Element::joinable) {
            return Err(bad(
                c,
                format!(
                    "{} is a {}: Join Geometry joins a wall, floor, roof or ceiling with another",
                    m.shown(id),
                    m.elements.get(id).map_or("nothing", Element::category)
                ),
            ));
        }
    }
    if first == second {
        return Err(bad(c, format!("{} can't be joined to itself", m.shown(first))));
    }
    Ok((first, second))
}

/// Whether `by` cuts `id` by a join.
fn cuts(m: &Model, by: Id, id: Id) -> bool {
    m.elements.get(id).is_some_and(|e| e.joined_by().contains(&m.guid(by)))
}

/// Element `id` named in `of`'s `joined_by`, or taken off it.
fn tie(m: &mut Model, of: Id, by: Guid, on: bool) {
    if let Some(list) = m.elements.get_mut(of).and_then(Element::joined_by_mut) {
        list.retain(|g| *g != by);
        if on {
            list.push(by);
        }
    }
}

/// Join Geometry: `first` cuts `second` from now on.
pub(super) fn join_geometry(s: &mut Session, p: &Value) -> R {
    let c = "join_geometry";
    let m = &s.model;
    let (first, second) = pair(c, m, p)?;
    let (a, b) = (m.shown(first), m.shown(second));
    if cuts(m, first, second) || cuts(m, second, first) {
        return Err(bad(c, format!("{a} and {b} are already joined: switch_join_order flips which cuts which")));
    }
    if m.elements.get(second).is_some_and(|e| e.joined_by().len() >= MAX_JOINS) {
        return Err(bad(c, format!("{b} is joined to {MAX_JOINS} elements already, the most one may be")));
    }
    if !crate::view3d::meets(m, first, second) {
        return Err(bad(c, format!("{a} and {b} don't meet: move one so they overlap or share a face, then join them")));
    }
    let g = m.guid(first);
    tie(s.model_mut(), second, g, true);
    Ok(json!({"first": first, "second": second}))
}

/// The pair a Switch or an Unjoin names, joined one way or the other: (the one that cuts, the one it cuts).
fn joined(c: &str, m: &Model, p: &Value) -> Result<(Id, Id), String> {
    let (first, second) = pair(c, m, p)?;
    if cuts(m, first, second) {
        Ok((first, second))
    } else if cuts(m, second, first) {
        Ok((second, first))
    } else {
        Err(bad(c, format!("{} and {} aren't joined: there is no join to change", m.shown(first), m.shown(second))))
    }
}

/// Switch Join Order: the one that was cut cuts the other.
pub(super) fn switch_join_order(s: &mut Session, p: &Value) -> R {
    let (by, cut) = joined("switch_join_order", &s.model, p)?;
    let (g_by, g_cut) = (s.model.guid(by), s.model.guid(cut));
    let m = s.model_mut();
    tie(m, cut, g_by, false);
    tie(m, by, g_cut, true);
    Ok(json!({"first": cut, "second": by}))
}

/// Unjoin Geometry: neither cuts the other.
pub(super) fn unjoin_geometry(s: &mut Session, p: &Value) -> R {
    let (by, cut) = joined("unjoin_geometry", &s.model, p)?;
    let g = s.model.guid(by);
    tie(s.model_mut(), cut, g, false);
    Ok(json!({"first": by, "second": cut}))
}
