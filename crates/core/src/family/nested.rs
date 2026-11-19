//! Nested families (T6a): families loaded into a family, and their instances placed in it, as Revit's Insert > Load Family and
//! Component do inside the Family Editor. A host carries each nested family's whole definition (`NestedFamily`), so a family
//! file holds what it nests; an instance (`NestedInstance`) places one of its types on a plane of the host, at a point of that
//! plane, an offset along its normal and a turn about it, its parameters set by numbers or formulas over the host's (Revit's
//! "=", Associate Family Parameter). The host's pieces and marks take the instances' own, so everything that reads
//! [`Family::pieces_at`] (plans, sections, the 3D view, IFC, `render`) shows them.
//!
//! A nested family's own `shared` option makes it shared: loading the host into a project (or into another family) loads its
//! shared nested families there too, asking about the ones the target has defined otherwise ("Shared Family Already Loaded"),
//! and one definition of a shared family is kept across the target. The instances are built into the host either way; a
//! shared instance's pieces say which it is (`Piece::nested`), and [`Family::shared_instances`] where each stands, which a
//! project makes an element of its own of for each component of the host (T6b, `crate::nested_elements`).
use super::*;
use crate::ids::{Guid, Id};
use crate::view3d::{dot, sub};

/// How many levels of nested families a project's family may hold under it.
pub const MAX_NESTING: usize = 5;
/// How many families one family may nest.
pub const MAX_NESTED: usize = 32;
/// How many nested instances one family may place.
pub const MAX_NESTED_INSTANCES: usize = 256;
/// How many nested definitions one family's tree may hold, at every depth.
pub const MAX_NESTED_TOTAL: usize = 128;
/// How many nested instances one build of a family may build, at every depth: what a hostile tree (a thousand instances of a
/// thousand instances) is stopped by.
pub const MAX_NESTED_BUILDS: usize = 2048;

/// A family loaded into another: its name there and its whole definition, carried inside the host.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestedFamily {
    #[serde(default, skip_serializing_if = "crate::ids::unset")]
    pub id: Id,
    #[serde(default, skip_serializing_if = "Guid::is_nil")]
    pub guid: Guid,
    pub name: String,
    pub family: Family,
}

/// An instance of a nested family placed in its host: a type of `family`, on the host's plane `host` (Ref. Level when none) at
/// `at` in that plane's axes, `offset` along its normal and turned `angle` degrees about it; `visible` whether it is there.
/// `params` sets the nested family's parameters: a number, or a formula over the host's parameters (`Width / 2`); a material,
/// a text or a URL parameter names a host parameter of its kind, or holds the material's name or the words.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestedInstance {
    #[serde(default, skip_serializing_if = "crate::ids::unset")]
    pub id: Id,
    #[serde(default, skip_serializing_if = "Guid::is_nil")]
    pub guid: Guid,
    /// Pinned where it is (FE-17, Pin and Unpin): a move, a rotation, a mirror, a scale, Align and Delete refuse it. A copy
    /// of it isn't.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    pub family: String,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub host: PlaneRef,
    #[serde(default)]
    pub at: [Expr; 2],
    #[serde(default)]
    pub offset: Expr,
    #[serde(default)]
    pub angle: Expr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<Expr>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Expr>,
    /// Its Visibility Settings (F8), over those of each of its family's forms and symbols.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<super::Visibility>,
    /// The host's family type parameter that picks its family and type (T24, the Options Bar's Label): where that holds one,
    /// it shows that family's type; where it holds none, its own `family` and `type`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl NestedInstance {
    /// Every formula of its own place: `at`, `offset`, `angle` and `visible` (its parameters' are [`Family::instance_exprs`]').
    pub fn place_exprs(&self) -> impl Iterator<Item = &Expr> {
        self.at.iter().chain([&self.offset, &self.angle]).chain(&self.visible)
    }
}

/// Which version of a shared nested family a target keeps when the family loaded has it otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Keep {
    /// The target's own, as it is: the family loaded takes it.
    #[default]
    Project,
    /// The one in the family loaded, overwriting the target's.
    Family,
}

/// What a build of nested instances may still cost.
#[derive(Debug, Default)]
pub struct Meter {
    builds: usize,
}

impl Meter {
    fn spend(&mut self) -> Result<(), String> {
        self.builds += 1;
        if self.builds > MAX_NESTED_BUILDS {
            Err(format!("its nested instances build more than {MAX_NESTED_BUILDS} instances, at every depth"))
        } else {
            Ok(())
        }
    }
}

/// Where an instance stands in its host: its origin and axes in the host's, and whether they are mirrored.
#[derive(Clone, Copy, Debug)]
struct Placed {
    origin: P3,
    x: P3,
    y: P3,
    z: P3,
}

impl Placed {
    fn point(&self, p: P3) -> P3 {
        std::array::from_fn(|k| self.origin[k] + self.x[k] * p[0] + self.y[k] * p[1] + self.z[k] * p[2])
    }

    /// Direction `v` of its axes in the host's.
    fn dir(&self, v: P3) -> P3 {
        std::array::from_fn(|k| self.x[k] * v[0] + self.y[k] * v[1] + self.z[k] * v[2])
    }

    fn right_handed(&self) -> bool {
        dot(cross(self.x, self.y), self.z) > 0.0
    }

    /// Whether it stands upright: its z the host's, so its plan marks are drawn in the host's plan.
    fn level(&self) -> bool {
        self.z[2] > 1.0 - 1e-9
    }
}

/// A nested instance worked out: its family, that family's values, and where it stands.
type Instanced<'a> = (&'a Family, BTreeMap<String, Val>, Placed);

