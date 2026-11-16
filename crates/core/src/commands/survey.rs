//! The survey commands (HBIM): `survey`, a survey campaign of the project; `survey_area`, a survey area or a drawn item
//! on a face, made, changed or shown; and `faces`, an element's faces in their own coordinates, which outlines are
//! drawn in.

use super::*;
use crate::decay::Decay;
use crate::face::FaceId;
use crate::survey::{MAX_SURVEYS, Method, SEVERITY, SurveyArea, SurveyKind};

/// A survey campaign: added, changed, renamed or deleted; without `name`, the list.
pub(super) fn survey(s: &mut Session, p: &Value) -> R {
    let c = "survey";
    only(c, p, &["name", "date", "method", "rename", "delete"])?;
    let Some(name) = name_param(c, p, "name")? else {
        return Ok(json!({"surveys": s.model.surveys}));
    };
    let m = s.model_mut();
    if flag(c, p, "delete")? {
        if !m.surveys.contains_key(&name) {
            return Err(bad(c, format!("there's no survey `{name}`")));
        }
        if let Some((id, _)) = m.elements.iter().find(|(_, e)| matches!(e, Element::SurveyArea(a) if a.survey.as_deref() == Some(&name))) {
            return Err(bad(c, format!("survey `{name}` is what survey area {id} was found by: give that another survey (or none) first")));
        }
        m.surveys.remove(&name);
        return Ok(json!({"deleted": name}));
    }
    let was = m.surveys.get(&name).cloned();
    let rename = name_param(c, p, "rename")?;
    if was.is_none() && rename.is_some() {
        return Err(bad(c, format!("there's no survey `{name}` to rename")));
    }
    let mut v = was.clone().unwrap_or_default();
    if let Some(d) = string(c, p, "date")? {
        let d = d.trim().to_string();
        if !d.is_empty() && !crate::survey::is_date(&d) {
            return Err(bad(c, format!("`date` is a day as YYYY-MM-DD, or \"\" for none, not `{d}`")));
        }
        v.date = d;
    }
    match p.get("method") {
        None | Some(Value::Null) => {}
        Some(Value::String(t)) => {
            v.method = Method::named(t).ok_or_else(|| {
                bad(c, format!("`method` is visual, thermography or scan, or {{\"custom\": text}} for another, not `{t}`"))
            })?
        }
        Some(Value::Object(o)) if o.len() == 1 && o.get("custom").is_some_and(Value::is_string) => {
            let t = o.get("custom").and_then(Value::as_str).unwrap_or_default().trim().to_string();
            v.method = Method::named(&t).unwrap_or(Method::Custom(t));
        }
        Some(other) => return Err(bad(c, format!("`method` is visual, thermography, scan or {{\"custom\": text}}, not {other}"))),
    }
    let name = match rename {
        Some(to) if to != name => {
            if m.surveys.contains_key(&to) {
                return Err(bad(c, format!("there's already a survey `{to}`")));
            }
            m.surveys.remove(&name);
            rename_entry(m, &LibKind::Survey, &name, &to);
            to
        }
        _ => name,
    };
    v.check(&name).map_err(|e| bad(c, e))?;
    if !m.surveys.contains_key(&name) && m.surveys.len() >= MAX_SURVEYS {
        return Err(bad(c, format!("the project holds up to {MAX_SURVEYS} surveys")));
    }
    m.surveys.insert(name.clone(), v.clone());
    Ok(json!({"name": name, "survey": v}))
}

/// A face as a command gives it: a word or `{"side": i}`.
fn face_param(c: &str, v: &Value) -> Result<FaceId, String> {
    serde_json::from_value(v.clone())
        .map_err(|_| bad(c, format!("`face` is exterior, interior, start, end, top, bottom or {{\"side\": i}}, not {v}")))
}

