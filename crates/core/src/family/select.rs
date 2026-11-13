//! What a family document selects, and the selection commands Revit's family editor has on it (FE-1, FE-20), for any front
//! end: what one thing selected is ([`Picked`]); Filter's kinds, each with what of the selection is of it ([`groups`],
//! [`kept`]); Select by ID, the parts a list of ids and GUIDs names ([`find`]) and what names a part ([`names`], IDs of
//! Selection); and Select All Instances, the nested instances of the selected ones' family and type ([`instances`]).
//!
//! A part is named as an element is: by its id (six digits, given when the family is loaded into a project: one made since
//! has none yet) or its GUID, however written (`inquiry`'s words).

use bimcraft_i18n::ui_msg;

use super::Family;
use super::forms::FormKind;
use crate::ids::{Guid, Id};

/// One thing selected in a family document that isn't adaptive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Picked {
    /// A form, by its place in the family's forms.
    Form(usize),
    /// A reference plane, a datum or the family's own.
    Plane(Guid),
    /// A reference line.
    Line(Guid),
    /// A model line (FE-15).
    ModelLine(Guid),
    Dimension(Guid),
    /// A plan symbol, by its place in the family's symbols.
    Symbol(usize),
    /// A connector, by its place in the family's connectors.
    Connector(usize),
    /// A nested instance, by its place in the family's nested instances.
    Nested(usize),
}

/// What Filter counts a selection by, as Revit's counts a project's by category: a form by what it is, solid or void, and
/// the other things a family document selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A solid form.
    Form(FormKind),
    /// A void form.
    Void(FormKind),
    /// A form of the triangles a file brought, solid or void.
    Mesh,
    ReferencePlane,
    ReferenceLine,
    ModelLine,
    Dimension,
    Symbol,
    Connector,
    Nested,
}

impl Kind {
    /// Where Filter lists it, the same in every language: the forms first, each kind solid then void as the Create tab
    /// has them (Extrusion, Blend, Revolve, Sweep, Swept Blend), and a file's meshes; then the datums (reference planes,
    /// then lines), dimensions, symbols, connectors and nested instances, in the order a family's lists hold them.
    pub fn rank(self) -> u8 {
        let form = |k: FormKind| match k {
            FormKind::Extrusion => 0,
            FormKind::Blend => 1,
            FormKind::Revolve => 2,
            FormKind::Sweep => 3,
            FormKind::SweptBlend => 4,
        };
        match self {
            Kind::Form(k) => form(k),
            Kind::Void(k) => 5 + form(k),
            Kind::Mesh => 10,
            Kind::ReferencePlane => 11,
            Kind::ReferenceLine => 12,
            Kind::ModelLine => 13,
            Kind::Dimension => 14,
            Kind::Symbol => 15,
            Kind::Connector => 16,
            Kind::Nested => 17,
        }
    }

    /// What Filter's list calls it, in English: several of it, as Revit's list names categories.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Form(FormKind::Extrusion) => ui_msg!("Extrusions"),
            Kind::Form(FormKind::Blend) => ui_msg!("Blends"),
            Kind::Form(FormKind::Revolve) => ui_msg!("Revolves"),
            Kind::Form(FormKind::Sweep) => ui_msg!("Sweeps"),
            Kind::Form(FormKind::SweptBlend) => ui_msg!("Swept Blends"),
            Kind::Void(FormKind::Extrusion) => ui_msg!("Void Extrusions"),
            Kind::Void(FormKind::Blend) => ui_msg!("Void Blends"),
            Kind::Void(FormKind::Revolve) => ui_msg!("Void Revolves"),
            Kind::Void(FormKind::Sweep) => ui_msg!("Void Sweeps"),
            Kind::Void(FormKind::SweptBlend) => ui_msg!("Void Swept Blends"),
            Kind::Mesh => ui_msg!("Meshes"),
            Kind::ReferencePlane => ui_msg!("Reference Planes"),
            Kind::ReferenceLine => ui_msg!("Reference Lines"),
            Kind::ModelLine => ui_msg!("Model Lines"),
            Kind::Dimension => ui_msg!("Dimensions"),
            Kind::Symbol => ui_msg!("Symbols"),
            Kind::Connector => ui_msg!("Connectors"),
            Kind::Nested => ui_msg!("Nested Instances"),
        }
    }
}

