//! Parts of building elements, as Revit's Parts: a wall, a floor, a roof, a ceiling or a component taken apart into
//! pieces that are elements of their own, one for each layer of its type (or one for all of it), each with what it is
//! made of, what survey and restoration say of it, and its quantities. Aimed at the recording of historic buildings
//! (HBIM), where a wall is its plaster and its masonry, and each is described, dated and treated on its own.
//!
//! A part holds a recipe, never a shape: the element it is a part of (its `host`, by GUID) and what of it (`source`: a
//! layer of its type, by the layer's GUID, or all of it). Its solid, thickness, material, area and volume are worked out
//! from the host as it stands, each time they are asked for, so a part follows its host with nothing stored to go stale:
//! move the wall, stretch it, put a window in it or change its type's layers, and its parts are what they are of it now.
//! When the recipe no longer applies (the layer is gone from the host's type, the host's type was swapped), the part is
//! in conflict: it stays in the model, and says what to fix.
//!
//! A file holds parts among its other elements (category `part`), since format 21: a BIMCraft that reads up to 20 refuses
//! such a file as one of a newer BIMCraft, and so never opens it and drops its parts when it saves.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{Element, Guid, Id, MAX_TEXT, Model};

pub mod drawn;
pub mod solid;
mod state;

pub(crate) use state::json;
pub use state::{HostFrame, PartState, exported};

/// The categories of element that may have parts.
pub const HOSTS: [&str; 5] = ["wall", "floor", "roof", "ceiling", "component"];

/// How deep parts divided from parts may go.
pub const MAX_DEPTH: usize = 16;

/// What of its host a part is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// All of it: what a host whose type has no layers, or a component, is taken apart into.
    #[default]
    Whole,
    /// One layer of its type, by the layer's GUID (`Layer::guid`): it follows the layer's thickness, material and place
    /// in the type, and is in conflict once the host's type has no such layer.
    Layer(Guid),
}

impl Source {
    pub fn is_whole(&self) -> bool {
        *self == Source::Whole
    }
}

/// What kind of part it is, in IFC4's words for an `IfcBuildingElementPart` (its `PredefinedType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartType {
    /// A layer that insulates: what a thermal layer's part starts as.
    Insulation,
    /// A panel cast before it was put in place.
    PrecastPanel,
    /// None of IFC's kinds: its `object_type` says what it is (a stratigraphic unit, a patch of decay).
    UserDefined,
    #[default]
    NotDefined,
}

impl PartType {
    pub const ALL: [PartType; 4] = [PartType::Insulation, PartType::PrecastPanel, PartType::UserDefined, PartType::NotDefined];

    /// Its name in commands and files.
    pub fn name(self) -> &'static str {
        match self {
            PartType::Insulation => "insulation",
            PartType::PrecastPanel => "precastpanel",
            PartType::UserDefined => "userdefined",
            PartType::NotDefined => "notdefined",
        }
    }

    /// As IFC writes it: `INSULATION`, `PRECASTPANEL`, `USERDEFINED`, `NOTDEFINED`.
    pub fn ifc(self) -> &'static str {
        match self {
            PartType::Insulation => "INSULATION",
            PartType::PrecastPanel => "PRECASTPANEL",
            PartType::UserDefined => "USERDEFINED",
            PartType::NotDefined => "NOTDEFINED",
        }
    }

    /// By its name, in either case, as a command gives it or IFC writes it.
    pub fn parse(name: &str) -> Option<PartType> {
        let n = name.trim().to_ascii_lowercase();
        PartType::ALL.into_iter().find(|t| t.name() == n)
    }

    pub fn is_default(&self) -> bool {
        *self == PartType::NotDefined
    }
}

pub use crate::decay::Decay;

/// The lowest and highest a decay's severity goes: 1, slight, to 5, the worst.
pub const SEVERITY: std::ops::RangeInclusive<u8> = 1..=5;

/// What survey and restoration say of a part: the heritage set every part has, each value its own and none required.
/// A listing, a schedule and IFC name them [`Heritage::NAMES`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heritage {
    /// `SU_Code`: the code of the stratigraphic unit it is (a masonry unit, a covering or finish unit), as the survey
    /// numbers them.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub su_code: String,
    /// `Historical_Period`: when it was built, in the project's own names for its periods.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub historical_period: String,
    /// `Decay_Pathology`: how it has decayed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decay_pathology: Option<Decay>,
    /// `Decay_Severity`: how badly, 1 to 5 ([`SEVERITY`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decay_severity: Option<u8>,
    /// `Intervention_Type`: what is to be done to it (cleaning, consolidation, repointing, filling…).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub intervention_type: String,
}

impl Heritage {
    /// The set's name in IFC: a property set of BIMCraft's own.
    pub const SET: &'static str = "BIMCraft_Heritage";

