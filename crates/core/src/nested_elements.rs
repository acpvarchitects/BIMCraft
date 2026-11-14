//! Shared nested instances as elements of their own (T6b), as Revit makes them: each component whose family places an
//! instance of a shared nested family (`family/nested.rs`, `Family::shared_instances`) exposes that instance in the project
//! as a [`NestedElement`], which Tab selects inside its host in a plan, schedules list among the components, a tag names,
//! and IFC writes as an element of its own, aggregated into its host's.
//!
//! It stays derived from its host: nothing of it is in a file. After every command and when a file is read,
//! [`Model::sync_nested`] makes one for each shared nested instance of each component, keeps the ones there are, brings
//! what they say of their host up to date and takes away the ones whose instance is gone (and the tags on them). Its GUID is
//! worked out from its host's and the nested instances' (`guid_of`), so a tag or a selection that holds it finds it again
//! after the file is saved and opened; its id, as any element's coming in from a file, may be another then. Its geometry is
//! its host's pieces that say they are its (`Piece::nested`), placed where its host stands: the host draws it in plans,
//! sections and 3D, so it is in every view as before, and it is selected apart only where the view lets it be picked.
//! An instance of a nested family that isn't shared stays part of its host and nothing more.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::family::Piece;
use crate::ids::{Guid, Id};
use crate::model::{Component, Element, Model};

type P3 = [f64; 3];

/// What [`Model::sync_nested`] changes: the nested elements to make, to bring up to date and to take away, and the tags on
/// those taken away.
#[derive(Debug, Default)]
pub struct NestedChanges {
    made: Vec<(Guid, NestedElement)>,
    changed: Vec<(Guid, NestedElement)>,
    stale: Vec<Guid>,
    tags: Vec<Guid>,
}

/// A shared nested instance of component `host`, an element of its own (T6b). `path` names the nested instances from the
/// host's family down to it, by their GUIDs; the rest is worked out from the host and kept up to date by
/// [`Model::sync_nested`]: its family and type, the host's level, and its family's axes in the model (`origin`, `axes`; a
/// mirrored host gives it left-handed ones). Never written to a file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestedElement {
    pub host: Guid,
    pub path: Vec<Guid>,
    pub family: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub level: Guid,
    pub origin: P3,
    pub axes: [P3; 3],
}

/// The GUID of the element shared nested instance `path` of component `host` is: the same for the same two, whatever
/// the element's id, so what holds it finds it after the file is saved and read again.
pub fn guid_of(host: Guid, path: &[Guid]) -> Guid {
    let fold = |g: Guid| {
        let x = g.as_u128();
        (x >> 64) as u64 ^ x as u64
    };
    let seed = path.iter().fold(crate::model::mix(fold(host) ^ 0x7436_6273_6e65_7374), |s, g| crate::model::mix(s ^ fold(*g)));
    Guid::seeded(seed, path.len() as u64)
}

impl Model {
    /// The shared nested instances of component `c` (whose GUID is `host`), as the elements they are, worked out from it.
    pub fn nested_of(&self, host: Guid, c: &Component) -> Vec<NestedElement> {
        let Some(f) = self.families.get(&c.family) else { return Vec::new() };
        if !f.nested.iter().any(|n| n.family.shared) {
            return Vec::new();
        }
        let Ok(values) = f.values(&c.type_name, &c.params) else { return Vec::new() };
        let placed = self.component_placement(c);
        let flip = |p: P3| [p[0], if c.mirrored { -p[1] } else { p[1] }, p[2]];
        let [a, b, z] = placed.axes;
        let turn = |p: P3| -> P3 { std::array::from_fn(|i| p[0] * a[i] + p[1] * b[i] + p[2] * z[i]) };
        f.shared_instances(&values)
            .into_iter()
            .map(|s| NestedElement {
                host,
                origin: placed.place(flip(s.origin)),
                axes: s.axes.map(|v| turn(flip(v))),
                path: s.path,
                family: s.family,
                type_name: s.type_name,
                level: c.level,
            })
            .collect()
    }

    /// Each component's shared nested instances made elements of their own, as they are now: one made for each that has
    /// none, those there are brought up to date, and those whose instance is gone taken away with the tags on them. Whether
    /// anything changed.
    pub fn sync_nested(&mut self) -> bool {
        match self.nested_changes() {
            Some(c) => {
                self.apply_nested(c);
                true
            }
            None => false,
        }
    }

    /// What [`Model::sync_nested`] would change; none where the elements are as their hosts make them. It reads the model only,
    /// so a command that changed nothing of it isn't made to look as if it had.
    pub fn nested_changes(&self) -> Option<NestedChanges> {
        let any_shared = self.families.values().any(|f| f.nested.iter().any(|n| n.family.shared));
        let mut want: BTreeMap<Guid, NestedElement> = BTreeMap::new();
        if any_shared {
            for (_, guid, e) in self.elements.with_guids() {
                if let Element::Component(c) = e {
                    for n in self.nested_of(guid, c) {
                        want.insert(guid_of(guid, &n.path), n);
                    }
                }
            }
        }
        let mut c = NestedChanges::default();
        for (_, guid, e) in self.elements.with_guids() {
            let Element::Nested(n) = e else { continue };
            match want.remove(&guid) {
                Some(w) if w == *n => {}
                Some(w) => c.changed.push((guid, w)),
                None => c.stale.push(guid),
            }
        }
        c.made = want.into_iter().collect();
        if !c.stale.is_empty() {
            let gone: std::collections::BTreeSet<Guid> = c.stale.iter().copied().collect();
            c.tags = self
                .elements
                .with_guids()
                .filter(|(_, _, e)| matches!(e, Element::Tag(t) if gone.contains(&t.host)))
                .map(|(_, g, _)| g)
                .collect();
        }
        (!(c.stale.is_empty() && c.changed.is_empty() && c.made.is_empty())).then_some(c)
    }

    /// `c` done: what [`Model::nested_changes`] found.
    pub fn apply_nested(&mut self, c: NestedChanges) {
        for g in c.stale.iter().chain(&c.tags) {
            self.elements.remove(*g);
            self.pinned.remove(g);
        }
        for (g, n) in c.changed {
            if let Some(e) = self.elements.get_mut(g) {
                *e = Element::Nested(n);
            }
        }
        for (g, n) in c.made {
            let id = self.fresh_id();
            // A GUID made from others' can't be one of theirs but by a collision of 128 bits; were it, it is left out.
            let _ = self.elements.insert_as(id, g, Element::Nested(n));
        }
    }

    /// The elements of component `host`'s shared nested instances, by id, with the paths that name them.
    pub fn nested_in(&self, host: Guid) -> Vec<(Id, &[Guid])> {
        self.lookup()
            .referrers(&host)
            .filter_map(|id| match self.elements.get(id) {
                Some(Element::Nested(n)) if n.host == host => Some((id, n.path.as_slice())),
                _ => None,
            })
            .collect()
    }

    /// Nested element `n`'s solids in the model: those of its host's that are its own, placed where the host stands. A
    /// shared instance nested inside it is an element of its own, so its pieces aren't among these.
    pub fn nested_pieces(&self, n: &NestedElement) -> Vec<Piece> {
        let Some(Element::Component(c)) = self.elements.get(n.host) else { return Vec::new() };
        self.component_pieces(c).into_iter().filter(|p| p.nested == n.path).collect()
    }

    /// Nested element `n`'s host component, where it is one.
    pub fn nested_host(&self, n: &NestedElement) -> Option<&Component> {
        match self.elements.get(n.host) {
            Some(Element::Component(c)) => Some(c),
            _ => None,
        }
    }
}