/// The id and the GUID `p` holds in `family`: none where the family hasn't it. An id of 0 and a nil GUID are ones not given
/// yet.
fn ident(family: &Family, p: Picked) -> Option<(Id, Guid)> {
    match p {
        Picked::Form(i) => family.forms.get(i).map(|f| (f.id, f.guid)),
        Picked::Plane(g) => family.planes.iter().find(|q| q.guid == g).map(|q| (q.id, q.guid)),
        Picked::Line(g) => family.lines.iter().find(|l| l.guid == g).map(|l| (l.id, l.guid)),
        Picked::ModelLine(g) => family.model_lines.iter().find(|l| l.guid == g).map(|l| (l.id, l.guid)),
        Picked::Dimension(g) => family.dimensions.iter().find(|d| d.guid == g).map(|d| (d.id, d.guid)),
        Picked::Symbol(i) => family.symbols.get(i).map(|y| (y.id, y.guid)),
        Picked::Connector(i) => family.connectors.get(i).map(|c| (c.id, c.guid)),
        Picked::Nested(i) => family.nested_instances.get(i).map(|n| (n.id, n.guid)),
    }
}

/// Whether `family` has `p`.
pub fn holds(family: &Family, p: Picked) -> bool {
    ident(family, p).is_some()
}

/// What `p` is, for Filter: none where `family` hasn't it.
pub fn kind(family: &Family, p: Picked) -> Option<Kind> {
    if !holds(family, p) {
        return None;
    }
    Some(match p {
        Picked::Form(i) => match family.forms.get(i)? {
            f if !f.mesh.is_empty() => Kind::Mesh,
            f if f.void => Kind::Void(f.kind()),
            f => Kind::Form(f.kind()),
        },
        Picked::Plane(_) => Kind::ReferencePlane,
        Picked::Line(_) => Kind::ReferenceLine,
        Picked::ModelLine(_) => Kind::ModelLine,
        Picked::Dimension(_) => Kind::Dimension,
        Picked::Symbol(_) => Kind::Symbol,
        Picked::Connector(_) => Kind::Connector,
        Picked::Nested(_) => Kind::Nested,
    })
}

/// A kind the selection holds, and what of the selection is of it, in the order selected.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub kind: Kind,
    pub items: Vec<Picked>,
}

/// Filter's list for `picked`, a selection of `family`'s: each kind it holds with what is of it (each thing once), the kinds
/// in an order of their own ([`Kind::rank`]), never by their names, which a translation would reorder. What the family
/// hasn't is left out.
pub fn groups(family: &Family, picked: &[Picked]) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for p in picked {
        let Some(kind) = kind(family, *p) else { continue };
        match out.iter_mut().find(|g| g.kind == kind) {
            Some(g) if g.items.contains(p) => {}
            Some(g) => g.items.push(*p),
            None => out.push(Group { kind, items: vec![*p] }),
        }
    }
    out.sort_by_key(|g| g.kind.rank());
    out
}

/// What of `picked` is of one of the kinds `keep`: the selection Filter leaves, in the order selected, each thing once.
pub fn kept(family: &Family, picked: &[Picked], keep: &[Kind]) -> Vec<Picked> {
    let mut out: Vec<Picked> = Vec::new();
    for p in picked {
        if kind(family, *p).is_some_and(|k| keep.contains(&k)) && !out.contains(p) {
            out.push(*p);
        }
    }
    out
}

/// What a list found in a family: its parts, each once, in the order named, and what named none.
#[derive(Debug, Default, PartialEq)]
pub struct Found {
    pub picked: Vec<Picked>,
    pub missing: Vec<String>,
}