    /// What its values are called where they are shown: in listings, in schedules and in IFC's property set.
    pub const NAMES: [&'static str; 5] = ["SU_Code", "Historical_Period", "Decay_Pathology", "Decay_Severity", "Intervention_Type"];

    /// Whether it says nothing.
    pub fn is_empty(&self) -> bool {
        *self == Heritage::default()
    }
}

/// A part of a building element: the element it is a part of and what of it, what overrides its material, what kind of
/// part IFC is to call it, and what survey and restoration say of it. Its shape and its quantities are its host's, worked
/// out when asked for (the module's notes say how).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingPart {
    /// The element it is a part of: a wall, a floor, a roof, a ceiling or a component ([`HOSTS`]). It goes when its host
    /// does.
    pub host: Guid,
    /// What of its host it is: one layer of its type, or all of it.
    #[serde(default, skip_serializing_if = "Source::is_whole")]
    pub source: Source,
    /// The part it was divided from; none, it was made straight from its host. It goes when that part does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Guid>,
    /// How it is divided (`divide_parts`): the cuts and offsets that make the parts divided from it, in its host's own
    /// coordinates ([`Model::element_frame`]), so they move and turn with the host. None, it is whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub division: Option<crate::divide::Recipe>,
    /// Which piece of its parent's division it is, by the cuts that made it; none for a part made straight from its host,
    /// or divided from its parent by hand (all of its source).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piece: Option<crate::divide::PieceKey>,
    /// What it is made of, where that isn't its layer's material: one of the project's library's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// What kind of part it is, as IFC names them.
    #[serde(default, skip_serializing_if = "PartType::is_default")]
    pub predefined_type: PartType,
    /// What it is, in the project's own words (`USM`, `Decay`): IFC's `ObjectType`, which a user-defined part must say.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub object_type: String,
    /// Left out of what is drawn, counted and exported, as Revit's Exclude Parts leaves one: it stays in the model, to be
    /// brought back.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub excluded: bool,
    /// What survey and restoration say of it.
    #[serde(default, skip_serializing_if = "Heritage::is_empty")]
    pub heritage: Heritage,
    /// Its values of the project's parameters, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, serde_json::Value>,
}

impl BuildingPart {
    /// A part of `host`, of `source` of it, with nothing else said.
    pub fn new(host: Guid, source: Source) -> BuildingPart {
        BuildingPart {
            host,
            source,
            parent: None,
            division: None,
            piece: None,
            material: None,
            predefined_type: PartType::NotDefined,
            object_type: String::new(),
            excluded: false,
            heritage: Heritage::default(),
            params: BTreeMap::new(),
        }
    }
}

/// The parts element `host` has that were made straight from it, and those it lacks: a part for each layer of its type
/// that has some thickness (a thermal one's starting as IFC's INSULATION), or one of all of it where its type has no
/// layers or it has no type of layers at all, as a component. An error for an element that has no parts to make, or
/// that has a part of all of it standing where its layers' parts would.
pub fn missing(m: &Model, host: Id) -> Result<(Vec<Id>, Vec<BuildingPart>), String> {
    let e = m.elements.get(host).ok_or_else(|| format!("no element {host}"))?;
    if !HOSTS.contains(&e.category()) {
        return Err(format!("{} {host} has no parts to make; a wall, a floor, a roof, a ceiling or a component has", e.category()));
    }
    let guid = m.guid(host);
    let layers: &[crate::model::Layer] = match e {
        Element::Wall(w) => m.type_layers("wall", &w.type_name),
        Element::Floor(f) => m.type_layers("floor", &f.type_name),
        Element::Roof(r) => m.type_layers("roof", &r.type_name),
        Element::Ceiling(c) => m.type_layers("ceiling", &c.type_name),
        _ => &[],
    };
    let mut wanted: Vec<BuildingPart> = layers
        .iter()
        .filter(|l| l.thickness > 0.0 && !l.guid.is_nil())
        .map(|l| BuildingPart {
            predefined_type: if l.function == crate::model::Function::Thermal { PartType::Insulation } else { PartType::NotDefined },
            ..BuildingPart::new(guid, Source::Layer(l.guid))
        })
        .collect();
    if wanted.is_empty() {
        wanted.push(BuildingPart::new(guid, Source::Whole));
    }
    let has: Vec<(Id, Source)> = m
        .parts_of(host)
        .into_iter()
        .filter_map(|id| match m.elements.get(id) {
            Some(Element::Part(p)) if p.parent.is_none() => Some((id, p.source)),
            _ => None,
        })
        .collect();
    let by_layer = wanted.iter().any(|w| !w.source.is_whole());
    if let Some((whole, _)) = has.iter().find(|(_, source)| by_layer && source.is_whole()) {
        return Err(format!(
            "{} {host} has a part of all of it, part {whole}, where its layers' parts would stand: delete that one first",
            e.category()
        ));
    }
    wanted.retain(|w| !has.iter().any(|(_, source)| *source == w.source));
    Ok((has.into_iter().map(|(id, _)| id).collect(), wanted))
}

