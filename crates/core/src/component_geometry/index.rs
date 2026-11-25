//! GEO-2g-2: what a plan reads of each component of a model before it builds any, kept across drawings, and where each
//! stands in plan. Each component's key, its envelope and the box of its model lines, placed, are found once and kept with
//! the elements' revisions they were found for ([`crate::ids::RevisionSnapshot`]); a uniform grid of the boxes in plan, of
//! every component on every level, finds those near a plan's window without a pass over all of them. An edit finds again
//! only what changed and what depends on it: its level, and a hosted one's host and what the host stands on.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use super::Key;
use crate::geom::P2;
use crate::ids::{Guid, Revision, RevisionSnapshot};
use crate::model::{Component, Element, Id, Model};

/// A box in space, lowest corner first.
pub(crate) type Box3 = [[f64; 3]; 2];

/// A model line's ends in its family's axes, mirrored where the key says.
pub(crate) type Line = [[f64; 3]; 2];

/// The side of a cell of the grid, mm.
const CELL: f64 = 4096.0;
/// The most cells a component is put in; one reaching over more is looked at by every query.
const MOST_CELLS: i64 = 256;
/// The most local shapes kept; past them they are found again.
const LOCALS: usize = 1 << 18;

/// What a key gives without building it: its local envelope (none where it isn't known) and its model lines' ends.
#[derive(Clone)]
pub(crate) struct Local {
    pub(crate) envelope: Option<Box3>,
    pub(crate) lines: Arc<Vec<Line>>,
}

/// What the index holds of a component.
#[derive(Clone)]
pub(crate) struct Entry {
    /// Its revision when found: its key and local shape stand while it is the same.
    revision: Option<Revision>,
    pub(crate) key: Arc<Key>,
    pub(crate) local: Local,
    /// Its envelope placed; none where it isn't known.
    pub(crate) forms: Option<Box3>,
    /// The box of its model lines placed; none for none.
    pub(crate) lines: Option<Box3>,
    /// Its level, and whether its family has plan symbols (annotation of that level, wherever it stands).
    level: Guid,
    symbols: bool,
    /// The elements its place is found from, by GUID: its level, its host, and what the host stands on. By GUID, so one not in
    /// the model now (a host gone, or not yet made) is waited on, and found when it comes (ec's review of #90).
    deps: Vec<Guid>,
    hosted: bool,
    at: Where,
}

/// Each cell's components, with the first cell of each (its lowest x and y): the one a query finds it in. A map, so one is taken
/// out of a crowded cell at once (ec's review of #90).
type Cells = HashMap<(i64, i64), HashMap<Id, (i64, i64)>>;

/// Where an entry is found.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Where {
    /// In these cells, `[x0, y0, x1, y1]`.
    Cells([i64; 4]),
    /// Over too many cells: looked at by every query.
    Big,
    /// Its envelope isn't known: a candidate everywhere.
    Always,
}

/// The index of a model's components ([`Index::of`]), and what it was found for.
#[derive(Clone)]
pub(crate) struct Index {
    revisions: RevisionSnapshot,
    types: crate::shared::Identity<crate::model::Types>,
    entries: HashMap<Id, Entry>,
    cells: Cells,
    big: BTreeSet<Id>,
    always: BTreeSet<Id>,
    /// The components with plan symbols, by level.
    marked: HashMap<Guid, BTreeSet<Id>>,
    /// What depends on each element beside itself, by the element's GUID.
    dependents: HashMap<Guid, BTreeSet<Id>>,
    /// The GUID of each element something depends on, as last found: what a gone one was.
    known: HashMap<Id, Guid>,
    locals: HashMap<Arc<Key>, Local>,
}

/// A plan's question of the index: its window (none, all of the plan), and for a floor plan its cut plane and view depth.
#[derive(Clone, Copy)]
pub(crate) struct Query {
    pub(crate) window: Option<[P2; 2]>,
    pub(crate) range: Option<(f64, f64)>,
    pub(crate) level: Guid,
}

fn cell(v: f64) -> i64 {
    (v / CELL).floor().clamp(-1e15, 1e15) as i64
}

fn meets(window: Option<[P2; 2]>, b: &Box3) -> bool {
    window.is_none_or(|[a, c]| !(a.0 > b[1][0] || c.0 < b[0][0] || a.1 > b[1][1] || c.1 < b[0][1]))
}

