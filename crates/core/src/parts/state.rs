//! What a part is now: its layer, its solid and what it measures, worked out from its host as the host stands, and
//! whether its recipe still applies. Nothing here is stored, so nothing here goes stale.

use serde_json::{Map, Value, json};

use super::solid::{self, Facing, Prism};
use super::{BuildingPart, Heritage, Source};
use crate::geom::{self, P2};
use crate::model::{Ceiling, Element, Function, Id, Key, Layer, Model, Variable, Wall};
use crate::view3d::P3;
use crate::walls::{self, Axis, Plane};

/// What a part is, as its host stands: found again each time it is asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct PartState {
    /// The layer it is, where it is one: its place in its host's type, counting from zero, and what the layer does.
    pub layer: Option<(usize, Function)>,
    /// What it is made of: its own material, else its layer's, else the material of its host's structure; empty for none.
    pub material: String,
    /// How thick it is, mm: its layer, or all of its host; none for a component, which has no thickness to name.
    pub thickness: Option<f64>,
    /// The area of its main face, mm², net of openings and holes ([`Model::part_quantities`]); none for a component.
    pub net_area: Option<f64>,
    /// How much it holds, mm³: its solid's volume.
    pub net_volume: f64,
    /// Why its recipe no longer applies to its host, and what to do about it; none while it does. In conflict, it has
    /// no solid: its area and volume are 0.
    pub conflict: Option<String>,
}

/// A part's solid before it is triangles: prisms side by side, or the triangles a family's forms are.
enum Shape {
    Prisms(Vec<Prism>, Facing),
    Mesh(Vec<[P3; 3]>),
}

impl Model {
    /// The parts of element `host`, in order of id: those made straight from it, and those divided from them.
    pub fn parts_of(&self, host: impl Key) -> Vec<Id> {
        let Some(guid) = self.elements.guid(host) else { return Vec::new() };
        let mut ids: Vec<Id> = self
            .lookup()
            .referrers(&guid)
            .filter(|id| matches!(self.elements.get(*id), Some(Element::Part(p)) if p.host == guid))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// The layers of the type element `e` is of, a wall's from its exterior in, the others' from the top down, with the
    /// type's category and name: none for what has no layered type (a component), empty for a type without layers.
    fn host_layers<'a>(&'a self, e: &'a Element) -> Option<(&'static str, &'a str, &'a [Layer])> {
        let (cat, name) = match e {
            Element::Wall(w) => ("wall", w.type_name.as_str()),
            Element::Floor(f) => ("floor", f.type_name.as_str()),
            Element::Roof(r) => ("roof", r.type_name.as_str()),
            Element::Ceiling(c) => ("ceiling", c.type_name.as_str()),
            _ => return None,
        };
        Some((cat, name, self.type_layers(cat, name)))
    }

    /// The layer part `p` is of, in its host's type as it is now: its place there, counting from zero, and the layer. None
    /// for a part of all of its host. An error, which says what to fix, when the recipe no longer applies: the host's type
    /// has no layer of that GUID (the type was swapped, or lost the layer), or the host is of a kind with no layers.
    pub fn part_layer(&self, p: &BuildingPart) -> Result<Option<(usize, &Layer)>, String> {
        let host = self.elements.get(p.host).ok_or("the element it is a part of is gone: delete it")?;
        let Source::Layer(guid) = p.source else { return Ok(None) };
        let who = format!("{} {}", host.category(), self.shown(p.host));
        let again = format!("delete it and make the {}'s parts again with `create_parts`", host.category());
        let Some((cat, name, layers)) = self.host_layers(host) else {
            return Err(format!("it is a layer, and {who} has no layers: {again}"));
        };
        match layers.iter().enumerate().find(|(_, l)| l.guid == guid) {
            Some(found) => Ok(Some(found)),
            // Where the layer is now, if the project still has it: in the type the host was of.
            None => match self.library.parts().find(|(_, part)| part.guid == guid) {
                Some((_, part)) => Err(format!(
                    "it is {}, and {who} is of {cat} type `{name}` now, which has no such layer: {again}, or give the {} its type back",
                    part.name(),
                    host.category()
                )),
                None => Err(format!("the layer it was made of is gone from {cat} type `{name}`, which {who} is of: {again}")),
            },
        }
    }