/// A ring of `(u, v)` points under `key`, as [`points`] reads plan points.
fn rings(c: &str, p: &Value, key: &str) -> Result<Vec<Vec<P2>>, String> {
    let Some(a) = p.get(key).and_then(Value::as_array) else {
        return Err(bad(c, format!("`{key}` is a list of rings [[[u, v], …], …]")));
    };
    if a.len() > MAX_POINTS {
        return Err(bad(c, format!("`{key}` has more than {MAX_POINTS} rings")));
    }
    a.iter().map(|r| points(c, &json!({ key: r }), key)).collect()
}

/// A survey area or a drawn item made (with `host` and no `id`), changed (with `id`) or shown (`id` alone).
pub(super) fn survey_area(s: &mut Session, p: &Value) -> R {
    let c = "survey_area";
    only(
        c,
        p,
        &[
            "id",
            "host",
            "face",
            "outline",
            "holes",
            "item",
            "condition",
            "severity",
            "description",
            "intervention",
            "depth",
            "survey",
            "photos",
        ],
    )?;
    let m = &s.model;
    let id = match p.get("id") {
        v if absent(v) => None,
        Some(v) => Some(element_id(c, m, v)?),
        None => None,
    };
    let mut a = match id {
        Some(id) => match m.elements.get(id) {
            Some(Element::SurveyArea(a)) => a.clone(),
            Some(e) => return Err(bad(c, format!("{id} is a {}, not a survey area", e.category()))),
            None => return Err(bad(c, format!("no element {id}"))),
        },
        None => {
            let (Some(host), Some(face)) = (p.get("host").filter(|v| !v.is_null()), p.get("face").filter(|v| !v.is_null())) else {
                return Err(bad(c, "a new survey area needs `host` (a wall, a floor, a ceiling, a roof or a column) and its `face`"));
            };
            if absent(p.get("outline")) {
                return Err(bad(c, "a new survey area needs its `outline` [[u, v], …] in the face's own coordinates (see `faces`)"));
            }
            let host = m.guid(element_id(c, m, host)?);
            SurveyArea {
                host,
                face: face_param(c, face)?,
                kind: SurveyKind::Area,
                outline: Vec::new(),
                holes: Vec::new(),
                condition: None,
                severity: None,
                description: String::new(),
                intervention: String::new(),
                depth: None,
                survey: None,
                photos: Vec::new(),
                params: BTreeMap::new(),
            }
        }
    };
    let only_shown = id.is_some() && p.as_object().is_some_and(|o| o.len() == 1);
    if id.is_some() {
        if let Some(v) = p.get("host").filter(|v| !v.is_null()) {
            a.host = m.guid(element_id(c, m, v)?);
        }
        if let Some(v) = p.get("face").filter(|v| !v.is_null()) {
            a.face = face_param(c, v)?;
        }
    }
    if !absent(p.get("outline")) {
        a.outline = points(c, p, "outline")?;
    }
    if !absent(p.get("holes")) {
        a.holes = rings(c, p, "holes")?;
    }
    match p.get("item") {
        None => {}
        Some(Value::Null) => a.kind = SurveyKind::Area,
        Some(Value::String(t)) => a.kind = SurveyKind::Item(t.trim().to_string()),
        Some(v) => {
            return Err(bad(c, format!("`item` is a drawn item's category (stone, brick, painting…), or null for a survey area, not {v}")));
        }
    }
    match p.get("condition") {
        None => {}
        Some(Value::Null) => a.condition = None,
        Some(Value::String(word)) => {
            a.condition = Some(Decay::named(word).ok_or_else(|| {
                let words: Vec<String> = Decay::NAMED.iter().map(|d| d.label().to_ascii_lowercase()).collect();
                bad(c, format!("`condition` is one of {}, or {{\"custom\": text}} for another, not `{word}`", words.join(", ")))
            })?)
        }
        Some(Value::Object(o)) if o.len() == 1 && o.get("custom").is_some_and(Value::is_string) => {
            let own = o.get("custom").and_then(Value::as_str).unwrap_or_default().trim().to_string();
            a.condition = Some(Decay::named(&own).unwrap_or(Decay::Custom(own)));
        }
        Some(v) => return Err(bad(c, format!("`condition` is a word of the list, {{\"custom\": text}} or null, not {v}"))),
    }
    match p.get("severity") {
        None => {}
        Some(Value::Null) => a.severity = None,
        Some(v) => {
            let n = v.as_u64().and_then(|n| u8::try_from(n).ok()).filter(|n| SEVERITY.contains(n));
            a.severity = Some(n.ok_or_else(|| bad(c, format!("`severity` is a whole number from 1 (slight) to 5 (the worst), not {v}")))?);
        }
    }
    for (key, field) in [("description", &mut a.description), ("intervention", &mut a.intervention)] {
        match p.get(key) {
            None | Some(Value::Null) => {}
            Some(Value::String(t)) => *field = t.clone(),
            Some(v) => return Err(bad(c, format!("`{key}` is text, not {v}"))),
        }
    }
    match p.get("depth") {
        None => {}
        Some(Value::Null) => a.depth = None,
        Some(_) => a.depth = num(c, p, "depth")?,
    }
    match p.get("survey") {
        None => {}
        Some(Value::Null) => a.survey = None,
        Some(Value::String(t)) if t.trim().is_empty() => a.survey = None,
        Some(Value::String(t)) => {
            if !m.surveys.contains_key(t.trim()) {
                let names: Vec<&String> = m.surveys.keys().collect();
                return Err(bad(c, format!("there's no survey `{t}`; add it with `survey` (the project has {names:?})")));
            }
            a.survey = Some(t.trim().to_string());
        }
        Some(v) => return Err(bad(c, format!("`survey` is a survey's name, or null, not {v}"))),
    }
    match p.get("photos") {
        None => {}
        Some(Value::Null) => a.photos.clear(),
        Some(Value::Array(list)) if list.len() <= crate::survey::MAX_PHOTOS => {
            a.photos = list
                .iter()
                .map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(c, format!("`photos` are paths of files, not {v}"))))
                .collect::<Result<_, _>>()?;
        }
        Some(v) => {
            return Err(bad(
                c,
                format!("`photos` is a list of up to {} paths of files beside the project, not {v}", crate::survey::MAX_PHOTOS),
            ));
        }
    }
    let id = match id {
        Some(id) if only_shown => id,
        Some(id) => {
            if let Some(e) = s.model_mut().elements.get_mut(id) {
                *e = Element::SurveyArea(a);
            }
            id
        }
        None => s.model_mut().insert(Element::SurveyArea(a)),
    };
    let mut out = json!({"id": id});
    if let (Some(o), Some(Element::SurveyArea(a))) = (out.as_object_mut(), s.model.elements.get(id)) {
        for (k, v) in crate::survey::quantities_json(&s.model, a) {
            o.insert(k, v);
        }
    }
    Ok(out)
}

