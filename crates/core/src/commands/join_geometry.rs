//! Join Geometry, Switch Join Order and Unjoin Geometry in the project (T23): two of the walls, floors, roofs and ceilings that
//! meet (overlap, or share a face) are joined, and the first cuts the second, which names it in its `joined_by` and loses what
//! the first's own solids take of it, in section and in 3D ([`crate::view3d::join_voids`]). A pair an edit parts stays joined,
//! cutting nothing, and `Model::warnings` says so. A pair is joined one way at a time, and joins never go round in a loop:
//! each is cut by the others' own solids, so a loop would take what they all share out of every one of them.

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

/// The loop of joins `first` cutting `second` would close: `second` cutting `first` already through a chain, each cutting
/// the next, as `[second, …, first]`; none when it closes none. The walk goes back from `first` through what cuts each
/// (`joined_by`, at most [`MAX_JOINS`] of each), every element once, leaving out `skip` (`(cut, by)`: the join a Switch
/// takes back).
fn closes_loop(m: &Model, first: Id, second: Id, skip: Option<(Id, Id)>) -> Option<Vec<Id>> {
    // What each element reached was reached from: the one it cuts.
    let mut cuts_next: BTreeMap<Id, Id> = BTreeMap::new();
    let mut seen = BTreeSet::from([first]);
    let mut todo = vec![first];
    while let Some(at) = todo.pop() {
        let Some(e) = m.elements.get(at) else { continue };
        for g in e.joined_by().iter().take(MAX_JOINS) {
            let Some(by) = m.elements.id(*g) else { continue };
            if skip == Some((at, by)) {
                continue;
            }
            if by == second {
                let mut chain = vec![second, at];
                let mut x = at;
                while let Some(&next) = cuts_next.get(&x) {
                    chain.push(next);
                    x = next;
                }
                return Some(chain);
            }
            if seen.insert(by) {
                cuts_next.insert(by, at);
                todo.push(by);
            }
        }
    }
    None
}

/// Refused when `first` cutting `second` would close a loop of joins ([`closes_loop`]), naming it.
fn no_loop(c: &str, m: &Model, first: Id, second: Id, skip: Option<(Id, Id)>) -> Result<(), String> {
    let Some(chain) = closes_loop(m, first, second, skip) else { return Ok(()) };
    let names: Vec<String> = chain.iter().chain([&second]).map(|id| m.shown(*id)).collect();
    Err(bad(
        c,
        format!(
            "{} cutting {} would close a loop of joins ({}), which would take what they all share out of every one of them: \
             join these two the other way round, or unjoin one of the loop first",
            m.shown(first),
            m.shown(second),
            names.join(" cuts ")
        ),
    ))
}

/// Refused when `id` is cut by [`MAX_JOINS`] elements already, the most one may be.
fn full(c: &str, m: &Model, id: Id) -> Result<(), String> {
    if m.elements.get(id).is_some_and(|e| e.joined_by().len() >= MAX_JOINS) {
        return Err(bad(c, format!("{} is cut by {MAX_JOINS} joined elements already, the most one may be", m.shown(id))));
    }
    Ok(())
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
    full(c, m, second)?;
    no_loop(c, m, first, second, None)?;
    if !crate::view3d::meets(m, first, second, None) {
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
    let c = "switch_join_order";
    let (by, cut) = joined(c, &s.model, p)?;
    // The one that cut is cut from now on: it must have room for one more.
    full(c, &s.model, by)?;
    no_loop(c, &s.model, cut, by, Some((cut, by)))?;
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