    /// What part `p` is made of: its own material, else its layer's; a part of all of its host, the material of the
    /// host's structure (its type's first structure layer with one, else its thickest with one). None where none is said.
    pub fn part_material<'a>(&'a self, p: &'a BuildingPart) -> Option<&'a str> {
        if let Some(own) = p.material.as_deref() {
            return Some(own);
        }
        let named = |l: &&Layer| !l.material.trim().is_empty();
        let of_layer = match self.part_layer(p).ok()? {
            Some((_, l)) => Some(l).filter(named),
            None => {
                let (_, _, layers) = self.host_layers(self.elements.get(p.host)?)?;
                let structure = layers.iter().filter(named).find(|l| l.function == Function::Structure);
                structure.or_else(|| layers.iter().filter(named).max_by(|a, b| a.thickness.total_cmp(&b.thickness)))
            }
        };
        of_layer.map(|l| l.material.as_str())
    }

    /// How thick part `p` is, mm: its layer's thickness, or all of its host's (a wall's across, a floor's, a roof's or a
    /// ceiling's from top to underside, as its type says). None for a component, or a part in conflict.
    pub fn part_thickness(&self, p: &BuildingPart) -> Option<f64> {
        if let Some((_, l)) = self.part_layer(p).ok()? {
            return Some(l.thickness);
        }
        let t = &self.types;
        match self.elements.get(p.host)? {
            Element::Wall(w) => Some(self.thickness(w)),
            Element::Floor(f) => t.floor.get(&f.type_name).map(|x| x.thickness),
            Element::Roof(r) => t.roof.get(&r.type_name).map(|x| x.thickness),
            Element::Ceiling(c) => t.ceiling.get(&c.type_name).map(|x| x.thickness),
            _ => None,
        }
    }

    /// Part `p`'s solid as its host stands: a closed surface of triangles in model coordinates (millimetres, x east, y
    /// north, z up; not its host's own axes), each triangle counter-clockwise seen from outside, so its normal points out
    /// of the solid. Every edge of a triangle is another's the other way round ([`solid::closed`]), and nothing lies
    /// inside: what `csg::subtract` takes, and what a division of the part starts from.
    ///
    /// A wall's part is its layer between the layer's two faces (all of the wall for [`Source::Whole`]), with the ends
    /// its joins give the wall (mitred, butted or square), its top and base where what it is attached to puts them, and
    /// its doors and windows cut through. A floor's, a roof's or a ceiling's is its layer between its two planes, less
    /// its openings and the shafts through it; over a variable layer the layers above follow a shaped top and those below
    /// stay level. A component's is its family's forms, as they are placed.
    ///
    /// An error when the part is in conflict ([`Model::part_layer`]) or has nothing to it (a layer of no thickness), which
    /// says what to fix. A part that was divided from another holds no cuts of its own yet: it is all of its source.
    pub fn part_solid(&self, p: &BuildingPart) -> Result<Vec<[P3; 3]>, String> {
        self.part_shape(p).map(|(tris, _)| tris)
    }

    /// The prisms part `p`'s solid is made of, side by side, never overlapping, each ring counter-clockwise or clockwise
    /// in plan as its host gives it: what [`Model::part_solid`] closes into one surface. None for a component's part,
    /// which is triangles from the start. For what divides a part in plan, where a prism is easier to cut than a mesh.
    pub fn part_prisms(&self, p: &BuildingPart) -> Result<Option<Vec<Prism>>, String> {
        Ok(match self.part_recipe(p)? {
            Shape::Prisms(prisms, _) => Some(prisms),
            Shape::Mesh(_) => None,
        })
    }