/// A shared nested instance where its host places it (T6b): what a project makes an element of its own of, for each
/// component of the host. `path` names it by the GUIDs of the nested instances from the host's down to it (one for an
/// instance the host places; more for one placed in a shared family the host nests); `family` and `type_name` are its
/// family's name and type; `origin` and `axes` its family's axes in the host's (mirrored where `axes` are left-handed).
#[derive(Clone, Debug, PartialEq)]
pub struct SharedInstance {
    pub path: Vec<Guid>,
    pub family: String,
    pub type_name: String,
    pub origin: P3,
    pub axes: [P3; 3],
    /// Its family's plan marks, in the host's axes, where it stands upright on a level plane (and those inside it): what a
    /// plan picks one of symbols alone by.
    pub marks: Vec<Mark>,
}

/// The nested family named `name` in `list`.
pub fn find<'a>(list: &'a [NestedFamily], name: &str) -> Option<&'a NestedFamily> {
    list.iter().find(|n| n.name == name)
}

impl Family {
    /// The family it nests under `name`.
    pub fn nested_family(&self, name: &str) -> Option<&NestedFamily> {
        find(&self.nested, name)
    }

    /// The families it nests of category `of`, each with each of its types, in the order it nests them: what a family type
    /// parameter of that category may pick (T24).
    pub fn choices(&self, of: Category) -> Vec<(String, String)> {
        self.nested
            .iter()
            .filter(|n| n.family.category == of)
            .flat_map(|n| n.family.types.keys().map(|t| (n.name.clone(), t.clone())))
            .collect()
    }

    /// Whether `v` is a value family type parameter `p` may hold: none, or a family it nests of `p`'s category and one of that
    /// family's types (T24). Any other parameter's is its own to say.
    pub fn choice_ok(&self, p: &Parameter, v: &Value) -> Result<(), String> {
        if p.kind != Kind::FamilyType {
            return Ok(());
        }
        let val = read(p, v)?;
        let Some((f, t)) = val.chosen() else { return Ok(()) };
        let of = p.of.unwrap_or_default();
        let named = category_name(of);
        let fits = || {
            let names: Vec<&str> = self.nested.iter().filter(|n| n.family.category == of).map(|n| n.name.as_str()).collect();
            if names.is_empty() { format!("it nests no {named} family") } else { format!("its {named} families are {}", names.join(", ")) }
        };
        let Some(n) = self.nested_family(f) else {
            return Err(format!("parameter `{}` picks `{f}`, which the family doesn't nest ({})", p.name, fits()));
        };
        if n.family.category != of {
            return Err(format!(
                "parameter `{}` picks `{f}`, a family of {}: it picks a {named} family ({})",
                p.name,
                category_name(n.family.category),
                fits()
            ));
        }
        if !n.family.types.contains_key(t) {
            let types: Vec<&str> = n.family.types.keys().map(String::as_str).collect();
            return Err(format!("parameter `{}` picks `{f}`'s type `{t}`, which it hasn't (it has {})", p.name, types.join(", ")));
        }
        Ok(())
    }

    /// Its family type parameters' values (each one's default and each type's) and the families they may pick, sound: each
    /// none or a family of its category it nests, and a type of it ([`Family::choice_ok`]).
    pub(super) fn check_choices(&self) -> Result<(), String> {
        for p in self.parameters.iter().filter(|p| p.kind == Kind::FamilyType) {
            self.choice_ok(p, &p.value)?;
            for (t, set) in &self.types {
                if let Some(v) = set.get(&p.name) {
                    self.choice_ok(p, v).map_err(|e| format!("type `{t}`: {e}"))?;
                }
            }
        }
        Ok(())
    }

