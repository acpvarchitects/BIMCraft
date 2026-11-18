//! Parts of elements read back (`parts.rs`): an element that is the sum of its parts in the file, each an
//! `IfcBuildingElementPart`, as BIMCraft writes one taken apart (and Revit one shown by its parts), came in with their body
//! as its own ([`Reader::items`]); here it is taken apart again, as `create_parts` would, and each part of the file is the
//! part that is where it is, with what the file says of it: its kind, what it is made of, the heritage set. A part BIMCraft
//! divided, which the file says how in [`crate::ifc::PART_DIVISION`], is divided so again, and its pieces are read so in
//! their turn. A part is made only where the file has one: one an export left out (excluded) stays out.

use std::collections::{HashMap, HashSet};

use super::{Links, PropertySets, Reader, clip, element_material, real, text};
use crate::model::{Element, Guid, Id, Model};
use crate::parts::{BuildingPart, Decay, Heritage, PartType, SEVERITY};
use crate::step::Entity;
use crate::view3d::P3;

/// What [`take_apart`] made and left out.
#[derive(Default)]
pub(super) struct Taken {
    /// Each part made, with the part of the file it was made of.
    pub made: Vec<(u64, Id)>,
    /// Each part made of a layer of its element's type, with the layer's place in it: the layer's GUID is the library's,
    /// which the import gives anew once everything is in ([`relayer`]).
    pub layers: Vec<(Id, usize)>,
    /// Each part of the file that didn't come in, and why.
    pub left: Vec<(u64, &'static str)>,
    /// The parts of the file made or left out so far: each is read once, whatever else a file says it is a part of.
    done: HashSet<u64>,
}

/// A box round points: its least and its greatest corner.
type Bounds = (P3, P3);

fn bounds(points: impl Iterator<Item = P3>) -> Option<Bounds> {
    let mut b: Option<Bounds> = None;
    for p in points.filter(|p| p.iter().all(|x| x.is_finite())) {
        let (lo, hi) = b.get_or_insert((p, p));
        *lo = std::array::from_fn(|k| lo[k].min(p[k]));
        *hi = std::array::from_fn(|k| hi[k].max(p[k]));
    }
    b
}

fn middle((lo, hi): Bounds) -> P3 {
    std::array::from_fn(|k| (lo[k] + hi[k]) / 2.0)
}

/// Whether `p` is in box `b`, a millimetre either way.
fn within(p: P3, (lo, hi): Bounds) -> bool {
    (0..3).all(|k| p[k] >= lo[k] - 1.0 && p[k] <= hi[k] + 1.0)
}

/// Each element of the file that is the sum of its parts and came in, taken apart again; `origin` is what each product of
/// the file came in as, `rels` the file's `IfcRelDefinesByProperties`, `library` what gives the library's entries their
/// GUIDs. An error only when the import runs out of its budget.
pub(super) fn take_apart(
    r: &Reader,
    l: &Links,
    m: &mut Model,
    origin: &HashMap<u64, Id>,
    rels: &[u64],
    library: &mut dyn FnMut(&Model, Option<&crate::library::Kind>, &str) -> Guid,
    budget: &crate::memory::Budget,
) -> Result<Taken, String> {
    let mut out = Taken::default();
    let mut wholes: Vec<u64> = l
        .parts
        .keys()
        .copied()
        .filter(|w| r.get(*w).is_some_and(|x| x.name != "IFCBUILDINGELEMENTPART" && r.parts_of(x).is_some()))
        .collect();
    if wholes.is_empty() {
        return Ok(out);
    }
    wholes.sort_unstable();
    // A part names its layer by the layer's GUID: the layers have theirs for now.
    m.sync_library_with(library);
    let sets = PropertySets::read(r, rels, &[Heritage::SET, crate::ifc::PART_DIVISION]);
    for whole in wholes {
        budget.check()?;
        let Some(x) = r.get(whole) else { continue };
        let parts = r.parts_of(x).unwrap_or_default();
        let host =
            origin.get(&whole).copied().filter(|id| m.elements.get(*id).is_some_and(|e| crate::parts::HOSTS.contains(&e.category())));
        let wanted = host.and_then(|h| crate::parts::missing(m, h).ok()).map(|(_, wanted)| wanted);
        let (Some(host), Some(wanted)) = (host, wanted) else {
            out.left.extend(parts.into_iter().map(|p| (p, "its element didn't come in, or has no parts BIMCraft can make")));
            continue;
        };
        // What points at an element points at its id until the import gives GUIDs.
        let wanted: Vec<BuildingPart> = wanted.into_iter().map(|p| BuildingPart { host: Guid::of_id(host), ..p }).collect();
        match_parts(r, l, &sets, m, &parts, wanted, 0, &mut out);
    }
    Ok(out)
}

/// The parts of the file `theirs` matched to the parts `ours` could make, each made where it holds the middle of one of
/// theirs, the nearest first, and said what the file says of it; and, where theirs is the sum of parts, divided again.
#[allow(clippy::too_many_arguments)]
fn match_parts(
    r: &Reader,
    l: &Links,
    sets: &PropertySets,
    m: &mut Model,
    theirs: &[u64],
    ours: Vec<BuildingPart>,
    depth: usize,
    out: &mut Taken,
) {
    let theirs: Vec<u64> = theirs.iter().copied().filter(|id| out.done.insert(*id)).collect();
    let shift = r.shift;
    let where_theirs: Vec<Option<Bounds>> = theirs
        .iter()
        .map(|id| r.get(*id).and_then(|x| bounds(r.body(x).into_iter().flatten().map(|p| [p[0] - shift.0, p[1] - shift.1, p[2]]))))
        .collect();
    let where_ours: Vec<Option<Bounds>> = ours.iter().map(|p| m.part_solid(p).ok().and_then(|t| bounds(t.into_iter().flatten()))).collect();
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (i, b) in where_theirs.iter().enumerate() {
        let Some(b) = b else { continue };
        let at = middle(*b);
        for (j, o) in where_ours.iter().enumerate() {
            if let Some(o) = o.filter(|o| within(at, *o)) {
                let mid = middle(o);
                pairs.push(((0..3).map(|k| (at[k] - mid[k]).powi(2)).sum::<f64>(), i, j));
            }
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let (mut matched, mut taken) = (vec![None; theirs.len()], vec![false; ours.len()]);
    for (_, i, j) in pairs {
        if matched[i].is_none() && !taken[j] {
            matched[i] = Some(j);
            taken[j] = true;
        }
    }
    for (i, &id) in theirs.iter().enumerate() {
        let (Some(j), Some(x)) = (matched[i], r.get(id)) else {
            out.left.push((id, "no part of its element BIMCraft can make is where it is"));
            continue;
        };
        let plain = ours[j].clone();
        let mut said = plain.clone();
        say(r, l, sets, m, id, x, &mut said);
        let part = m.insert(Element::Part(said.clone()));
        if crate::parts::check(m, &format!("part {part}"), part, &said).is_err() {
            m.elements.insert(part, Element::Part(plain.clone()));
            out.left.push((id, "what it says of itself breaks a rule of BIMCraft's: it came in as its layer's part, saying nothing"));
        }
        out.made.push((id, part));
        if let Ok(Some((k, _))) = m.part_layer(&plain) {
            out.layers.push((part, k));
        }
        if let Some(pieces) = r.parts_of(x) {
            divide(r, l, sets, m, id, x, part, &pieces, depth, out);
        }
    }
}

/// Part `part`, made of the file's part `id` (`x`), divided as the file says, and its pieces matched to the file's `pieces`;
/// all of them left out, and the part whole, where the file doesn't say how (another program's, or BIMCraft's divided by hand).
#[allow(clippy::too_many_arguments)]
fn divide(
    r: &Reader,
    l: &Links,
    sets: &PropertySets,
    m: &mut Model,
    id: u64,
    x: &Entity,
    part: Id,
    pieces: &[u64],
    depth: usize,
    out: &mut Taken,
) {
    let whole = || pieces.iter().map(|p| (*p, "BIMCraft divides a part by its cuts, which the file doesn't say: the part came in whole"));
    let Some(json) = sets.said(id, "Division").and_then(|p| text(p.args.get(2))) else {
        out.left.extend(whole());
        return;
    };
    let (Ok(recipe), Some(Element::Part(p))) = (serde_json::from_str::<crate::divide::Recipe>(json), m.elements.get(part).cloned()) else {
        out.left.extend(whole());
        return;
    };
    if recipe.cuts.len() > crate::divide::MAX_CUTS || recipe.offsets.len() > crate::divide::MAX_CUTS {
        out.left.extend(whole());
        return;
    }
    let Some(frame) = m.element_frame(p.host) else {
        out.left.extend(whole());
        return;
    };
    // In its own placement's axes, in millimetres: in the model, then in its host's own coordinates, as a part keeps it.
    let (f, shift) = (r.frame(x), r.shift);
    let model = recipe.moved(
        &|q| {
            let at = f.at(q);
            [at[0] - shift.0, at[1] - shift.1, at[2]]
        },
        &|d| f.dir(d),
    );
    let divided = BuildingPart { division: Some(frame.local_recipe(&model)), ..p.clone() };
    let made = m.part_solid(&divided).and_then(|solid| m.part_division(&divided, &solid));
    let (Ok(outcome), true) = (made, depth < crate::parts::MAX_DEPTH) else {
        out.left.extend(whole());
        return;
    };
    m.elements.insert(part, Element::Part(divided.clone()));
    let template = BuildingPart { parent: Some(Guid::of_id(part)), division: None, excluded: false, ..divided };
    let ours: Vec<BuildingPart> =
        outcome.pieces.into_iter().map(|piece| BuildingPart { piece: Some(piece.key), ..template.clone() }).collect();
    match_parts(r, l, sets, m, pieces, ours, depth + 1, out);
}

/// What the file says of its part `id` (`x`), said of `p`: its kind and object type, what it is made of where that isn't what
/// it would be, and its heritage set.
fn say(r: &Reader, l: &Links, sets: &PropertySets, m: &mut Model, id: u64, x: &Entity, p: &mut BuildingPart) {
    p.object_type = text(x.args.get(4)).map(clip).unwrap_or_default();
    // IFC2x3 has no kind of part.
    p.predefined_type = match x.args.get(8) {
        Some(crate::step::V::Enum(e)) => PartType::parse(e).unwrap_or_default(),
        _ => PartType::NotDefined,
    };
    if p.predefined_type == PartType::UserDefined && p.object_type.is_empty() {
        p.predefined_type = PartType::NotDefined;
    }
    if let Some(name) = element_material(r, l, id).filter(|n| !n.is_empty())
        && m.part_material(p) != Some(name.as_str())
    {
        m.materials.entry(name.clone()).or_default();
        p.material = Some(name);
    }
    let [su, period, decay, severity, intervention] = Heritage::NAMES;
    let value = |name: &str| sets.said(id, name).and_then(|v| v.args.get(2));
    let words = |name: &str| text(value(name)).map(clip).unwrap_or_default();
    p.heritage = Heritage {
        su_code: words(su),
        historical_period: words(period),
        decay_pathology: text(value(decay)).map(|t| Decay::named(t).unwrap_or_else(|| Decay::Custom(clip(t)))),
        decay_severity: real(value(severity))
            .filter(|s| s.fract() == 0.0 && *s >= 0.0 && *s <= 255.0)
            .map(|s| s as u8)
            .filter(|s| SEVERITY.contains(s)),
        intervention_type: words(intervention),
    };
}

/// Each part of a layer named by the layer's GUID, as the library now gives it: `layers` holds where in its host's type each
/// layer is ([`Taken::layers`]).
pub(super) fn relayer(m: &mut Model, layers: &[(Id, usize)]) {
    for &(id, k) in layers {
        let Some(Element::Part(p)) = m.elements.get(id) else { continue };
        let layer = match m.elements.get(p.host) {
            Some(Element::Wall(w)) => m.type_layers("wall", &w.type_name).get(k),
            Some(Element::Floor(f)) => m.type_layers("floor", &f.type_name).get(k),
            Some(Element::Roof(x)) => m.type_layers("roof", &x.type_name).get(k),
            Some(Element::Ceiling(c)) => m.type_layers("ceiling", &c.type_name).get(k),
            _ => None,
        };
        if let Some(g) = layer.map(|l| l.guid)
            && let Some(Element::Part(p)) = m.elements.get(id).cloned()
        {
            m.elements.insert(id, Element::Part(BuildingPart { source: crate::parts::Source::Layer(g), ..p }));
        }
    }
}

/// What the file's materials say of their weight and their heat, said of the project's materials of their names: IFC4's
/// `Pset_MaterialCommon` and `Pset_MaterialThermal`, IFC2x3's general and thermal properties, in kg/m³, W/(m·K) and
/// J/(kg·K) as an export names them.
pub(super) fn physics(r: &Reader, ids: &[u64], m: &mut Model) {
    let positive = |v: Option<f64>| v.filter(|x| x.is_finite() && *x > 0.0);
    for x in ids.iter().filter_map(|id| r.get(*id)) {
        let material = |at: usize| r.at(x.args.get(at)).filter(|e| e.name == "IFCMATERIAL").and_then(|e| text(e.args.first())).map(clip);
        let (name, density, conductivity, heat) = match x.name.as_str() {
            "IFCMATERIALPROPERTIES" => {
                let props: Vec<&Entity> = super::refs(x.args.get(2))
                    .into_iter()
                    .filter_map(|p| r.get(p))
                    .filter(|p| p.name == "IFCPROPERTYSINGLEVALUE")
                    .collect();
                let said =
                    |name: &str| positive(props.iter().find(|p| text(p.args.first()) == Some(name)).and_then(|p| real(p.args.get(2))));
                match text(x.args.first()) {
                    Some("Pset_MaterialCommon") => (material(3), said("MassDensity"), None, None),
                    Some("Pset_MaterialThermal") => (material(3), None, said("ThermalConductivity"), said("SpecificHeatCapacity")),
                    _ => continue,
                }
            }
            "IFCGENERALMATERIALPROPERTIES" => (material(0), positive(real(x.args.get(3))), None, None),
            "IFCTHERMALMATERIALPROPERTIES" => (material(0), None, positive(real(x.args.get(4))), positive(real(x.args.get(1)))),
            _ => continue,
        };
        if let Some(found) = name.and_then(|n| m.materials.get_mut(&n)) {
            found.density = density.or(found.density);
            found.conductivity = conductivity.or(found.conductivity);
            found.specific_heat = heat.or(found.specific_heat);
        }
    }
}