    /// Part `p`'s net volume, mm³, and the net area of its main face, mm² (none for a component, which has no layer to
    /// have a face of): what a bill of quantities for the works reads.
    ///
    /// Both are measured on its solid ([`Model::part_solid`]), so whatever shapes the solid is in them: the host's
    /// joins, its doors and windows, its openings and shafts, and once parts are divided, their cuts, gaps and offsets.
    /// The volume is the solid's. The area is that of the solid's faces that lie in the plane of the layer's own face
    /// and look the way it does: for a wall's part the face toward the wall's exterior side (its elevation, less the
    /// doors and windows), for a floor's, a roof's or a ceiling's part its top (less the openings and shafts; a sloped
    /// one's measured along its slope).
    pub fn part_quantities(&self, p: &BuildingPart) -> Result<(Option<f64>, f64), String> {
        let (tris, facing) = self.part_shape(p)?;
        Ok((facing.map(|f| solid::facing(&tris, &f)), solid::volume(&tris)))
    }

    /// The way part `p`'s main face looks: what [`solid::facing`] takes the area of on its solid, or on a division of
    /// it. None for a component's part.
    pub fn part_facing(&self, p: &BuildingPart) -> Result<Option<Facing>, String> {
        Ok(match self.part_recipe(p)? {
            Shape::Prisms(_, facing) => Some(facing),
            Shape::Mesh(_) => None,
        })
    }

    /// Why part `p`'s recipe no longer applies to its host, and what to do about it; none while it applies. The part of
    /// a part in conflict is in conflict with it.
    pub fn part_conflict(&self, p: &BuildingPart) -> Option<String> {
        self.part_shape(p).err()
    }

    /// What part `id` is now; none when `id` is no part.
    pub fn part_state(&self, id: impl Key) -> Option<PartState> {
        let Some(Element::Part(p)) = self.elements.get(id) else { return None };
        let layer = self.part_layer(p).ok().flatten().map(|(k, l)| (k, l.function));
        let material = self.part_material(p).unwrap_or_default().to_string();
        let thickness = self.part_thickness(p);
        Some(match self.part_quantities(p) {
            Ok((net_area, net_volume)) => PartState { layer, material, thickness, net_area, net_volume, conflict: None },
            Err(conflict) => PartState { layer, material, thickness, net_area: None, net_volume: 0.0, conflict: Some(conflict) },
        })
    }

    /// Part `p`'s solid and the way its main face looks.
    fn part_shape(&self, p: &BuildingPart) -> Result<(Vec<[P3; 3]>, Option<Facing>), String> {
        let (tris, facing) = match self.part_recipe(p)? {
            Shape::Prisms(prisms, facing) => (solid::mesh(&prisms), Some(facing)),
            Shape::Mesh(tris) => (tris, None),
        };
        if tris.is_empty() {
            let host = self.elements.get(p.host).map_or("element", Element::category);
            return Err(format!("there is nothing of it: {host} {} has no solid there; exclude it or delete it", self.shown(p.host)));
        }
        Ok((tris, facing))
    }

    /// What part `p` is of its host, as prisms or triangles: its recipe followed, or why it can't be.
    fn part_recipe(&self, p: &BuildingPart) -> Result<Shape, String> {
        // What it was divided from must hold, up to the part made straight from the host.
        let mut up = p.parent;
        let mut depth = 0;
        while let Some(g) = up {
            let Some(Element::Part(parent)) = self.elements.get(g) else {
                return Err(format!("the part it was divided from, {}, is gone: delete it", self.shown(g)));
            };
            if let Err(e) = self.part_layer(parent) {
                return Err(format!("part {}, which it was divided from, is in conflict: {e}", self.shown(g)));
            }
            depth += 1;
            if depth > super::MAX_DEPTH {
                return Err("it is divided from parts too deep to follow".into());
            }
            up = parent.parent;
        }
        let layer = self.part_layer(p)?;
        if let Some((k, l)) = layer
            && l.thickness <= 0.0
        {
            return Err(format!("layer {} of its type has no thickness, so there is nothing of it: exclude it or delete it", k + 1));
        }
        let host = self.elements.get(p.host).ok_or("the element it is a part of is gone: delete it")?;
        let id = self.elements.id(p.host).unwrap_or_default();
        let k = layer.map(|(k, _)| k);
        Ok(match host {
            Element::Wall(w) => {
                let axis = Axis::of(w);
                let facing = match axis.arc() {
                    // Its exterior side is to its left: turning left, toward the arc's centre.
                    Some((centre, _, _, sweep)) => Facing::Round { centre, inward: sweep > 0.0 },
                    None => {
                        let out = axis.tangent(0.0).perp();
                        Facing::Along(vec![[out.0, out.1, 0.0]])
                    }
                };
                Shape::Prisms(self.wall_prisms(id, w, k), facing)
            }
            Element::Floor(f) => {
                self.slab_shape(self.floor_prisms(f), self.type_layers("floor", &f.type_name), self.floor_under(f).map(|u| u.1), k)
            }
            Element::Roof(r) => {
                self.slab_shape(self.roof_prisms(r), self.type_layers("roof", &r.type_name), self.roof_under(r).map(|u| u.1), k)
            }
            Element::Ceiling(c) => self.slab_shape(self.ceiling_prisms(c), self.type_layers("ceiling", &c.type_name), None, k),
            Element::Component(c) => Shape::Mesh(self.component_pieces(c).into_iter().flat_map(|piece| piece.tris).collect()),
            other => return Err(format!("{} {id} has no parts", other.category())),
        })
    }