    /// The family and type nested instance `inst` shows for the host's `values`: what the parameter it is labelled with picks
    /// (T24), else its own.
    pub fn chosen<'a>(&self, inst: &'a NestedInstance, values: &'a BTreeMap<String, Val>) -> (&'a str, &'a str) {
        inst.label.as_ref().and_then(|l| values.get(l)).and_then(Val::chosen).unwrap_or((&inst.family, &inst.type_name))
    }

    /// The families nested instance `inst` may show: those of its label's category where it is labelled, else its own.
    pub(crate) fn instance_families(&self, inst: &NestedInstance) -> Vec<&NestedFamily> {
        match self.label_of(inst).and_then(|p| p.of) {
            Some(of) => self.nested.iter().filter(|n| n.family.category == of).collect(),
            None => self.nested_family(&inst.family).into_iter().collect(),
        }
    }

    /// Each family type parameter a nested instance was labelled with since `before` (the family as it was; none, all of its
    /// labels are new) made to hold that instance's family and type where it holds none: its default, and a type that holds
    /// none outright. Labelling an instance leaves it showing what it showed (T24); a value taken back to none afterwards
    /// stays none, the instance showing its own. A label of another category's parameter, or of one a formula works out, fills
    /// nothing, so the check says what is wrong with the label.
    pub fn fill_labels(&mut self, before: Option<&Family>) {
        let had = |i: &NestedInstance| {
            !i.guid.is_nil() && before.is_some_and(|b| b.nested_instances.iter().any(|j| j.guid == i.guid && j.label == i.label))
        };
        let fills: Vec<(String, Value)> = self
            .nested_instances
            .iter()
            .filter(|i| !had(i))
            .filter(|i| {
                let category = self.nested_family(&i.family).map(|n| n.family.category);
                self.label_of(i).is_some_and(|p| p.formula.is_none() && p.of.is_some() && p.of == category)
            })
            .filter_map(|i| i.label.as_ref().map(|l| (l.clone(), serde_json::json!({"family": i.family, "type": i.type_name}))))
            .collect();
        for (l, v) in fills {
            let Some(p) = self.parameters.iter_mut().find(|p| p.name == l) else { continue };
            if p.value.is_null() {
                p.value = v.clone();
            }
            if !p.instance {
                for held in self.types.values_mut().filter_map(|set| set.get_mut(&l)) {
                    if held.is_null() {
                        *held = v.clone();
                    }
                }
            }
        }
    }

    /// The family type parameter nested instance `inst` is labelled with, where it is one.
    fn label_of(&self, inst: &NestedInstance) -> Option<&Parameter> {
        let l = inst.label.as_ref()?;
        self.parameters.iter().find(|p| p.name == *l && p.kind == Kind::FamilyType)
    }

    /// Instance `inst`'s parameter values worked out over the host's `values`: the nested family's parameter, its kind, and
    /// what it comes to. A text-like parameter takes a host parameter's words where it names one, else its own; a family type,
    /// the host's family type parameter's choice. A labelled instance's parameters its family hasn't (another of those its
    /// label may pick has them) are left out.
    fn instance_seeds(
        &self,
        inst: &NestedInstance,
        nested: &Family,
        values: &BTreeMap<String, Val>,
    ) -> Result<BTreeMap<String, Val>, String> {
        let mut out = BTreeMap::new();
        let mut read = HashMap::new();
        for (name, e) in &inst.params {
            let Some(p) = nested.parameters.iter().find(|p| p.name == *name) else {
                if inst.label.is_some() {
                    continue;
                }
                return Err(format!("`{name}` isn't a parameter of nested family `{}`", inst.family));
            };
            let v = if p.kind == Kind::FamilyType {
                match e {
                    Expr::Formula(s) => match values.get(s.as_str()) {
                        Some(v @ Val::Choice { .. }) => v.clone(),
                        _ => Val::no_choice(),
                    },
                    Expr::Number(_) => Val::no_choice(),
                }
            } else if p.kind.text() {
                match e {
                    Expr::Formula(s) => match values.get(s.as_str()) {
                        Some(Val::Text(t)) if self.parameters.iter().any(|q| q.name == *s) => Val::Text(t.clone()),
                        _ => Val::Text(s.clone()),
                    },
                    Expr::Number(x) => Val::Text(x.to_string()),
                }
            } else {
                let x = number(e, values, &mut read).map_err(|e| format!("`{name}`: {e}"))?;
                Val::Number(if p.kind == Kind::Integer { x.round() } else { x })
            };
            out.insert(name.clone(), v);
        }
        Ok(out)
    }

    /// Nested instance `i`, worked out for the host's `values`: its family, that family's values for its type and its
    /// parameters, and where it stands; none where `visible` says it isn't there.
    fn placed_instance(&self, i: usize, values: &BTreeMap<String, Val>) -> Result<Option<Instanced<'_>>, String> {
        let inst = &self.nested_instances[i];
        let mut read = HashMap::new();
        if let Some(v) = &inst.visible
            && number(v, values, &mut read)? == 0.0
        {
            return Ok(None);
        }
        let (family, type_name) = self.chosen(inst, values);
        let nested = self.nested_family(family).ok_or_else(|| format!("it places `{family}`, which it doesn't nest"))?;
        let seeds = self.instance_seeds(inst, &nested.family, values)?;
        let wall = match values.get(WALL) {
            Some(Val::Number(t)) => *t,
            _ => DEFAULT_WALL,
        };
        let own = nested.family.values_seeded(type_name, &BTreeMap::new(), wall, &seeds).map_err(|e| format!("`{family}`: {e}"))?;
        let frame = self.frame(&inst.host).map_err(|e| format!("its host: {e}"))?;
        let (x_at, y_at) = (number(&inst.at[0], values, &mut read)?, number(&inst.at[1], values, &mut read)?);
        let offset = number(&inst.offset, values, &mut read)?;
        let angle = number(&inst.angle, values, &mut read)?;
        let (s, c) = angle.to_radians().sin_cos();
        let (x, y, z) = (add(mul(frame.x, c), mul(frame.y, s)), add(mul(frame.x, -s), mul(frame.y, c)), frame.z);
        // Its family stands by its origin at the point it is placed at (O1), as a component does.
        let origin =
            crate::hosting::Placement::by_origin(frame.point(P2(x_at, y_at), offset), [x, y, z], false, nested.family.origin()).origin;
        let placed = Placed { origin, x, y, z };
        Ok(Some((&nested.family, own, placed)))
    }

    /// Its nested instances' pieces, sized by `values` (the host's), where each stands in it.
    pub(super) fn nested_pieces(&self, values: &BTreeMap<String, Val>, meter: &mut Meter) -> Result<Vec<Piece>, String> {
        let mut out = Vec::new();
        for i in 0..self.nested_instances.len() {
            out.extend(self.instance_built(i, values, meter).map_err(|e| format!("nested instance {}: {e}", i + 1))?);
        }
        Ok(out)
    }

    /// Nested instance `i`'s pieces, sized by `values` (the host's), where it stands in the host flexed for them: what a view
    /// picks it by. Empty where it isn't there or doesn't build.
    pub fn instance_pieces(&self, i: usize, values: &BTreeMap<String, Val>) -> Vec<Piece> {
        let Ok(flexed) = self.flexed(values) else { return Vec::new() };
        flexed.instance_built(i, values, &mut Meter::default()).unwrap_or_default()
    }

    /// Nested instance `i`'s pieces, sized by `values`, in the host's axes.
    fn instance_built(&self, i: usize, values: &BTreeMap<String, Val>, meter: &mut Meter) -> Result<Vec<Piece>, String> {
        let mut out = Vec::new();
        if i >= self.nested_instances.len() {
            return Ok(out);
        }
        let Some((nested, own, placed)) = self.placed_instance(i, values)? else { return Ok(out) };
        meter.spend()?;
        let pieces = nested.pieces_metered(&own, None, meter)?;
        let mirrored = !placed.right_handed();
        for piece in pieces {
            let mut tris: Vec<[P3; 3]> = piece.tris.iter().map(|t| t.map(|p| placed.point(p))).collect();
            if mirrored {
                tris.iter_mut().for_each(|t| t.swap(1, 2));
            }
            let (bottom, top) = heights(&tris);
            let at = |ring: &[P2]| -> Vec<P2> { ring.iter().map(|p| placed.point([p.0, p.1, 0.0])).map(|q| P2(q[0], q[1])).collect() };
            let (outline, holes) = if placed.level() {
                (at(&piece.outline), piece.holes.iter().map(|h| at(h)).collect())
            } else {
                (crate::geom::hull(tris.iter().flatten().map(|p| P2(p[0], p[1])).collect()), Vec::new())
            };
            let vis = super::Visibility::both(piece.vis, self.nested_instances[i].visibility);
            // Part of this instance where its family is shared (T6b), and of what is shared inside it; else absorbed.
            let nested =
                if nested.shared { std::iter::once(self.nested_instances[i].guid).chain(piece.nested).collect() } else { Vec::new() };
            out.push(Piece {
                tris,
                caps: piece.caps,
                outline,
                holes,
                bottom,
                top,
                material: piece.material,
                round: piece.round,
                vis,
                nested,
                faces: piece.faces,
                keys: piece.keys,
                paint: piece.paint,
                form: piece.form,
                path: std::iter::once(self.nested_instances[i].guid).chain(piece.path).collect(),
            });
        }
        Ok(out)
    }

    /// Its shared nested instances, sized by `values` (the host's), where each stands in it flexed for them: those its own
    /// nested instances are where their family is shared, and those inside each of them, reached through shared ones only (a
    /// family that isn't shared keeps what it nests to itself), as Revit makes elements of them in a project. One whose
    /// `visible` leaves it out, or that doesn't work out, isn't there.
    pub fn shared_instances(&self, values: &BTreeMap<String, Val>) -> Vec<SharedInstance> {
        let mut out = Vec::new();
        self.shared_into(values, &mut out, &mut Meter::default(), MAX_NESTING + 1);
        out
    }

    fn shared_into(&self, values: &BTreeMap<String, Val>, out: &mut Vec<SharedInstance>, meter: &mut Meter, depth: usize) {
        if depth == 0 || self.nested_instances.is_empty() {
            return;
        }
        let Ok(flexed) = self.flexed(values) else { return };
        for (i, inst) in flexed.nested_instances.iter().enumerate() {
            let Ok(Some((nested, own, placed))) = flexed.placed_instance(i, values) else { continue };
            if !nested.shared || meter.spend().is_err() {
                continue;
            }
            let to = |p: P2| {
                let q = placed.point([p.0, p.1, 0.0]);
                P2(q[0], q[1])
            };
            let level = placed.level();
            let marks = if level { nested.marks(&own).unwrap_or_default().iter().map(|m| m.map(to)).collect() } else { Vec::new() };
            out.push(SharedInstance {
                path: vec![inst.guid],
                family: flexed.chosen(inst, values).0.to_string(),
                type_name: flexed.chosen(inst, values).1.to_string(),
                origin: placed.origin,
                axes: [placed.x, placed.y, placed.z],
                marks,
            });
            let mut inner = Vec::new();
            nested.shared_into(&own, &mut inner, meter, depth - 1);
            out.extend(inner.into_iter().map(|s| SharedInstance {
                path: std::iter::once(inst.guid).chain(s.path).collect(),
                origin: placed.point(s.origin),
                axes: s.axes.map(|a| placed.dir(a)),
                marks: if level { s.marks.iter().map(|m| m.map(to)).collect() } else { Vec::new() },
                ..s
            }));
        }
    }

    /// Its nested instances' plan marks, sized by `values`, for those that stand upright on a level plane.
    pub(super) fn nested_marks(
        &self,
        values: &BTreeMap<String, Val>,
        detail: Option<crate::graphics::Detail>,
        meter: &mut Meter,
    ) -> Result<Vec<Tagged>, String> {
        let mut out = Vec::new();
        for i in 0..self.nested_instances.len() {
            out.extend(self.instance_marks(i, values, detail, meter)?);
        }
        Ok(out)
    }

    /// Nested instance `i`'s plan marks, sized by `values`: its family's, where it stands upright on a level plane and, at
    /// `detail` if it says, its Visibility Settings draw there. None where its `visible` leaves it out.
    fn instance_marks(
        &self,
        i: usize,
        values: &BTreeMap<String, Val>,
        detail: Option<crate::graphics::Detail>,
        meter: &mut Meter,
    ) -> Result<Vec<Tagged>, String> {
        if let (Some(d), Some(v)) = (detail, self.nested_instances[i].visibility)
            && !v.at(d)
        {
            return Ok(Vec::new());
        }
        let who = |e: String| format!("nested instance {}: {e}", i + 1);
        let Some((nested, own, placed)) = self.placed_instance(i, values).map_err(who)? else { return Ok(Vec::new()) };
        if !placed.level() {
            return Ok(Vec::new());
        }
        meter.spend()?;
        let to = |p: P2| {
            let q = placed.point([p.0, p.1, 0.0]);
            P2(q[0], q[1])
        };
        Ok(nested.marks_metered(&own, detail, meter).map_err(who)?.into_iter().map(|(m, sub)| (m.map(to), sub)).collect())
    }

    /// Whether a plan at `detail` draws plan marks of nested instance `i`, sized by `values` (the host's), where it stands
    /// in the host flexed for them: what a plan shows of an instance whose family is of symbols alone.
    pub fn instance_marked(&self, i: usize, values: &BTreeMap<String, Val>, detail: crate::graphics::Detail) -> bool {
        let Ok(flexed) = self.flexed(values) else { return false };
        i < flexed.nested_instances.len()
            && flexed.instance_marks(i, values, Some(detail), &mut Meter::default()).is_ok_and(|marks| !marks.is_empty())
    }

    /// Whether it nests a family named `name`, at any depth.
    pub fn nests(&self, name: &str) -> bool {
        self.nested.iter().any(|n| n.name == name || n.family.nests(name))
    }

    /// The shared families nested in it, each once (the first met), reached through shared ones only: a family that isn't
    /// shared keeps what it nests to itself.
    pub fn shared_within(&self) -> Vec<(String, Family)> {
        let mut out: Vec<(String, Family)> = Vec::new();
        let mut stack: Vec<&NestedFamily> = self.nested.iter().rev().collect();
        let mut seen = 0;
        while let Some(n) = stack.pop() {
            seen += 1;
            if seen > MAX_NESTED_TOTAL * 4 {
                break;
            }
            if !n.family.shared {
                continue;
            }
            if !out.iter().any(|(name, _)| *name == n.name) {
                out.push((n.name.clone(), n.family.clone()));
            }
            stack.extend(n.family.nested.iter().rev());
        }
        out
    }

    /// Each shared family nested in it, at any depth, made one definition: the first copy met, going down its nested
    /// families in their order (each family before what it nests: the one [`Family::shared_within`] reports), every other
    /// copy rewritten to it, as Revit holds one definition of a shared family. Rewriting a copy may change a family that
    /// nests it, so the walk is repeated until nothing changes, at most as deep as nesting goes. Whether anything changed.
    pub fn unify_shared(&mut self) -> bool {
        let mut any = false;
        for _ in 0..=MAX_NESTING + 1 {
            let mut changed = false;
            for (name, _) in self.shared_within() {
                // Its first copy as it is now, after the names before it were made one.
                let Some((_, first)) = self.shared_within().into_iter().find(|(n, _)| *n == name) else { continue };
                changed |= self.replace_shared(&name, &first);
            }
            any |= changed;
            if !changed {
                break;
            }
        }
        any
    }

    /// Every shared family nested in it named `name`, at any depth, made `definition`: one definition of a shared family.
    /// Whether any was.
    pub fn replace_shared(&mut self, name: &str, definition: &Family) -> bool {
        replace_shared(&mut self.nested, name, definition, MAX_NESTING + 1)
    }

    /// Its parameters an instance's `params` name that hold words (a material, text, a URL): what is no formula there.
    pub fn instance_words<'a>(&self, inst: &'a NestedInstance) -> Vec<&'a str> {
        let families = self.instance_families(inst);
        inst.params
            .keys()
            .filter(|k| families.iter().any(|n| n.family.parameters.iter().any(|p| p.name == **k && p.kind.text())))
            .map(String::as_str)
            .collect()
    }

    /// Each formula instance `inst`'s parameters hold over the host's, but the words (a material, text, a URL), which name a
    /// host parameter or are words.
    pub fn instance_exprs<'a>(&self, inst: &'a NestedInstance) -> Vec<(&'a str, &'a Expr)> {
        let words = self.instance_words(inst);
        inst.params.iter().filter(|(k, _)| !words.contains(&k.as_str())).map(|(k, e)| (k.as_str(), e)).collect()
    }

    /// Each nested instance inside its nested definitions, at every depth, given a GUID of its own where it holds none or
    /// repeats one an instance before it in the same definition holds (a definition written by hand, or a file's from before):
    /// what the element of a shared one three levels down is named by (`nested_elements::guid_of`). Its own instances are the
    /// project's to give (`Library::sync_parts`). Whether any was given.
    pub fn give_inner_guids(&mut self) -> bool {
        let mut any = false;
        for n in &mut self.nested {
            any |= give_guids(&n.name, &mut n.family, MAX_NESTING + 1);
        }
        any
    }

    /// Whether its nested families and instances are sound, `chain` the names of the families it is nested in (its own
    /// first, where known) and `total` the definitions counted so far in the tree: names unique and none of the chain's (no
    /// family is loaded into itself), within the caps, each definition sound in turn; each instance of a family it nests and
    /// one of its types, on one of its planes, setting parameters that family has and may be set.
    pub(super) fn check_nested(
        &self,
        has_material: &dyn Fn(&str) -> bool,
        chain: &mut Vec<String>,
        total: &mut usize,
    ) -> Result<(), String> {
        if self.nested.len() > MAX_NESTED {
            return Err(format!("it nests more than {MAX_NESTED} families"));
        }
        if self.nested_instances.len() > MAX_NESTED_INSTANCES {
            return Err(format!("it places more than {MAX_NESTED_INSTANCES} nested instances"));
        }
        if !self.nested.is_empty() && chain.len() > MAX_NESTING {
            return Err(format!("its families are nested more than {MAX_NESTING} deep"));
        }
        for (i, n) in self.nested.iter().enumerate() {
            let who = format!("nested family `{}`", n.name.chars().take(40).collect::<String>());
            if n.name.trim().is_empty()
                || n.name.trim() != n.name
                || n.name.chars().count() > MAX_TEXT
                || n.name.chars().any(char::is_control)
            {
                return Err(format!("nested family {}: a name of 1 to {MAX_TEXT} characters on one line, no spaces round it", i + 1));
            }
            if self.nested[..i].iter().any(|m| m.name == n.name) {
                return Err(format!("{who}: two nested families have that name"));
            }
            if chain.contains(&n.name) {
                return Err(format!("{who}: a family can't be loaded into itself, nor into a family it nests"));
            }
            *total += 1;
            if *total > MAX_NESTED_TOTAL {
                return Err(format!("its tree holds more than {MAX_NESTED_TOTAL} nested families"));
            }
            // A shared family's instances name the elements a project makes of them (T6b): each by a GUID of its own.
            if n.family.shared {
                let mut seen = std::collections::BTreeSet::new();
                for (j, inst) in n.family.nested_instances.iter().enumerate() {
                    if !given(inst.guid) || !seen.insert(inst.guid) {
                        return Err(format!(
                            "{who}: nested instance {} has no GUID of its own ({}): load the family again, which gives it one",
                            j + 1,
                            inst.guid
                        ));
                    }
                }
            }
            chain.push(n.name.clone());
            let checked = n.family.check_in(has_material, chain, total);
            chain.pop();
            checked.map_err(|e| format!("{who}: {e}"))?;
        }
        let long = |e: &Expr| matches!(e, Expr::Formula(f) if f.chars().count() > MAX_FORMULA);
        for (i, inst) in self.nested_instances.iter().enumerate() {
            let who = format!("nested instance {}", i + 1);
            let n = self.nested_family(&inst.family).ok_or_else(|| {
                format!("{who}: it places `{}`, which the family doesn't nest", inst.family.chars().take(40).collect::<String>())
            })?;
            if !n.family.types.contains_key(&inst.type_name) {
                let names: Vec<&str> = n.family.types.keys().map(String::as_str).collect();
                return Err(format!(
                    "{who}: `{}` has no type `{}` (it has {})",
                    inst.family,
                    inst.type_name.chars().take(40).collect::<String>(),
                    names.join(", ")
                ));
            }
            self.frame(&inst.host).map_err(|e| format!("{who}: its host: {e}"))?;
            if inst.place_exprs().any(long) || inst.params.values().any(long) {
                return Err(format!("{who}: a formula of it is longer than {MAX_FORMULA} characters"));
            }
            // Its label (T24): a family type parameter of the host, of its family's category.
            if let Some(l) = &inst.label {
                let shown: String = l.chars().take(40).collect();
                let Some(q) = self.parameters.iter().find(|q| q.name == *l) else {
                    return Err(format!("{who}: it is labelled `{shown}`, which isn't one of the family's parameters"));
                };
                if q.kind != Kind::FamilyType {
                    return Err(format!("{who}: it is labelled `{shown}`, which is {}: a label is a family type parameter", q.kind.said()));
                }
                if q.of != Some(n.family.category) {
                    return Err(format!(
                        "{who}: it is labelled `{shown}`, a family type of {}, and `{}` is of {}: label it with one of its category",
                        category_name(q.of.unwrap_or_default()),
                        inst.family,
                        category_name(n.family.category)
                    ));
                }
            }
            // Its parameters: of any family its label may pick, the same kind in each that has one.
            let families = self.instance_families(inst);
            for (name, e) in &inst.params {
                let found: Vec<(&str, &Parameter)> = families
                    .iter()
                    .filter_map(|m| m.family.parameters.iter().find(|p| p.name == *name).map(|p| (m.name.as_str(), p)))
                    .collect();
                let Some(&(_, first)) = found.first() else {
                    return Err(format!("{who}: `{}` isn't a parameter of `{}`", name.chars().take(40).collect::<String>(), inst.family));
                };
                if let Some((other, q)) = found.iter().find(|(_, q)| q.kind != first.kind) {
                    return Err(format!(
                        "{who}: `{name}` is {} in `{}` and {} in `{other}`: what its label picks among holds it as one kind",
                        first.kind.said(),
                        found[0].0,
                        q.kind.said()
                    ));
                }
                for (family, p) in found {
                    self.instance_param_ok(&who, family, name, e, p, has_material)?;
                }
            }
        }
        Ok(())
    }

    /// Whether nested instance `who`'s parameter `name`, `p` of nested family `family`, may be set to `e` from the host.
    fn instance_param_ok(
        &self,
        who: &str,
        family: &str,
        name: &str,
        e: &Expr,
        p: &Parameter,
        has_material: &dyn Fn(&str) -> bool,
    ) -> Result<(), String> {
        if p.formula.is_some() {
            return Err(format!("{who}: `{name}` is worked out by a formula of `{family}`: it can't be set from here"));
        }
        if p.reporting {
            return Err(format!("{who}: `{name}` is a reporting parameter: it reads a dimension and is set by none"));
        }
        // A family type is associated with one of the host's of its category, whose every choice the nested family
        // nests too (T24): what the host picks, it picks among its own.
        if p.kind == Kind::FamilyType {
            let of = p.of.unwrap_or_default();
            let host = match e {
                Expr::Formula(s) => self.parameters.iter().find(|q| q.name == *s && q.kind == Kind::FamilyType && q.of == p.of),
                Expr::Number(_) => None,
            };
            let Some(q) = host else {
                let said = match e {
                    Expr::Formula(s) => s.chars().take(40).collect::<String>(),
                    Expr::Number(x) => x.to_string(),
                };
                return Err(format!(
                    "{who}: `{name}` is a family type of {}: associate it with a family type parameter of the host of that \
                     category, not `{said}`",
                    category_name(of)
                ));
            };
            let Some(nested) = self.nested_family(family) else { return Ok(()) };
            // What the nested family may pick, gathered once: each of the host's choices is looked up in it, not walked for.
            let theirs: std::collections::HashSet<(String, String)> = nested.family.choices(of).into_iter().collect();
            for (f, t) in self.choices(of) {
                if !theirs.contains(&(f.clone(), t.clone())) {
                    return Err(format!(
                        "{who}: `{name}` is associated with `{}`, which may pick `{f}` (type `{t}`), and `{family}` doesn't \
                         nest that: load it into `{family}` too, or leave it out",
                        q.name
                    ));
                }
            }
            return Ok(());
        }
        if p.kind.text() {
            let Expr::Formula(s) = e else {
                return Err(format!("{who}: `{name}` is {}: it takes words or a parameter's name, not a number", p.kind.said()));
            };
            if s.chars().count() > MAX_TEXT || s.chars().any(char::is_control) {
                return Err(format!("{who}: `{name}`: words of up to {MAX_TEXT} characters on one line"));
            }
            match self.parameters.iter().find(|q| q.name == *s) {
                Some(q) if q.kind != p.kind => {
                    return Err(format!(
                        "{who}: `{name}` is {} and `{s}` is {}: associate it with one of its kind",
                        p.kind.said(),
                        q.kind.said()
                    ));
                }
                Some(_) => {}
                None if p.kind == Kind::Material && !has_material(s) => {
                    return Err(format!("{who}: `{name}`: there's no material or material parameter `{s}`"));
                }
                None => {}
            }
        } else if let Expr::Number(x) = e
            && !(x.is_finite() && x.abs() <= MAX_COORD)
        {
            return Err(format!("{who}: `{name}` is {x}, not a usable number"));
        }
        Ok(())
    }

    /// Its nested families and instances drawn on the plane or line `guid`: `nested instance 2`.
    pub(super) fn nested_on(&self, guid: Guid) -> impl Iterator<Item = String> + '_ {
        self.nested_instances
            .iter()
            .enumerate()
            .filter(move |(_, n)| n.host.guid() == guid)
            .map(|(i, _)| format!("nested instance {}", i + 1))
    }
}

