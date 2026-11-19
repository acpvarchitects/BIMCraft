//! Paint: a material laid over one face of an element, over what its type is made of: a wall's
//! exterior, a floor's top. It lives in the element's `params`, under [`KEY`], as `{face: material}`,
//! and shows in the 3D view, in the material's colour and texture.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::family::FormFace;
use crate::ids::Guid;
use crate::model::{Element, Model};

/// The key an element holds its paint under.
pub const KEY: &str = "Paint";

/// Whether `name` is the one an element holds its paint under: no project or family parameter may take it.
pub fn reserved(name: &str) -> bool {
    name == KEY
}

/// The faces an element of `category` has to paint, by name: a wall's two sides; a floor's, a roof's or a ceiling's top and
/// bottom; and of the other things a model is made of that hold values, all of it.
pub fn faces(category: &str) -> &'static [&'static str] {
    match category {
        "wall" => &["exterior", "interior"],
        "floor" | "roof" | "ceiling" => &["top", "bottom"],
        "column" | "structuralcolumn" | "footing" | "beam" | "stripfooting" | "pipe" | "duct" | "conduit" | "cabletray" | "wire"
        | "downconductor" | "groundconductor" | "cable" | "component" | "door" | "window" | "stair" | "ramp" | "railing" => &["all"],
        _ => &[],
    }
}

/// What an element's paint says, if `key` is where it's held: faces it has, each a material of the library.
pub(crate) fn check_value(m: &Model, who: &str, key: &str, v: &Value, e: &Element) -> Option<Result<(), String>> {
    (key == KEY).then(|| {
        let Some(painted) = v.as_object() else { return Err(format!("{who}: `{KEY}` is {{face: material}}, not {v}")) };
        let faces = faces(e.category());
        for (face, material) in painted {
            if !faces.contains(&face.as_str()) {
                let has = if faces.is_empty() { "none".to_string() } else { faces.join(", ") };
                return Err(format!("{who}: a {} has no face `{face}` to paint; its faces are {has}", e.category()));
            }
            if !material.as_str().is_some_and(|n| m.materials.contains_key(n)) {
                return Err(format!("{who}: its `{face}` is painted a material of the library, not {material}; add one with `material`"));
            }
        }
        Ok(())
    })
}

/// FE-16c-2: one face of a placed component painted in the project: the face `face` (its GEO-2b key) of form `form` of the
/// family the nested instances `nested` lead down to (by their GUIDs, from the component's family; none, the family's own
/// form), painted `material`, a material of the library (by its GUID in a file).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FacePaint {
    pub form: Guid,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nested: Vec<Guid>,
    pub face: FormFace,
    pub material: String,
}

/// The face of a placed component under a point ([`Model::component_face`]): its form, the nested instances down to it and
/// its key, as [`FacePaint`] names one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceAt {
    pub form: Guid,
    pub nested: Vec<Guid>,
    pub face: FormFace,
}

impl crate::family::Family {
    /// The family the nested instances `nested` lead down to from it (by their GUIDs, each its parent's instance, placing the
    /// family it names), and the place there of form `form`. None where a step or the form isn't there.
    pub fn form_at(&self, nested: &[Guid], form: Guid) -> Option<(&crate::family::Family, usize)> {
        let f = self.family_at(nested)?;
        let i = f.forms.iter().position(|x| x.guid == form && !form.is_nil())?;
        Some((f, i))
    }

    /// The family the nested instances `nested` lead down to from it; itself for none.
    pub fn family_at(&self, nested: &[Guid]) -> Option<&crate::family::Family> {
        let mut f = self;
        for g in nested {
            let inst = f.nested_instances.iter().find(|i| i.guid == *g && !g.is_nil())?;
            f = &f.nested_family(&inst.family)?.family;
        }
        Some(f)
    }