/// Everything `family` holds that a front end selects, in the order its lists hold it.
pub fn all(family: &Family) -> Vec<Picked> {
    let mut out: Vec<Picked> = (0..family.forms.len()).map(Picked::Form).collect();
    out.extend(family.planes.iter().map(|p| Picked::Plane(p.guid)));
    out.extend(family.lines.iter().map(|l| Picked::Line(l.guid)));
    out.extend(family.model_lines.iter().map(|l| Picked::ModelLine(l.guid)));
    out.extend(family.dimensions.iter().map(|d| Picked::Dimension(d.guid)));
    out.extend((0..family.symbols.len()).map(Picked::Symbol));
    out.extend((0..family.connectors.len()).map(Picked::Connector));
    out.extend((0..family.nested_instances.len()).map(Picked::Nested));
    out
}

/// The parts of `family` `text` names, as Select by ID finds a project's elements (`inquiry::find`): by id or by GUID
/// (dashed or not, braced or not, in either case, or as an IFC GlobalId's 22 characters), any number of them, separated by
/// spaces, commas, semicolons or lines; too long a list is cut as a project's is. A part with no id yet is found by its
/// GUID alone, one with neither by nothing.
pub fn find(family: &Family, text: &str) -> Found {
    let parts: Vec<(Picked, Id, Guid)> = all(family).into_iter().filter_map(|p| ident(family, p).map(|(id, g)| (p, id, g))).collect();
    let mut out = Found::default();
    for word in crate::inquiry::words(text) {
        let named = crate::inquiry::guid_of(word).and_then(|g| {
            parts.iter().find(|(_, id, guid)| match g.pending() {
                Some(n) => !crate::ids::unset(id) && *id == n,
                None => !guid.is_nil() && *guid == g,
            })
        });
        match named {
            Some((p, ..)) if out.picked.contains(p) => {}
            Some((p, ..)) => out.picked.push(*p),
            None => out.missing.push(word.to_string()),
        }
    }
    out
}

/// What names a part of a family: its id (0: none yet) and its GUID (nil: none yet), and what it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Names {
    pub picked: Picked,
    pub kind: Kind,
    pub id: Id,
    pub guid: Guid,
}

/// What names `p` in `family`; none where the family hasn't it.
pub fn names(family: &Family, p: Picked) -> Option<Names> {
    let (id, guid) = ident(family, p)?;
    Some(Names { picked: p, kind: kind(family, p)?, id, guid })
}

/// Select All Instances: every nested instance of `family` of the family and type of one of the nested instances in
/// `picked`, in the order the family holds them. With `shown`, Visible in View: only those it says the view in front
/// shows, by place ([`super::views::nested_shown`]; one it says nothing of isn't shown); without, all of the family's.
/// Nothing when `picked` holds no nested instance: a form, a plane or a dimension has no type.
pub fn instances(family: &Family, picked: &[Picked], shown: Option<&[bool]>) -> Vec<Picked> {
    let types: Vec<(&str, &str)> = picked
        .iter()
        .filter_map(|p| match p {
            Picked::Nested(i) => family.nested_instances.get(*i).map(|n| (n.family.as_str(), n.type_name.as_str())),
            _ => None,
        })
        .collect();
    family
        .nested_instances
        .iter()
        .enumerate()
        .filter(|(_, n)| types.contains(&(n.family.as_str(), n.type_name.as_str())))
        .filter(|(i, _)| shown.is_none_or(|s| s.get(*i).copied().unwrap_or(false)))
        .map(|(i, _)| Picked::Nested(i))
        .collect()
}