/// The box round `points`; none for none, or one not of finite numbers.
fn box_of(points: impl Iterator<Item = [f64; 3]>) -> Option<Box3> {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    let mut any = false;
    for p in points {
        if !p.iter().all(|v| v.is_finite()) {
            return None;
        }
        any = true;
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    any.then_some([lo, hi])
}

/// `c`'s model lines in its family's axes, at any detail, mirrored as it is: what [`Model::component_model_line_parts`]
/// places.
pub(crate) fn local_lines(m: &Model, c: &Component) -> Vec<Line> {
    let Some(f) = m.families.get(&c.family) else { return Vec::new() };
    if f.model_lines.is_empty() {
        return Vec::new();
    }
    let Ok(values) = f.values(&c.type_name, &c.params) else { return Vec::new() };
    let flip = |p: [f64; 3]| [p[0], if c.mirrored { -p[1] } else { p[1] }, p[2]];
    f.model_line_parts(&values).into_iter().map(|([a, b], ..)| [flip(a), flip(b)]).collect()
}

impl Index {
    /// Of every component of `m`.
    pub(crate) fn of(m: &Model) -> Index {
        let mut index = Index {
            revisions: m.elements.revision_snapshot(),
            types: m.types.identity(),
            entries: HashMap::new(),
            cells: HashMap::new(),
            big: BTreeSet::new(),
            always: BTreeSet::new(),
            marked: HashMap::new(),
            dependents: HashMap::new(),
            known: HashMap::new(),
            locals: HashMap::new(),
        };
        for (id, e) in m.elements.iter() {
            if let Element::Component(c) = e {
                index.put(m, *id, e, c, None);
            }
        }
        index
    }

    /// Brought up to `m`, whose families are those it was found with: what changed since, and what depends on it, found
    /// again. Whether it was already.
    pub(crate) fn bring_up(&mut self, m: &Model) -> bool {
        let now = m.elements.revision_snapshot();
        let mut todo = now.changed_since(&self.revisions);
        let types = !self.types.is_of(&m.types);
        if todo.is_empty() && !types {
            return true;
        }
        if types {
            // A host's faces are of its type: every hosted one is placed again.
            todo.extend(self.entries.iter().filter(|(_, e)| e.hosted).map(|(id, _)| *id));
        }
        let deps: Vec<Id> = todo
            .iter()
            .filter_map(|id| m.elements.guid(*id).or_else(|| self.known.get(id).copied()))
            .filter_map(|g| self.dependents.get(&g))
            .flatten()
            .copied()
            .collect();
        todo.extend(deps);
        for id in todo {
            let old = self.take(id);
            if let Some(e @ Element::Component(c)) = m.elements.get(id) {
                self.put(m, id, e, c, old);
            }
        }
        self.revisions = now;
        self.types = m.types.identity();
        false
    }

    pub(crate) fn get(&self, id: Id) -> Option<&Entry> {
        self.entries.get(&id)
    }

    /// `c`'s entry found, its key and local shape kept from `old` where `c` is as it was.
    fn put(&mut self, m: &Model, id: Id, e: &Element, c: &Component, old: Option<Entry>) {
        count(&ENTRIES);
        let revision = m.elements.revision(id);
        let (key, local) = match old.filter(|o| o.revision.is_some() && o.revision == revision) {
            Some(o) => (o.key, o.local),
            None => {
                let key = m.shape_key(c);
                match self.locals.get_key_value(&key) {
                    Some((k, l)) => (Arc::clone(k), l.clone()),
                    None => {
                        let local = Local { envelope: super::local_envelope(m, c), lines: Arc::new(local_lines(m, c)) };
                        if self.locals.len() >= LOCALS {
                            self.locals.clear();
                        }
                        let key = Arc::new(key);
                        self.locals.insert(Arc::clone(&key), local.clone());
                        (key, local)
                    }
                }
            }
        };
        let forms = local.envelope.and_then(|b| match crate::bounds::place_component(m, c, b) {
            crate::bounds::Bounds::Box(b) => Some(b),
            _ => None,
        });
        let lines = if local.lines.is_empty() {
            None
        } else {
            let placed = m.component_placement(c);
            // Lines not of finite numbers: a box of no use, so it is looked at everywhere.
            box_of(local.lines.iter().flatten().map(|p| placed.place(*p))).or(Some([[f64::NEG_INFINITY; 3], [f64::INFINITY; 3]]))
        };
        let symbols = m.families.get(&c.family).is_some_and(|f| !f.symbols.is_empty());
        // Its level and host, and what its host stands on (a wall's levels, a floor's): what its place is found from.
        let mut deps: Vec<Guid> = Vec::new();
        let mut refs = e.refs();
        if let Some(h) = c.host {
            refs.push(h.host.guid());
            if let Some(host) = m.elements.get(h.host.guid()) {
                refs.extend(host.refs());
            }
        }
        let own = m.elements.guid(id);
        for g in refs {
            if !g.is_nil() && Some(g) != own && !deps.contains(&g) {
                deps.push(g);
            }
        }
        for g in &deps {
            self.dependents.entry(*g).or_default().insert(id);
            if let Some(d) = m.elements.id(*g) {
                self.known.insert(d, *g);
            }
        }
        let reach = match (forms, lines) {
            (Some(f), Some(l)) => Some([std::array::from_fn(|k| f[0][k].min(l[0][k])), std::array::from_fn(|k| f[1][k].max(l[1][k]))]),
            (Some(b), None) => Some(b),
            // No envelope: it may be anywhere its build puts it.
            (None, _) => None,
        };
        let at = match reach {
            Some(b) => {
                let r = [cell(b[0][0]), cell(b[0][1]), cell(b[1][0]), cell(b[1][1])];
                if (r[2] - r[0] + 1).saturating_mul(r[3] - r[1] + 1) > MOST_CELLS || r[2] - r[0] >= MOST_CELLS || r[3] - r[1] >= MOST_CELLS
                {
                    Where::Big
                } else {
                    Where::Cells(r)
                }
            }
            None => Where::Always,
        };
        match at {
            Where::Cells([x0, y0, x1, y1]) => {
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        self.cells.entry((x, y)).or_default().insert(id, (x0, y0));
                    }
                }
            }
            Where::Big => {
                self.big.insert(id);
            }
            Where::Always => {
                self.always.insert(id);
            }
        }
        if symbols {
            self.marked.entry(c.level).or_default().insert(id);
        }
        let entry = Entry { revision, key, local, forms, lines, level: c.level, symbols, deps, hosted: c.host.is_some(), at };
        self.entries.insert(id, entry);
    }

    /// `id`'s entry taken out of the index.
    fn take(&mut self, id: Id) -> Option<Entry> {
        let e = self.entries.remove(&id)?;
        match e.at {
            Where::Cells([x0, y0, x1, y1]) => {
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        if let Some(v) = self.cells.get_mut(&(x, y)) {
                            count(&SCANNED);
                            v.remove(&id);
                            if v.is_empty() {
                                self.cells.remove(&(x, y));
                            }
                        }
                    }
                }
            }
            Where::Big => {
                self.big.remove(&id);
            }
            Where::Always => {
                self.always.remove(&id);
            }
        }
        if e.symbols
            && let Some(set) = self.marked.get_mut(&e.level)
        {
            set.remove(&id);
            if set.is_empty() {
                self.marked.remove(&e.level);
            }
        }
        for d in &e.deps {
            if let Some(set) = self.dependents.get_mut(d) {
                set.remove(&id);
                if set.is_empty() {
                    self.dependents.remove(d);
                }
            }
        }
        Some(e)
    }

    /// The components that may show in a plan asking `q`, by id: those whose envelope meets its window and (a floor plan)
    /// may reach from its view depth to under its cut, those whose model lines do, those with symbols on its level, and
    /// those whose envelope isn't known. Every one a plan shows, and a few near.
    pub(crate) fn query(&self, q: Query) -> BTreeSet<Id> {
        let mut out: BTreeSet<Id> = self.always.iter().copied().collect();
        if let Some(set) = self.marked.get(&q.level) {
            out.extend(set.iter().copied());
        }
        let fits = |e: &Entry| {
            let forms =
                e.forms.is_some_and(|b| meets(q.window, &b) && q.range.is_none_or(|(cut, depth)| !(b[0][2] >= cut || b[1][2] <= depth)));
            let slack = crate::model::TOL + 1.0;
            let lines = e.lines.is_some_and(|b| {
                meets(q.window, &b) && q.range.is_none_or(|(cut, depth)| b[0][2] <= cut + slack && b[1][2] >= depth - slack)
            });
            forms || lines
        };
        let mut look = |id: Id| {
            count(&CANDIDATES);
            if let Some(e) = self.entries.get(&id)
                && fits(e)
            {
                out.insert(id);
            }
        };
        for &id in &self.big {
            look(id);
        }
        let Some([a, b]) = q.window else {
            for (id, e) in &self.entries {
                if matches!(e.at, Where::Cells(_)) {
                    look(*id);
                }
            }
            return out;
        };
        let r = [cell(a.0), cell(a.1), cell(b.0), cell(b.1)];
        let span = (r[2] - r[0] + 1).saturating_mul(r[3] - r[1] + 1);
        // Each found once: in the first cell of the window it is in.
        let mut home = |(x, y): (i64, i64), list: &HashMap<Id, (i64, i64)>| {
            for (&id, &(x0, y0)) in list {
                if (x0.max(r[0]), y0.max(r[1])) == (x, y) {
                    look(id);
                }
            }
        };
        if span <= self.cells.len() as i64 {
            for x in r[0]..=r[2] {
                for y in r[1]..=r[3] {
                    count(&CELLS);
                    if let Some(list) = self.cells.get(&(x, y)) {
                        home((x, y), list);
                    }
                }
            }
        } else {
            // A window over more cells than hold anything: the cells that do, looked at instead.
            for (&(x, y), list) in &self.cells {
                count(&CELLS);
                if (r[0]..=r[2]).contains(&x) && (r[1]..=r[3]).contains(&y) {
                    home((x, y), list);
                }
            }
        }
        out
    }

    /// How many local shapes it holds.
    #[cfg(test)]
    pub(crate) fn locals_len(&self) -> usize {
        self.locals.len()
    }

    /// Whether it is of `m`'s elements as they are, every entry found as `m` would find it now (a test's check).
    #[cfg(test)]
    pub(crate) fn same_as_made(&self, m: &Model) -> bool {
        let fresh = Index::of(m);
        let entries = self.entries.len() == fresh.entries.len()
            && self.entries.iter().all(|(id, e)| {
                fresh.entries.get(id).is_some_and(|f| {
                    *e.key == *f.key && e.forms == f.forms && e.lines == f.lines && e.at == f.at && e.local.envelope == f.local.envelope
                })
            });
        let sorted = |c: &Cells| {
            let mut v: Vec<_> = c.iter().map(|(k, l)| (*k, l.iter().map(|(i, f)| (*i, *f)).collect::<BTreeSet<_>>())).collect();
            v.sort();
            v
        };
        let deps = self.entries.iter().all(|(id, e)| fresh.entries.get(id).is_some_and(|f| e.deps == f.deps));
        entries
            && deps
            && sorted(&self.cells) == sorted(&fresh.cells)
            && self.big == fresh.big
            && self.always == fresh.always
            && self.marked == fresh.marked
            && self.dependents == fresh.dependents
    }
}

#[cfg(test)]
thread_local! {
    static ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CELLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CANDIDATES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SCANNED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(not(test))]
struct Counter;
#[cfg(not(test))]
static ENTRIES: Counter = Counter;
#[cfg(not(test))]
static CELLS: Counter = Counter;
#[cfg(not(test))]
static CANDIDATES: Counter = Counter;
#[cfg(not(test))]
static SCANNED: Counter = Counter;

#[cfg(test)]
fn count(c: &'static std::thread::LocalKey<std::cell::Cell<usize>>) {
    c.with(|n| n.set(n.get() + 1));
}

#[cfg(not(test))]
fn count(_: &Counter) {}

/// How many entries of a cell this thread has looked at to take a component out of it, since it began.
#[cfg(test)]
pub(crate) fn scanned() -> usize {
    SCANNED.with(std::cell::Cell::get)
}

/// How many entries this thread has found, cells it has looked in and candidates it has looked at, since it began.
#[cfg(test)]
pub(crate) fn counts() -> (usize, usize, usize) {
    (ENTRIES.with(std::cell::Cell::get), CELLS.with(std::cell::Cell::get), CANDIDATES.with(std::cell::Cell::get))
}
