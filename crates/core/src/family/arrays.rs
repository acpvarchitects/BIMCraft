//! Bounded, parameter-driven placement of a linear or radial family array.
//! A placement is relative to the seed geometry; the seed itself is member zero.
use super::constraints::Transform;
use super::{Expr, Val};
use crate::adaptive::Frame;
use crate::view3d::{P3, dot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_ARRAY_MEMBERS: usize = 1000;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveTo {
    #[default]
    Second,
    Last,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArrayLayout {
    Linear { displacement: P3 },
    Radial { centre: P3, axis: P3, degrees: f64 },
}
impl Default for ArrayLayout {
    fn default() -> Self {
        Self::Linear { displacement: [100., 0., 0.] }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrayPattern {
    pub number: Expr,
    #[serde(default)]
    pub move_to: MoveTo,
    pub layout: ArrayLayout,
}
impl Default for ArrayPattern {
    fn default() -> Self {
        Self { number: Expr::Number(2.), move_to: MoveTo::Second, layout: ArrayLayout::default() }
    }
}
impl ArrayPattern {
    pub fn count(&self, values: &BTreeMap<String, Val>) -> Result<usize, String> {
        let number = crate::adaptive::read(&self.number, values)?;
        if number.fract() != 0. || !(2. ..=MAX_ARRAY_MEMBERS as f64).contains(&number) {
            return Err(format!("Array Number must be a whole number from 2 to {MAX_ARRAY_MEMBERS}"));
        }
        Ok(number as usize)
    }
    /// Returns every member including the seed, in order. Last divides the span into N-1 gaps.
    /// A complete radial circle divides by N instead, so the final member never duplicates the seed.
    pub fn placements(&self, values: &BTreeMap<String, Val>) -> Result<Vec<Frame>, String> {
        let count = self.count(values)?;
        let finite = |p: P3| p.iter().all(|v| v.is_finite() && v.abs() <= crate::model::MAX_COORD);
        match self.layout {
            ArrayLayout::Linear { displacement } => {
                if !finite(displacement) || dot(displacement, displacement) < 1e-12 {
                    return Err("Array displacement must be finite and nonzero".into());
                }
            }
            ArrayLayout::Radial { centre, axis, degrees } => {
                if !finite(centre)
                    || !finite(axis)
                    || dot(axis, axis) < 1e-12
                    || !degrees.is_finite()
                    || degrees.abs() < 1e-9
                    || degrees.abs() > 360.
                {
                    return Err("Radial array needs a finite centre, a nonzero axis and an angle up to 360 degrees".into());
                }
            }
        }
        let divisor = match (&self.layout, self.move_to) {
            (ArrayLayout::Radial { degrees, .. }, MoveTo::Last) if (degrees.abs() - 360.).abs() < 1e-9 => count as f64,
            (_, MoveTo::Last) => (count - 1) as f64,
            (_, MoveTo::Second) => 1.,
        };
        (0..count)
            .map(|index| {
                let factor = index as f64 / divisor;
                match self.layout {
                    ArrayLayout::Linear { displacement } => Frame::at(displacement.map(|v| v * factor)).checked(),
                    ArrayLayout::Radial { centre, axis, degrees } => {
                        let transform = Transform::Rotate { centre, axis, degrees: degrees * factor };
                        Frame {
                            origin: transform.point([0.; 3]),
                            x: transform.vector([1., 0., 0.]),
                            y: transform.vector([0., 1., 0.]),
                            z: transform.vector([0., 0., 1.]),
                        }
                        .checked()
                    }
                }
            })
            .collect()
    }
}

/// An associated array. Slot zero is authored; all trailing slots are rebuilt from it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyArray {
    #[serde(default, skip_serializing_if = "crate::ids::unset")]
    pub id: crate::ids::Id,
    #[serde(default, skip_serializing_if = "crate::ids::Guid::is_nil")]
    pub guid: crate::ids::Guid,
    pub members: Vec<crate::ids::Guid>,
    pub pattern: ArrayPattern,
    #[serde(default)]
    pub constrain: bool,
}
impl crate::library::HasPart for FamilyArray {
    fn ident(&mut self) -> (&mut crate::ids::Id, &mut crate::ids::Guid) {
        (&mut self.id, &mut self.guid)
    }
}
fn reguid<T: Serialize + serde::de::DeserializeOwned>(value: T, mapping: &BTreeMap<String, String>) -> Result<T, String> {
    fn rewrite(value: &mut serde_json::Value, map: &BTreeMap<String, String>) {
        match value {
            serde_json::Value::String(s) => {
                if let Some(next) = map.get(s) {
                    *s = next.clone();
                }
            }
            serde_json::Value::Array(a) => {
                for v in a {
                    rewrite(v, map);
                }
            }
            serde_json::Value::Object(o) => {
                for v in o.values_mut() {
                    rewrite(v, map);
                }
            }
            _ => {}
        }
    }
    let mut value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    rewrite(&mut value, mapping);
    serde_json::from_value(value).map_err(|e| e.to_string())
}
fn copy_message(error: super::constraints::CopyRefused, family: &super::Family) -> String {
    use super::constraints::CopyRefused;
    match error {
        CopyRefused::Datum(n) => format!("{} is a datum and cannot be copied", family.said(n)),
        CopyRefused::Origin(n) => format!("{} defines the origin and cannot be copied", family.said(n)),
        CopyRefused::Pivot(n) => format!("{} turns about a pivot and cannot be copied", family.said(n)),
    }
}
fn transform_message(error: super::constraints::TransformRefused) -> String {
    use super::constraints::{Stays, TransformRefused};
    match error {
        TransformRefused::Bad(message) => message,
        TransformRefused::Unsat(error) => error.to_string(),
        TransformRefused::Stays(Stays::Formula(_)) => {
            "A seed coordinate is controlled by a formula: associate the array or use free seed coordinates".into()
        }
        TransformRefused::Stays(Stays::Hosted(_)) => "The array leaves the seed's work plane: choose a matching view or axis".into(),
        TransformRefused::Stays(Stays::Mesh(_)) => "An imported triangle mesh has no editable sketch to array".into(),
        TransformRefused::Stays(Stays::Pivot(_)) => "The seed turns about a pivot and cannot be copied".into(),
        TransformRefused::Stays(Stays::Pinned(_)) => "The seed is pinned: unpin it before moving it".into(),
        TransformRefused::Stays(Stays::Datum(_) | Stays::Origin(_)) => "A datum or origin plane must remain in place".into(),
        TransformRefused::Nested(_) => "A nested instance must remain on its host".into(),
        TransformRefused::NotScaled(_) => "The seed form cannot be scaled".into(),
        TransformRefused::NotMirrored(_) => "The seed form cannot be mirrored".into(),
    }
}
impl super::Family {
    /// Replaces trailing real members from each seed, deterministically, without mutating the input.
    /// The scratch family has no arrays, so the native copy/transform solver cannot recurse into this rebuild.
    pub fn rebuild_arrays(&self, values: &BTreeMap<String, Val>) -> Result<Self, String> {
        self.rebuild_array_members(values, true)
    }
    /// Plain copies keep formulas and the native Copy/Transform refusal rules.
    pub fn plain_array_copies(&self, values: &BTreeMap<String, Val>) -> Result<Self, String> {
        self.rebuild_array_members(values, false)
    }
    fn rebuild_array_members(&self, values: &BTreeMap<String, Val>, resolved: bool) -> Result<Self, String> {
        use super::constraints::{Copying, Transforming};
        use crate::ids::Guid;
        let mut out = self.clone();
        let owned = |tag: Option<(Guid, u32)>| tag.is_some_and(|(g, slot)| slot > 0 && self.arrays.iter().any(|a| a.guid == g));
        let removed: std::collections::BTreeSet<Guid> = out
            .forms
            .iter()
            .filter(|f| owned(f.array))
            .flat_map(|f| std::iter::once(f.guid).chain(f.all_lines().map(|l| l.guid)))
            .collect();
        out.forms.retain(|f| !owned(f.array));
        out.nested_instances.retain(|n| !owned(n.array));
        out.dimensions.retain(|d| {
            !d.refs.iter().any(|r| match r {
                super::DimRef::Form(g, _) | super::DimRef::Plane(g) | super::DimRef::Line(g, _) | super::DimRef::LinePoint(g, _) => {
                    removed.contains(g)
                }
                super::DimRef::Sketch(g, _) | super::DimRef::Point(g, ..) => removed.contains(g),
            })
        });
        let mut used = std::collections::BTreeSet::new();
        let mut identities = std::collections::BTreeSet::new();
        for array in &self.arrays {
            if !identities.insert(array.guid) {
                return Err("Array GUIDs must be unique within the family".into());
            }
            if array.guid.is_nil() || array.guid.pending().is_some() || array.members.is_empty() {
                return Err("An associated array needs a persistent GUID and at least one seed member".into());
            }
            let placements = array.pattern.placements(values)?;
            if let Expr::Formula(label) = &array.pattern.number
                && self.parameters.iter().any(|p| p.name == *label && p.kind != super::Kind::Integer)
            {
                return Err("An array Number label must be an Integer parameter".into());
            }
            let mut source = out.clone();
            source.arrays.clear();
            let mut taking = Copying::default();
            for member in &array.members {
                if !used.insert(*member) {
                    return Err("A seed member can belong to only one array".into());
                }
                if let Some(i) = source.forms.iter().position(|f| f.guid == *member && f.array == Some((array.guid, 0))) {
                    taking.forms.push(i);
                } else if let Some(i) = source.nested_instances.iter().position(|n| n.guid == *member && n.array == Some((array.guid, 0))) {
                    taking.nested.push(i);
                } else {
                    return Err(format!("Array seed {member} is missing or no longer belongs to its array"));
                }
            }
            if out.forms.len() + taking.forms.len() * (placements.len() - 1) > super::MAX_FORMS
                || out.nested_instances.len() + taking.nested.len() * (placements.len() - 1) > super::nested::MAX_NESTED_INSTANCES
            {
                return Err("The array exceeds the family's form or nested-instance budget".into());
            }
            for (slot, placement) in placements.iter().enumerate().skip(1) {
                let copies = source.copies(&taking).map_err(|e| format!("Array member {}: {}", slot + 1, copy_message(e, &source)))?;
                let mut mapping = BTreeMap::new();
                let salt = (array.guid.as_u128() as u64) ^ ((array.guid.as_u128() >> 64) as u64) ^ (slot as u64).rotate_left(32);
                for (original, copy) in &copies.guids {
                    let g = original.as_u128();
                    mapping.insert(copy.to_string(), Guid::seeded(salt, (g as u64) ^ ((g >> 64) as u64)).to_string());
                }
                for (original, copy) in taking.nested.iter().filter_map(|i| source.nested_instances.get(*i)).zip(&copies.nested) {
                    let g = original.guid.as_u128();
                    mapping.insert(copy.guid.to_string(), Guid::seeded(salt, (g as u64) ^ ((g >> 64) as u64)).to_string());
                }
                for (index, dimension) in copies.dimensions.iter().enumerate() {
                    mapping.insert(dimension.guid.to_string(), Guid::seeded(salt, index as u64 ^ 0x617272617964696d).to_string());
                }
                let mut scratch = source.clone();
                scratch.forms = reguid(copies.forms, &mapping)?;
                scratch.nested_instances = reguid(copies.nested, &mapping)?;
                scratch.dimensions = reguid(copies.dimensions, &mapping)?;
                for form in scratch.forms.iter_mut().filter(|_| resolved) {
                    for line in form.all_lines_mut() {
                        for expr in line.exprs_mut() {
                            *expr = Expr::Number(crate::adaptive::read(expr, values)?);
                        }
                    }
                    form.from = Expr::Number(crate::adaptive::read(&form.from, values)?);
                    form.to = Expr::Number(crate::adaptive::read(&form.to, values)?);
                }
                let transform = match array.pattern.layout {
                    ArrayLayout::Linear { .. } => Transform::Translate { delta: placement.origin },
                    ArrayLayout::Radial { centre, axis, degrees } => {
                        let divisor = match array.pattern.move_to {
                            MoveTo::Second => 1.,
                            MoveTo::Last if (degrees.abs() - 360.).abs() < 1e-9 => placements.len() as f64,
                            MoveTo::Last => (placements.len() - 1) as f64,
                        };
                        Transform::Rotate { centre, axis, degrees: (degrees * slot as f64 / divisor) % 360. }
                    }
                };
                let transforming = Transforming { forms: (0..scratch.forms.len()).collect(), ..Default::default() };
                let transformed = scratch
                    .transformed(values, &transforming, &transform)
                    .map_err(|e| format!("Array member {}: {}", slot + 1, transform_message(e)))?;
                scratch = transformed.family;
                for nested in &mut scratch.nested_instances {
                    let host = source.frame(&nested.host)?;
                    let point = host.point(
                        crate::geom::P2(crate::adaptive::read(&nested.at[0], values)?, crate::adaptive::read(&nested.at[1], values)?),
                        crate::adaptive::read(&nested.offset, values)?,
                    );
                    let point = placement.point(point);
                    let (at, offset) = host.local(point);
                    let z = placement.vector(host.z);
                    if dot(z, host.z) < 1. - 1e-7 {
                        return Err("A radial nested-instance array must rotate about its host normal".into());
                    }
                    let angle = crate::adaptive::read(&nested.angle, values)?.to_radians();
                    let direction = placement.vector(std::array::from_fn(|i| host.x[i] * angle.cos() + host.y[i] * angle.sin()));
                    nested.at = [Expr::Number(at.0), Expr::Number(at.1)];
                    nested.offset = Expr::Number(offset);
                    nested.angle = Expr::Number(dot(direction, host.y).atan2(dot(direction, host.x)).to_degrees());
                }
                for form in &mut scratch.forms {
                    form.array = Some((array.guid, slot as u32));
                    if let Some(old) = self.forms.iter().find(|f| f.guid == form.guid) {
                        form.id = old.id;
                        let ids = old.all_lines().map(|l| (l.guid, l.id)).collect::<BTreeMap<_, _>>();
                        for line in form.all_lines_mut() {
                            if let Some(id) = ids.get(&line.guid) {
                                line.id = *id;
                            }
                        }
                    }
                }
                for nested in &mut scratch.nested_instances {
                    nested.array = Some((array.guid, slot as u32));
                    if let Some(old) = self.nested_instances.iter().find(|n| n.guid == nested.guid) {
                        nested.id = old.id;
                    }
                }
                for dimension in &mut scratch.dimensions {
                    if let Some(old) = self.dimensions.iter().find(|d| d.guid == dimension.guid) {
                        dimension.id = old.id;
                    }
                }
                if out.dimensions.len() + scratch.dimensions.len() > super::MAX_DIMENSIONS {
                    return Err("The array exceeds the dimension budget".into());
                }
                out.forms.extend(scratch.forms);
                out.nested_instances.extend(scratch.nested_instances);
                out.dimensions.extend(scratch.dimensions);
            }
        }
        Ok(out)
    }
}