    /// Wall `id`'s prisms (`walls::prisms`, its doors and windows cut through), or with `layer`, what of each lies between
    /// that layer's two faces: across the wall from its exterior face in, as the 3D view cuts a wall into its layers, the
    /// outermost and the innermost taking whatever of the wall's ends lies past them.
    fn wall_prisms(&self, id: Id, w: &Wall, layer: Option<usize>) -> Vec<Prism> {
        let axis = Axis::of(w);
        let across = |q: P2| axis.project(q).1;
        let faces = walls::boundaries(self, w);
        let last = faces.len().saturating_sub(2);
        walls::prisms(self, id, true)
            .into_iter()
            .filter_map(|prism| {
                let mut ring = prism.ring;
                if let Some(k) = layer {
                    if let Some(outer) = faces.get(k).filter(|_| k > 0) {
                        ring = geom::clip(&ring, |q| outer - across(q));
                    }
                    if let Some(inner) = faces.get(k + 1).filter(|_| k < last) {
                        ring = geom::clip(&ring, |q| across(q) - inner);
                    }
                }
                (ring.len() >= 3 && geom::area(&ring).abs() > 1e-6).then_some(Prism { ring, bottom: prism.bottom, top: prism.top })
            })
            .collect()
    }

    /// A ceiling's slab as prisms, as a floor's are (`floor_prisms`): its outline less its openings, from its underside
    /// up through its type's thickness.
    fn ceiling_prisms(&self, c: &Ceiling) -> Vec<(Vec<P2>, Plane, Plane)> {
        self.ceiling_slabs(c)
            .into_iter()
            .filter_map(|(face, rise)| {
                let under = face.first()?[2];
                Some((face.iter().map(|q| P2(q[0], q[1])).collect(), Plane::flat(under), Plane::flat(under + rise)))
            })
            .collect()
    }