/// Whether `g` is a GUID a part may hold: neither none nor an id waiting to be one.
fn given(g: Guid) -> bool {
    !g.is_nil() && g.pending().is_none()
}

/// Definition `family`'s nested instances (it nested as `name`), and those inside the families it nests, at most `depth`
/// deep, each given a GUID of its own where it holds none or repeats one before it. The GUID given is made from `name` and
/// the instance's place, so a file read twice, and each copy of one shared definition, give the same. Whether any was given.
fn give_guids(name: &str, family: &mut Family, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let seed = name.bytes().fold(crate::model::mix(0x7432_3464_6e65_7374), |s, b| crate::model::mix(s ^ u64::from(b)));
    let mut any = false;
    let mut seen = std::collections::BTreeSet::new();
    let held: std::collections::BTreeSet<Guid> = family.nested_instances.iter().map(|i| i.guid).collect();
    for (i, inst) in family.nested_instances.iter_mut().enumerate() {
        if given(inst.guid) && seen.insert(inst.guid) {
            continue;
        }
        let mut n = i as u64;
        let mut g = Guid::seeded(seed, n);
        while !given(g) || held.contains(&g) || seen.contains(&g) {
            n = n.wrapping_add(1 << 32);
            g = Guid::seeded(seed, n);
        }
        seen.insert(g);
        inst.guid = g;
        any = true;
    }
    for n in &mut family.nested {
        any |= give_guids(&n.name, &mut n.family, depth - 1);
    }
    any
}