    /// How many of the nested instances `nested` lead through from the top are shared ones, each in a shared one before it:
    /// those are elements of their own in a project ([`crate::nested_elements`]).
    pub fn shared_steps(&self, nested: &[Guid]) -> usize {
        let mut f = self;
        for (k, g) in nested.iter().enumerate() {
            let Some(inst) = f.nested_instances.iter().find(|i| i.guid == *g && !g.is_nil()) else { return k };
            let Some(n) = f.nested_family(&inst.family).filter(|n| n.family.shared) else { return k };
            f = &n.family;
        }
        nested.len()
    }
}

/// Whether face paint `p` of a component of family `f` still names a face: its form there, by the path, a solid one that still
/// has the face ([`crate::family::Family::has_face`]). What a build makes hangs on the instance's values too, so a face only
/// some instances have keeps its paint, and paint on one none has paints nothing.
pub fn face_alive(f: &crate::family::Family, p: &FacePaint) -> bool {
    f.form_at(&p.nested, p.form).is_some_and(|(g, i)| !g.forms[i].void && g.has_face(i, &p.face))
}

/// The most faces of one component a project may paint.
pub const MAX_FACE_PAINT: usize = crate::family::MAX_PAINT;

/// Whether component `c`'s face paint (family `f`'s) is sound: at most [`MAX_FACE_PAINT`] entries, each face once, each a
/// form of the family by its path that is no void, each a material of the library.
pub(crate) fn check_faces(m: &Model, who: &str, c: &crate::model::Component, f: &crate::family::Family) -> Result<(), String> {
    if c.face_paint.is_empty() {
        return Ok(());
    }
    if c.face_paint.len() > MAX_FACE_PAINT {
        return Err(format!("{who}: it paints {} faces, more than {MAX_FACE_PAINT}", c.face_paint.len()));
    }
    let mut seen = std::collections::BTreeSet::new();
    for p in &c.face_paint {
        let face = serde_json::to_string(&p.face).unwrap_or_default();
        let said = format!("{who}: its paint on {face} of form {}", p.form);
        if !seen.insert((p.form, &p.nested, &p.face)) {
            return Err(format!("{said} is there twice; keep one"));
        }
        if p.material.trim().is_empty() {
            return Err(format!("{said} needs a material"));
        }
        if !m.materials.contains_key(&p.material) {
            return Err(format!("{said}: material `{}` isn't in the library; add it with `material`", p.material));
        }
        match f.form_at(&p.nested, p.form) {
            None => return Err(format!("{said}: family `{}` has no such form there; take the paint off", c.family)),
            Some((g, i)) if g.forms[i].void => {
                return Err(format!("{said}: the form is a void, never seen, so it takes no paint; take the paint off"));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

impl Model {
    /// The places in component `c`'s face paint of the entries that name no face its family has ([`face_alive`]) or a
    /// material the library hasn't; all of them where its family is gone.
    pub(crate) fn dead_face_paint(&self, c: &crate::model::Component) -> Vec<usize> {
        let f = self.families.get(&c.family);
        (0..c.face_paint.len())
            .filter(|i| {
                let p = &c.face_paint[*i];
                !(f.is_some_and(|f| face_alive(f, p)) && self.materials.contains_key(&p.material))
            })
            .collect()
    }
}

impl Model {
    /// Its components' face paint that names no face, or a material the library hasn't, taken off as a file is read, the
    /// components of its model groups' types too: what went, as JSON pointers into the file (`/elements/100004/face_paint/1`).
    pub(crate) fn drop_dead_face_paint_read(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        let dead: Vec<(crate::model::Id, Vec<usize>)> = self
            .elements
            .iter()
            .filter_map(|(id, e)| match e {
                Element::Component(c) if !c.face_paint.is_empty() => Some((*id, self.dead_face_paint(c))),
                _ => None,
            })
            .filter(|(_, d)| !d.is_empty())
            .collect();
        for (id, gone) in dead {
            if let Some(Element::Component(c)) = self.elements.get_mut(id) {
                drop_at(&mut c.face_paint, &gone);
            }
            out.extend(gone.iter().map(|i| format!("/elements/{id}/face_paint/{i}")));
        }
        let mut groups: Vec<(String, usize, Vec<usize>)> = Vec::new();
        for (name, t) in self.model_groups.iter() {
            for (i, member) in t.members.iter().enumerate() {
                if let Element::Component(c) = &member.element
                    && !c.face_paint.is_empty()
                {
                    let d = self.dead_face_paint(c);
                    if !d.is_empty() {
                        groups.push((name.clone(), i, d));
                    }
                }
            }
        }
        for (name, i, gone) in groups {
            if let Some(Element::Component(c)) = self.model_groups.get_mut(&name).and_then(|t| t.members.get_mut(i)).map(|x| &mut x.element)
            {
                drop_at(&mut c.face_paint, &gone);
            }
            let at = crate::lenient::escaped(&name);
            out.extend(gone.iter().map(|k| format!("/model_groups/{at}/members/{i}/element/face_paint/{k}")));
        }
        out
    }
}

/// `list` less the entries at places `gone` (in order).
fn drop_at<T>(list: &mut Vec<T>, gone: &[usize]) {
    let mut k = 0;
    list.retain(|_| {
        k += 1;
        !gone.contains(&(k - 1))
    });
}

/// The face paint of a file's component `e` (its JSON) that this version can't read (a face key or a field of a newer build's)
/// made an entry naming nothing, which the read then leaves out by its place in the list as the file had it.
pub fn unread_face_paint(e: &mut Value) {
    let Some(Value::Array(list)) = e.get_mut("face_paint") else { return };
    for p in list.iter_mut().filter(|p| serde_json::from_value::<FacePaint>((*p).clone()).is_err()) {
        *p = serde_json::json!({"form": Guid::default(), "face": "all", "material": ""});
    }
}

/// What each key of component `c`'s piece `piece` is painted, in step with `piece.keys`, as the 3D view and plans show it: the
/// project's paint of that face, else of the whole component (`all`), else its family's (FE-16c-1); none, its form's
/// material. Empty where nothing of it is painted.
pub fn piece_paint(c: &crate::model::Component, piece: &crate::family::Piece) -> Vec<Option<String>> {
    let all = c.params.get(KEY).and_then(|p| p.get("all")).and_then(Value::as_str);
    let own: Vec<&FacePaint> = if piece.form.is_nil() {
        Vec::new()
    } else {
        c.face_paint.iter().filter(|p| p.form == piece.form && p.nested == piece.path).collect()
    };
    if own.is_empty() && all.is_none() && piece.paint.is_empty() {
        return Vec::new();
    }
    piece
        .keys
        .iter()
        .enumerate()
        .map(|(k, key)| {
            own.iter()
                .find(|p| p.face == *key)
                .map(|p| p.material.clone())
                .or_else(|| all.map(str::to_string))
                .or_else(|| piece.paint.get(k).filter(|n| !n.is_empty()).cloned())
        })
        .collect()
}

/// Reply `v` of a command, with the face paint the engine took off after it (`dropped`, by component) said as `paint_dropped`:
/// `[{id, form, nested?, face}]`, so no paint goes unsaid.
pub(crate) fn say_dropped(mut v: Value, dropped: Vec<(crate::model::Id, FacePaint)>) -> Value {
    if let (false, Some(o)) = (dropped.is_empty(), v.as_object_mut()) {
        let said: Vec<Value> = dropped
            .into_iter()
            .map(|(id, p)| {
                let mut one = serde_json::json!({"id": id, "form": p.form, "face": p.face});
                if !p.nested.is_empty() {
                    one["nested"] = serde_json::json!(p.nested);
                }
                one
            })
            .collect();
        o.insert("paint_dropped".into(), Value::Array(said));
    }
    v
}