    /// A floor's, a roof's or a ceiling's `pieces` (each a ring in plan from its underside's plane up to its top's), less
    /// the shafts through them, or with `layer`, that layer of each: its type's `layers` from the top down, each its
    /// share of the way down, or under a `variable` layer, those above it following the top, those below it the level
    /// underside and the variable one between (`Variable::bounds`). The main face is the top: every plane a piece's top
    /// lies in.
    fn slab_shape(
        &self,
        pieces: Vec<(Vec<P2>, Plane, Plane)>,
        layers: &[Layer],
        variable: Option<Variable>,
        layer: Option<usize>,
    ) -> Shape {
        let mut prisms = Vec::new();
        for (ring, bottom, top) in pieces {
            // A shaft that reaches into the slab cuts all of it, so every layer of it: found on the whole slab.
            let rise = ring.iter().map(|q| top.at(*q) - bottom.at(*q)).fold(0.0, f64::max);
            let rings = self.less_shafts(&bottom.lift(&ring), rise).0;
            let (bottom, top) = match layer {
                None => (bottom, top),
                Some(k) => {
                    // How far down from the top each layer starts, where the top and the underside are `depth` apart.
                    let depth = top.z - bottom.z;
                    let starts = Variable::bounds(variable, layers, depth);
                    let at = |k: usize| {
                        let down = starts.get(k).copied().unwrap_or(depth);
                        match variable {
                            Some(v) if k > v.index => Plane { z: bottom.z + (depth - down), ..bottom },
                            _ => Plane { z: top.z - down, ..top },
                        }
                    };
                    (at(k + 1), at(k))
                }
            };
            prisms.extend(rings.into_iter().map(|ring| Prism { ring, bottom, top }));
        }
        let mut ways: Vec<P3> = Vec::new();
        for way in prisms.iter().map(|p| solid::normal(&p.top)) {
            if !ways.iter().any(|w| crate::view3d::dot(*w, way) >= 1.0 - 1e-12) {
                ways.push(way);
            }
        }
        Shape::Prisms(prisms, Facing::Along(ways))
    }
}

/// What a listing shows of part `id`, `p`, beside its own fields (`element_json`): the level it stands on, its host's
/// category and type, its layer's place in its host's type (from 1, as a type's layers are numbered) and what the layer
/// does, what it is made of, how thick it is, its net area and volume, what kind of part it is, the heritage set's values
/// under the names they are shown by ([`Heritage::NAMES`]), and its conflict, when it is in one. These are what a part's
/// values are read from by name (`schedule::fields` of `part`): by a view filter's rules, a schedule's fields, Filter on
/// a selection and a tag.
pub(crate) fn json(m: &Model, id: Id, p: &BuildingPart, extra: &mut Map<String, Value>) {
    let Some(state) = m.part_state(id) else { return };
    extra.insert("level".into(), json!(m.elements.get(id).and_then(|e| m.level_of(e))));
    let host = m.elements.get(p.host);
    extra.insert("host_category".into(), json!(host.map(Element::category)));
    let host_type = match host {
        Some(Element::Wall(w)) => Some(&w.type_name),
        Some(Element::Floor(f)) => Some(&f.type_name),
        Some(Element::Roof(r)) => Some(&r.type_name),
        Some(Element::Ceiling(c)) => Some(&c.type_name),
        Some(Element::Component(c)) => Some(&c.type_name),
        _ => None,
    };
    if let Some(name) = host_type {
        extra.insert("host_type".into(), json!(name));
    }
    // What kind of part it is, said even where it is none of IFC's: its own field is left out then.
    extra.insert("predefined_type".into(), json!(p.predefined_type.name()));
    if let Some((k, function)) = state.layer {
        extra.insert("layer".into(), json!(k + 1));
        extra.insert("function".into(), json!(function));
    }
    // Its own material is the field it holds; what it is made of, whichever says so, is shown under the same name.
    if !state.material.is_empty() {
        extra.insert("material".into(), json!(state.material));
    }
    extra.insert("own_material".into(), json!(p.material.is_some()));
    if let Some(t) = state.thickness {
        extra.insert("thickness".into(), json!(t));
    }
    // To the square centimetre and the cubic centimetre: a patch of plaster is small.
    if let Some(area) = state.net_area {
        extra.insert("area_m2".into(), json!((area / 1e2).round() / 1e4));
    }
    extra.insert("volume_m3".into(), json!((state.net_volume / 1e3).round() / 1e6));
    let h = &p.heritage;
    let [su, period, decay, severity, intervention] = Heritage::NAMES;
    for (name, text) in [(su, &h.su_code), (period, &h.historical_period), (intervention, &h.intervention_type)] {
        if !text.is_empty() {
            extra.insert(name.into(), json!(text));
        }
    }
    if let Some(d) = &h.decay_pathology {
        extra.insert(decay.into(), json!(d.label()));
    }
    if let Some(s) = h.decay_severity {
        extra.insert(severity.into(), json!(s));
    }
    if let Some(conflict) = state.conflict {
        extra.insert("conflict".into(), json!(conflict));
    }
}