/// Every shared family in `list` named `name`, at most `depth` deep, made `definition`; whether any was.
fn replace_shared(list: &mut [NestedFamily], name: &str, definition: &Family, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let mut any = false;
    for n in list.iter_mut() {
        if n.name == name && n.family.shared {
            if n.family != *definition {
                // Its instances in this family were placed against its origin as it was: kept, so they stay put (O1).
                let placed = n.family.placed_origin;
                n.family = definition.clone();
                n.family.placed_origin = placed;
                any = true;
            }
        } else {
            any |= replace_shared(&mut n.family.nested, name, definition, depth - 1);
        }
    }
    any
}

/// What loading family `incoming`, named `name`, into a target holding `has` asks: whether the target has `name` defined
/// otherwise (Family Already Exists), and the shared families nested in it that the target has otherwise (Shared Family
/// Already Loaded).
pub fn conflicts(incoming: &Family, name: &str, has: &dyn Fn(&str) -> Option<Family>) -> (bool, Vec<String>) {
    let exists = has(name).is_some_and(|f| f != *incoming);
    let shared =
        incoming.shared_within().into_iter().filter(|(n, def)| n != name && has(n).is_some_and(|f| f != *def)).map(|(n, _)| n).collect();
    (exists, shared)
}

/// The shared families nested in `incoming` (a family loaded into a target holding `has`) made one definition each within it
/// ([`Family::unify_shared`]), then reconciled with the target's, as `answers` say for each the target has otherwise (none:
/// the target's own): `Keep::Project` gives `incoming` the target's,
/// `Keep::Family` overwrites the target's with `incoming`'s, its values too (as `version_and_values` does, keeping the
/// target's own types and part identities). What the target
/// takes: each shared family it gains or has overwritten, by name; and the names it was asked about.
pub fn reconcile_shared(
    incoming: &mut Family,
    name: &str,
    has: &dyn Fn(&str) -> Option<Family>,
    answers: &BTreeMap<String, Keep>,
) -> (Vec<(String, Family)>, Vec<String>) {
    let (mut takes, mut asked): (Vec<(String, Family)>, Vec<String>) = (Vec::new(), Vec::new());
    // Its copies of each shared family made one first (the first met), so what the target takes of one never brings back
    // another's copy that differs.
    incoming.unify_shared();
    for (s, def) in incoming.shared_within() {
        if s == name {
            continue;
        }
        match has(&s) {
            Some(own) if own != def => {
                asked.push(s.clone());
                match answers.get(&s).copied().unwrap_or_default() {
                    Keep::Project => {
                        incoming.replace_shared(&s, &own);
                    }
                    Keep::Family => {
                        let overwritten = crate::commands::family_reload(def, &own, false);
                        incoming.replace_shared(&s, &overwritten);
                        takes.push((s, overwritten));
                    }
                }
            }
            Some(_) => {}
            None => takes.push((s, def)),
        }
    }
    // A shared family taken from the target (Keep::Project) or overwritten may be nested in another taken before it, whose
    // definition was read before the change: each taken as `incoming` holds it once all are made, so every definition the
    // target takes holds the same copies, in whatever order they are applied.
    incoming.unify_shared();
    let now = incoming.shared_within();
    for (s, def) in &mut takes {
        if let Some((_, current)) = now.iter().find(|(n, _)| n == s) {
            def.clone_from(current);
        }
    }
    (takes, asked)
}

