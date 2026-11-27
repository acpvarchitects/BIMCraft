//! Type objects and property sets of shared nested family occurrences.
use super::*;
use crate::family::{Family, Kind, Val};

const PARAMETERS: &str = "BIMCraft_FamilyParameters";
const IDENTITY: &str = "BIMCraft_NestedFamily";

/// Per-export fingerprints for independent, same-named embedded definitions. The immutable model
/// owns every referenced definition for the entire export. Its library assigns fresh part IDs to
/// loaded families, so those local bookkeeping fields are removed before comparing copies.
#[derive(Default)]
pub(super) struct Definitions(BTreeMap<*const Family, u64>);

impl Definitions {
    fn fingerprint(&mut self, f: &Family) -> u64 {
        *self.0.entry(std::ptr::from_ref(f)).or_insert_with(|| {
            let mut portable = f.clone();
            portable.forget_project_parts();
            serde_json::to_string(&portable).map(|s| key_of(&s)).unwrap_or_default()
        })
    }

    pub(super) fn variant(&mut self, f: &Family, project: Option<&Family>) -> Option<u64> {
        if project == Some(f) {
            return None;
        }
        let own = self.fingerprint(f);
        (project.map(|p| self.fingerprint(p)) != Some(own)).then_some(own)
    }
}

/// A single family parameter expressed in the IFC project's units.
pub(super) struct Property {
    name: String,
    measure: &'static str,
    value: String,
}

fn properties(f: &Family, values: &BTreeMap<String, Val>, type_only: bool) -> Vec<Property> {
    f.parameters
        .iter()
        .filter(|p| !type_only || !p.instance)
        .filter_map(|p| {
            let (measure, value) = match (p.kind, values.get(&p.name)?) {
                (Kind::Length, Val::Number(x)) => ("IFCLENGTHMEASURE", real(*x)),
                (Kind::Number, Val::Number(x)) => ("IFCREAL", real(*x)),
                (Kind::Angle, Val::Number(x)) => ("IFCPLANEANGLEMEASURE", ratio(x.to_radians())),
                (Kind::Area, Val::Number(x)) => ("IFCAREAMEASURE", ratio(x / 1e6)),
                (Kind::Volume, Val::Number(x)) => ("IFCVOLUMEMEASURE", ratio(x / 1e9)),
                (Kind::Integer, Val::Number(x)) => ("IFCINTEGER", (*x as i64).to_string()),
                (Kind::YesNo, Val::Number(x)) => ("IFCBOOLEAN", if *x == 0.0 { ".F." } else { ".T." }.into()),
                (Kind::Text | Kind::Material | Kind::Url, Val::Text(t)) => ("IFCTEXT", s(t)),
                (Kind::FamilyType, v) => ("IFCTEXT", s(&crate::family::said_choice(v))),
                _ => return None,
            };
            Some(Property { name: p.name.clone(), measure, value })
        })
        .collect()
}

impl Step {
    fn family_property_ids(&mut self, props: &[Property]) -> Vec<u64> {
        props.iter().map(|p| self.typed_property(&p.name, p.measure, &p.value)).collect()
    }

    /// The placed child's values can differ from the type defaults even for a type parameter bound by the host.
    pub(super) fn nested_properties(
        &mut self,
        ent: u64,
        id: Id,
        n: &crate::nested_elements::NestedElement,
        definition: Option<(&Family, &BTreeMap<String, Val>)>,
    ) {
        let props = definition.map(|(f, v)| properties(f, v, false)).unwrap_or_default();
        let (set, ids) = if props.is_empty() {
            (IDENTITY, vec![self.text_property("Family", None, &n.family), self.text_property("Type", None, &n.type_name)])
        } else {
            (PARAMETERS, self.family_property_ids(&props))
        };
        self.define(ent, set, &ids, (NESTED_PSET, NESTED_DEFINES), id);
    }

    /// Register a placed family or shared nested child against the same type table. Where this schema
    /// has no corresponding type class, classification belongs to the occurrence.
    pub(super) fn family_type(&mut self, f: &Family, name: &str, type_name: &str, ent: u64, nested: bool, variant: Option<u64>) {
        let (entity, kind, _) = f.ifc();
        let class = format!("{entity}TYPE");
        let omniclass = f.omniclass.as_ref().map(|o| (o.number.clone(), o.title.clone()));
        let attributes = classes::class(&entity).and_then(|c| c.2);
        if let Some(attributes) = attributes.filter(|_| self.schema != Schema::Ifc2x3 || v2x3::has_type(&class)) {
            let says = f.kind.is_some() || f.remembered().is_some();
            let predefined = if kind.is_empty() || says && kind != ".USERDEFINED." { &kind } else { ".NOTDEFINED." };
            let (before, after) = classes::around(attributes);
            let key = (class, before, after, format!("{name}:{type_name}"), None, Some((name.to_string(), variant)));
            let build = self.build(key, predefined);
            build.occurrences.push(ent);
            build.omniclass = omniclass;
            if nested && build.family_props.is_none() {
                build.family_props = Some(f.values(type_name, &BTreeMap::new()).map(|v| properties(f, &v, true)).unwrap_or_default());
            }
        } else if let Some(o) = omniclass {
            self.classified.entry(o).or_default().push(ent);
        }
    }

    pub(super) fn family_type_sets(&mut self, key: u64, props: &[Property]) -> String {
        if props.is_empty() {
            return "$".into();
        }
        let ids = self.family_property_ids(props);
        let set = self.property_set(PARAMETERS, &ids, NESTED_TYPE_PSET, key);
        format!("(#{set})")
    }
}