/// What [`Model::check`] asks of part `id`, `p`: a host that may have parts, a parent that is another part of the same
/// host (and none of its own parts), a layer named by a GUID, a material the library has, a kind IFC can write, values of
/// the heritage set that are what they say, and no other part of the same thing. Whether its recipe still applies to its
/// host is no rule: that is a conflict, which the part reports and the model lives with.
pub(crate) fn check(m: &Model, who: &str, id: Id, p: &BuildingPart) -> Result<(), String> {
    let text = |what: &str, s: &str| {
        if s.chars().count() > MAX_TEXT { Err(format!("{who}: its {what} is longer than {MAX_TEXT} characters")) } else { Ok(()) }
    };
    match m.elements.get(p.host) {
        Some(e) if HOSTS.contains(&e.category()) => {}
        Some(e) => {
            return Err(format!(
                "{who}: {} {} has no parts; a wall, a floor, a roof, a ceiling or a component has",
                e.category(),
                m.shown(p.host)
            ));
        }
        None => return Err(format!("{who}: its host {} doesn't exist", m.shown(p.host))),
    }
    if let Source::Layer(g) = p.source
        && (g.is_nil() || g.pending().is_some())
    {
        return Err(format!("{who}: `source` names a layer by its GUID, as `create_parts` gives it, not {g}"));
    }
    // Up its parents to the part made straight from the host: each a part of the same host, none met twice.
    let me = m.guid(id);
    let mut at = p.parent;
    let mut depth = 0;
    while let Some(g) = at {
        if g == me {
            return Err(format!("{who}: it can't be divided from itself, nor from a part divided from it"));
        }
        let Some(Element::Part(up)) = m.elements.get(g) else {
            return Err(format!("{who}: its parent {} isn't a part", m.shown(g)));
        };
        if up.host != p.host {
            return Err(format!("{who}: its parent {} is a part of another element", m.shown(g)));
        }
        depth += 1;
        if depth > MAX_DEPTH {
            return Err(format!("{who}: parts divide from parts {MAX_DEPTH} deep at most"));
        }
        at = up.parent;
    }
    if p.piece.is_some() && p.parent.is_none() {
        return Err(format!("{who}: a piece of a division is divided from a part; it has no `parent`"));
    }
    if let Some(r) = &p.division
        && (r.cuts.len() > crate::divide::MAX_CUTS || r.offsets.len() > crate::divide::MAX_CUTS)
    {
        return Err(format!("{who}: a division has {} cuts and {0} offsets at most", crate::divide::MAX_CUTS));
    }
    if let Some(name) = &p.material {
        text("material", name)?;
        if name.trim().is_empty() {
            return Err(format!("{who}: a material has a name; leave `material` out to take its layer's"));
        }
        if !m.materials.contains_key(name) {
            return Err(format!("{who}: material `{name}` isn't in the library; add it with `material`"));
        }
    }
    text("object type", &p.object_type)?;
    if p.predefined_type == PartType::UserDefined && p.object_type.trim().is_empty() {
        return Err(format!("{who}: a user-defined part says what it is; give it an `object_type` (USM, Decay…)"));
    }
    let h = &p.heritage;
    text(Heritage::NAMES[0], &h.su_code)?;
    text(Heritage::NAMES[1], &h.historical_period)?;
    if let Some(Decay::Custom(own)) = &h.decay_pathology {
        text(Heritage::NAMES[2], own)?;
        if own.trim().is_empty() {
            return Err(format!("{who}: a decay of the project's own has a name; leave {} out for none", Heritage::NAMES[2]));
        }
    }
    if let Some(s) = h.decay_severity.filter(|s| !SEVERITY.contains(s)) {
        return Err(format!("{who}: {} is {} (slight) to {} (the worst), not {s}", Heritage::NAMES[3], SEVERITY.start(), SEVERITY.end()));
    }
    text(Heritage::NAMES[4], &h.intervention_type)?;
    // One part for each thing: two of the same would be counted, and exported, twice. Said the same whichever of the two
    // is asked: by the one of the lower id.
    let twin = m
        .lookup()
        .referrers(&p.host)
        .filter(|other| {
            *other != id
                && matches!(m.elements.get(*other), Some(Element::Part(o)) if o.host == p.host && o.source == p.source && o.parent == p.parent && o.piece == p.piece)
        })
        .min();
    if let Some(other) = twin {
        let host = m.elements.get(p.host).map_or("element", Element::category);
        let (first, second) = (id.min(other), id.max(other));
        return Err(format!("part {first}: part {second} is that of {host} {} too; delete one of them", m.shown(p.host)));
    }
    Ok(())
}