/// What loading a family into a family did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nesting {
    /// It was there already, defined the same: nothing changed.
    pub unchanged: bool,
    /// It was there already, defined otherwise, and overwritten.
    pub overwritten: bool,
    /// The shared families nested in it the host had otherwise, which were asked about.
    pub shared_asked: Vec<String>,
    /// The shared families nested in it the host gained.
    pub shared_added: Vec<String>,
}

/// Family `incoming` loaded into the nested families `list` under `name`, as Revit's Insert > Load Family in a family does:
/// the shared families nested in it reconciled with `list`'s as `answers` say ([`reconcile_shared`]) and the ones `list`
/// hasn't added to it; then `incoming` itself, added, or over one of that name defined otherwise as `overwrite` says (`Some(true)`:
/// `version`, keeping `list`'s values where they fit; `Some(false)`: `version_and_values`), refused when it doesn't say. A
/// shared family is one definition across `list`, at every depth. `list` is unchanged on a refusal.
pub fn nest_into(
    list: &mut Vec<NestedFamily>,
    name: &str,
    mut incoming: Family,
    overwrite: Option<bool>,
    answers: &BTreeMap<String, Keep>,
) -> Result<Nesting, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a nested family needs a name".into());
    }
    if incoming.nests(name) {
        return Err(format!("`{name}` nests a family of its own name: a family can't be loaded into itself"));
    }
    // Its instances, and those of what it nests, each by a GUID of its own: what a project names their elements by (T6b).
    give_guids(name, &mut incoming, MAX_NESTING + 1);
    let mut report = Nesting::default();
    let at = list.iter().position(|n| n.name == name);
    if let Some(i) = at
        && list[i].family != incoming
        && overwrite.is_none()
    {
        return Err(format!(
            "it nests a family `{name}` already, defined otherwise: load it with `overwrite` `version` or `version_and_values`"
        ));
    }
    let mut work = list.clone();
    let has = |n: &str| find(&work, n).map(|f| f.family.clone());
    let (takes, asked) = reconcile_shared(&mut incoming, name, &has, answers);
    report.shared_asked = asked;
    for (s, def) in takes {
        match work.iter().position(|n| n.name == s) {
            Some(j) => work[j].family = Family { placed_origin: work[j].family.placed_origin, ..def.clone() },
            None => {
                report.shared_added.push(s.clone());
                work.push(NestedFamily { name: s.clone(), family: def.clone(), ..Default::default() });
            }
        }
        replace_shared(&mut work, &s, &def, MAX_NESTING + 1);
    }
    let now = match at {
        Some(i) if work[i].family == incoming => {
            report.unchanged = true;
            incoming
        }
        Some(i) => {
            let keep = overwrite.unwrap_or(true);
            let now = crate::commands::family_reload(incoming, &work[i].family, keep);
            work[i].family = now.clone();
            report.overwritten = true;
            now
        }
        None => {
            work.push(NestedFamily { name: name.to_string(), family: incoming.clone(), ..Default::default() });
            incoming
        }
    };
    if now.shared {
        replace_shared(&mut work, name, &now, MAX_NESTING + 1);
    }
    *list = work;
    Ok(report)
}

/// `p`, a point of family `f`, in the axes of its plane `host`: where on the plane it lies (x, y) and how far along its
/// normal, for placing an instance at a point picked in a view.
pub fn on_plane(f: &Family, host: &PlaneRef, p: P3) -> Option<(P2, f64)> {
    let frame = f.frame(host).ok()?;
    let d = sub(p, frame.origin);
    Some((P2(dot(d, frame.x), dot(d, frame.y)), dot(d, frame.z)))
}