/// An element's faces, each with its outline and openings in its own `(u, v)` and its area; or one of them, with `face`,
/// and where `at` ([[u, v], …]) lies on it in space.
pub(super) fn faces(s: &mut Session, p: &Value) -> R {
    let c = "faces";
    only(c, p, &["id", "face", "at"])?;
    let m = &s.model;
    let id = element_id(c, m, p.get("id").ok_or_else(|| bad(c, "`id` (an element) is required"))?)?;
    let g = m.guid(id);
    let wanted: Vec<FaceId> = match p.get("face") {
        v if absent(v) => m.faces_of(g),
        Some(v) => vec![face_param(c, v)?],
        None => Vec::new(),
    };
    let at = if absent(p.get("at")) { Vec::new() } else { points(c, p, "at")? };
    let mut out = Vec::new();
    for f in wanted {
        let Some(ch) = m.face_chart(g, f) else {
            return Err(bad(c, format!("{} {id} has no {} face", m.elements.get(id).map_or("element", |e| e.category()), f.name())));
        };
        let points: Vec<Value> = at.iter().map(|q| json!({"uv": q, "point": ch.point(*q), "normal": ch.normal(*q)})).collect();
        out.push(json!({"face": f, "outline": ch.outline, "openings": ch.openings, "area": ch.area(), "at": points}));
    }
    Ok(json!({"id": id, "faces": out}))
}