/// Whether `p` is pinned where it is (FE-17): a datum always, else as the family has it. What `family` hasn't is not.
pub fn pinned(family: &Family, p: Picked) -> bool {
    match p {
        Picked::Form(i) => family.forms.get(i).is_some_and(|f| f.pinned),
        Picked::Plane(g) => family.planes.iter().any(|x| x.guid == g && (x.pinned || x.datum())),
        Picked::Line(g) => family.lines.iter().any(|x| x.guid == g && x.pinned),
        Picked::ModelLine(g) => family.model_lines.iter().any(|x| x.guid == g && x.pinned),
        Picked::Dimension(g) => family.dimensions.iter().any(|x| x.guid == g && x.pinned),
        Picked::Symbol(i) => family.symbols.get(i).is_some_and(|x| x.pinned),
        Picked::Connector(i) => family.connectors.get(i).is_some_and(|x| x.pinned),
        Picked::Nested(i) => family.nested_instances.get(i).is_some_and(|x| x.pinned),
    }
}

/// The first of `picked` that is pinned, in their order: what a move, Delete or Align refuses, naming it.
pub fn pinned_among(family: &Family, picked: &[Picked]) -> Option<Picked> {
    picked.iter().copied().find(|p| pinned(family, *p))
}

/// Pins (`on`) or unpins each of `picked` that `family` has, as Pin and Unpin do (FE-17), and says which changed, in their
/// order: a datum is pinned always and never among them.
pub fn pin(family: &mut Family, picked: &[Picked], on: bool) -> Vec<Picked> {
    let mut changed = Vec::new();
    for &p in picked {
        let flag = match p {
            Picked::Form(i) => family.forms.get_mut(i).map(|x| &mut x.pinned),
            Picked::Plane(g) => family.planes.iter_mut().find(|x| x.guid == g && !x.datum()).map(|x| &mut x.pinned),
            Picked::Line(g) => family.lines.iter_mut().find(|x| x.guid == g).map(|x| &mut x.pinned),
            Picked::ModelLine(g) => family.model_lines.iter_mut().find(|x| x.guid == g).map(|x| &mut x.pinned),
            Picked::Dimension(g) => family.dimensions.iter_mut().find(|x| x.guid == g).map(|x| &mut x.pinned),
            Picked::Symbol(i) => family.symbols.get_mut(i).map(|x| &mut x.pinned),
            Picked::Connector(i) => family.connectors.get_mut(i).map(|x| &mut x.pinned),
            Picked::Nested(i) => family.nested_instances.get_mut(i).map(|x| &mut x.pinned),
        };
        if let Some(flag) = flag.filter(|f| **f != on) {
            *flag = on;
            changed.push(p);
        }
    }
    changed
}

/// `p` as a message names it: a form, a plane and a reference line as a move's refusal does ([`Family::said`]), the rest
/// by what it is and its place (`symbol 2`); English, as the core's messages are.
pub fn said(family: &Family, p: Picked) -> String {
    use super::constraints::Named;
    let at = |what: &str, i: usize| format!("{what} {}", i + 1);
    match p {
        Picked::Form(i) => family.said(Named::Form(i)),
        Picked::Plane(g) => family.said(Named::Plane(g)),
        Picked::Line(g) => family.said(Named::Line(g)),
        Picked::ModelLine(g) => family.model_lines.iter().position(|l| l.guid == g).map_or_else(|| g.to_string(), |i| at("model line", i)),
        Picked::Dimension(g) => family.dimensions.iter().position(|d| d.guid == g).map_or_else(|| g.to_string(), |i| at("dimension", i)),
        Picked::Symbol(i) => at("symbol", i),
        Picked::Connector(i) => at("connector", i),
        Picked::Nested(i) => at("nested instance", i),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const EDGE: &str = "fe200000-0000-4000-8000-000000000001";
    const LINE: &str = "fe200000-0000-4000-8000-000000000002";
    const DIM: &str = "fe200000-0000-4000-8000-000000000003";
    const SOLID: &str = "fe200000-0000-4000-8000-000000000004";

    fn g(s: &str) -> Guid {
        Guid::parse(s).unwrap()
    }

    /// Two extrusions (the first with an id and a GUID, as a project gives them), a void extrusion and a revolve; a plane of
    /// its own, a reference line, a dimension, two symbols and a connector; a small box nested, placed three times, one of
    /// another type and one switched off.
    fn family() -> Family {
        let square = |x: f64| json!([[x, 0], [x + 100.0, 0], [x + 100.0, 100], [x, 100]]);
        let small = json!({"category": "generic", "parameters": [{"name": "W", "value": 100}], "types": {"S": {}, "L": {"W": 300}},
            "forms": [{"profile": [[0, 0], ["W", 0], ["W", 100], [0, 100]], "to": 100}]});
        serde_json::from_value(json!({
            "category": "generic",
            "parameters": [{"name": "Shown", "kind": "yesno", "value": false}],
            "types": {"T": {}},
            "forms": [
                {"id": 123456, "guid": SOLID, "profile": square(0.0), "to": 100},
                {"profile": square(200.0), "to": 100},
                {"profile": square(400.0), "to": 100, "void": true},
                {"profile": square(600.0), "axis": [[590, 0], [590, 100]], "from": 0, "to": 180},
            ],
            "planes": [{"guid": EDGE, "id": 123457, "name": "Edge", "start": [-400, -1000, 0], "end": [-400, 1000, 0], "up": [0, 0, 1]}],
            "lines": [{"guid": LINE, "start": [-200, 900, 0], "end": [200, 900, 0]}],
            "dimensions": [{"guid": DIM, "refs": [{"plane": EDGE}, {"plane": "9e3c0a10-1f0b-4d8c-8a5e-0000000000a3"}], "view": "plan", "at": [-200, -1200, 0]}],
            "symbols": [{"line": [[-100, -700], [100, -700]]}, {"dot": {"centre": [0, -800], "radius": 20}}],
            "connectors": [{"kind": "power"}],
            "nested": [{"name": "Small", "family": small}],
            "nested_instances": [
                {"family": "Small", "type": "S", "at": [1000, 0]},
                {"family": "Small", "type": "L", "at": [1000, 500]},
                {"family": "Small", "type": "S", "at": [1000, 1000]},
                {"family": "Small", "type": "S", "at": [1000, 1500], "visible": "Shown"},
            ],
        }))
        .unwrap()
    }

    #[test]
    fn filter_counts_a_selection_by_kind_and_keeps_the_kinds_checked_in_the_order_selected() {
        let f = family();
        let picked = [
            Picked::Symbol(1),
            Picked::Form(1),
            Picked::Plane(g(EDGE)),
            Picked::Form(2),
            Picked::Nested(0),
            Picked::Form(0),
            Picked::Form(3),
            Picked::Dimension(g(DIM)),
            Picked::Line(g(LINE)),
            Picked::Connector(0),
            Picked::Symbol(0),
            Picked::Form(1),
            Picked::Form(9),
            Picked::Plane(g(LINE)),
        ];
        let listed: Vec<(&str, usize)> = groups(&f, &picked).iter().map(|g| (g.kind.name(), g.items.len())).collect();
        assert_eq!(
            listed,
            [
                ("Extrusions", 2),
                ("Revolves", 1),
                ("Void Extrusions", 1),
                ("Reference Planes", 1),
                ("Reference Lines", 1),
                ("Dimensions", 1),
                ("Symbols", 2),
                ("Connectors", 1),
                ("Nested Instances", 1),
            ],
            "each kind once, in an order of their own that no language changes: forms, solid then void, then datums, dimensions, \
             symbols, connectors and nested instances; what the family hasn't and what is there twice left out"
        );
        let extrusions = groups(&f, &picked).into_iter().find(|g| g.kind == Kind::Form(FormKind::Extrusion)).unwrap();
        assert_eq!(extrusions.items, [Picked::Form(1), Picked::Form(0)], "in the order selected");
        assert_eq!(
            kept(&f, &picked, &[Kind::Symbol, Kind::Form(FormKind::Extrusion)]),
            [Picked::Symbol(1), Picked::Form(1), Picked::Form(0), Picked::Symbol(0)]
        );
        assert!(kept(&f, &picked, &[]).is_empty() && kept(&f, &picked, &[Kind::Mesh]).is_empty());
        assert!(groups(&f, &[]).is_empty() && groups(&Family::default(), &picked).is_empty());
        // A mesh a file brought is one, solid or void.
        let mut meshed = f.clone();
        meshed.forms[2].mesh = std::sync::Arc::new(vec![[[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]]);
        assert_eq!(kind(&meshed, Picked::Form(2)), Some(Kind::Mesh));
        assert_eq!(kind(&f, Picked::Form(2)), Some(Kind::Void(FormKind::Extrusion)));
        assert_eq!(kind(&f, Picked::Form(9)), None);
    }

    #[test]
    fn a_part_is_found_by_its_id_or_its_guid_however_written_and_what_names_none_is_said() {
        let f = family();
        let ifc = crate::ifc::compress(g(LINE).as_u128());
        let text = format!("123456, {EDGE};{}\n{{{DIM}}} {ifc} nowhere 100099 123456 0", SOLID.replace('-', "").to_uppercase());
        assert_eq!(
            find(&f, &text),
            Found {
                picked: vec![Picked::Form(0), Picked::Plane(g(EDGE)), Picked::Dimension(g(DIM)), Picked::Line(g(LINE))],
                missing: vec!["nowhere".into(), "100099".into(), "0".into()],
            },
            "each once, in the order named; an id of none (0) names nothing"
        );
        assert_eq!(find(&f, "123457").picked, [Picked::Plane(g(EDGE))], "a plane by its id");
        assert_eq!(find(&f, " \n,, ;"), Found::default());
        // What has neither an id nor a GUID yet is found by nothing: the nil GUID names none of them.
        assert_eq!(find(&f, "00000000-0000-0000-0000-000000000000").picked, []);
        // A hostile list is cut short, never a hang or a panic.
        let many = (0..2 * crate::inquiry::MAX_NAMES).map(|i| i.to_string()).collect::<Vec<_>>().join(" ");
        assert_eq!(find(&f, &many).missing.len(), crate::inquiry::MAX_NAMES);
        assert!(find(&f, &"é".repeat(crate::inquiry::MAX_LIST)).picked.is_empty());
    }

    #[test]
    fn a_part_is_named_by_its_id_and_its_guid_or_by_neither_until_it_is_given_them() {
        let f = family();
        let n = names(&f, Picked::Form(0)).unwrap();
        assert_eq!((n.id, n.guid, n.kind), (123_456, g(SOLID), Kind::Form(FormKind::Extrusion)));
        let n = names(&f, Picked::Symbol(0)).unwrap();
        assert!(crate::ids::unset(&n.id) && n.guid.is_nil(), "made since the family was loaded: none yet");
        let n = names(&f, Picked::Line(g(LINE))).unwrap();
        assert_eq!((n.id, n.guid, n.kind), (0, g(LINE), Kind::ReferenceLine));
        assert_eq!(names(&f, Picked::Nested(7)), None);
        assert_eq!(all(&f).len(), 4 + 4 + 1 + 1 + 2 + 1 + 4, "the datums are planes too");
        assert!(all(&f).into_iter().all(|p| holds(&f, p) && names(&f, p).is_some()));
    }

    #[test]
    fn select_all_instances_takes_the_nested_instances_of_the_same_family_and_type_all_or_those_shown() {
        use crate::family::views::{Side, Which, nested_shown};
        use crate::graphics::Detail;
        let f = family();
        let values = f.values("T", &std::collections::BTreeMap::new()).unwrap();
        let picked = [Picked::Form(0), Picked::Nested(2)];
        assert_eq!(instances(&f, &picked, None), [Picked::Nested(0), Picked::Nested(2), Picked::Nested(3)], "all of the family's");
        // Visible in View: what the view in front shows. The one switched off is in none.
        let plan = nested_shown(&f, &values, Some(Which::Plan), Detail::Fine, 1200.0);
        assert_eq!(plan, [true, true, true, false]);
        assert_eq!(instances(&f, &picked, Some(&plan)), [Picked::Nested(0), Picked::Nested(2)]);
        assert_eq!(instances(&f, &[Picked::Nested(1)], Some(&plan)), [Picked::Nested(1)], "another type is another");
        assert_eq!(instances(&f, &[Picked::Nested(1), Picked::Nested(0)], None).len(), 4, "of each type selected");
        assert!(instances(&f, &[Picked::Form(0), Picked::Plane(g(EDGE))], None).is_empty(), "nothing with a type");
        assert!(instances(&f, &[Picked::Nested(9)], None).is_empty());
        assert!(instances(&f, &picked, Some(&[])).is_empty(), "what the view says nothing of isn't shown");
        // One whose Visibility Settings leave it out of Front and Back is shown in the plan, the sides and {3D}, not there.
        let mut f = f;
        f.nested_instances[2].visibility = Some(crate::family::Visibility { front_back: false, ..Default::default() });
        let front = nested_shown(&f, &values, Some(Which::Elevation(Side::Front)), Detail::Fine, 1200.0);
        assert_eq!(front, [true, true, false, false]);
        assert_eq!(instances(&f, &picked, Some(&front)), [Picked::Nested(0)], "not the one Front and Back leave out");
        for which in [Some(Which::Plan), Some(Which::Elevation(Side::Left)), None] {
            assert_eq!(nested_shown(&f, &values, which, Detail::Fine, 1200.0), [true, true, true, false], "{which:?}");
        }
        // One drawn at Fine alone isn't shown at Coarse; one only shown when cut, only in a plan whose cut plane is in it.
        f.nested_instances[0].visibility = Some(crate::family::Visibility { coarse: false, medium: false, ..Default::default() });
        f.nested_instances[1].visibility = Some(crate::family::Visibility { only_when_cut: true, ..Default::default() });
        assert_eq!(nested_shown(&f, &values, None, Detail::Coarse, 1200.0), [false, false, true, false]);
        assert_eq!(nested_shown(&f, &values, Some(Which::Plan), Detail::Fine, 50.0), [true, true, true, false], "cut at 50 mm");
        assert_eq!(nested_shown(&f, &values, Some(Which::Plan), Detail::Fine, 1200.0), [true, false, true, false], "cut over it");
        // A family of symbols alone builds nothing: its instance is what a plan draws of its symbols, at the detail they
        // are drawn at (Medium and Fine unless said), and nothing in an elevation or {3D}.
        let flat = json!({"category": "generic", "types": {"M": {}}, "symbols": [{"line": [[0, 0], [100, 0]]}]});
        f.nested.push(serde_json::from_value(json!({"name": "Mark", "family": flat})).unwrap());
        f.nested_instances.push(serde_json::from_value(json!({"family": "Mark", "type": "M", "at": [0, 2000]})).unwrap());
        assert!(f.instance_pieces(4, &values).is_empty() && f.instance_marked(4, &values, Detail::Fine));
        assert!(nested_shown(&f, &values, Some(Which::Plan), Detail::Fine, 1200.0)[4]);
        assert!(!nested_shown(&f, &values, Some(Which::Plan), Detail::Coarse, 1200.0)[4]);
        assert!(!nested_shown(&f, &values, Some(Which::Elevation(Side::Front)), Detail::Fine, 1200.0)[4]);
        assert!(!nested_shown(&f, &values, None, Detail::Fine, 1200.0)[4]);
        let shown = nested_shown(&f, &values, Some(Which::Plan), Detail::Fine, 1200.0);
        assert_eq!(instances(&f, &[Picked::Nested(4)], Some(&shown)), [Picked::Nested(4)], "found by what the plan draws of it");
    }
}
