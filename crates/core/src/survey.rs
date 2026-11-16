//! Survey areas and drawn items, for the recording of historic buildings (HBIM): a closed outline on one face of a wall,
//! a floor, a ceiling, a roof or a column, with what a condition survey says of it (its decay, how bad, what to do), or a
//! stone, a brick, a painting or an ornament drawn on a face to be counted. Both are one element, a [`SurveyArea`], with a
//! [`SurveyKind`].
//!
//! An area holds its host (by GUID), which face of it ([`FaceId`]) and its outline in that face's own coordinates, `(u, v)`
//! in millimetres ([`crate::face`]), never a shape in space: [`Model::survey_resolved`] works out where it is, what it
//! measures (on the true surface, developed on a curved wall), what the openings cutting it take out and the volume of
//! its depth, each time it is asked, so it follows its host moved, lengthened, retyped or thickened with nothing stored to
//! go stale. An outline that falls off its face, wholly or in part (the wall shortened under it, a side of a floor gone),
//! keeps its data and is in conflict ([`Resolved::conflict`]), never dropped. Drawing one changes nothing of its host.
//!
//! A survey (a campaign: its name, its date, its method) is a library entry of the project ([`Survey`], `Model::surveys`,
//! by name), which an area names: one campaign is shared by every area it found, it has an id and a GUID as any entry,
//! renaming it renames it everywhere, and a file holds an area's campaign by its GUID.
//!
//! Photos are links to files beside the project, never held in it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::decay::Decay;
use crate::face::{Chart, FaceId, Measure};
use crate::geom::{self, P2};
use crate::model::{Guid, MAX_COORD, MAX_POINTS, MAX_TEXT, Model};
use crate::view3d::P3;

/// The lowest and highest a decay's severity goes: 1, slight, to 5, the worst.
pub const SEVERITY: std::ops::RangeInclusive<u8> = 1..=5;

/// How long an area's description or intervention may be, characters.
pub const MAX_NOTE: usize = 4000;

/// How many photos one area may link.
pub const MAX_PHOTOS: usize = 64;

/// How many surveys a project may hold.
pub const MAX_SURVEYS: usize = 1000;

/// How much of an outline may lie off its face before it is in conflict, mm² (a hair's breadth along a long edge), or
/// this share of its area, whichever is more: what a round top laid out as a polygon misses of its circle.
pub const OFF_FACE: f64 = 1.0;
pub const OFF_SHARE: f64 = 1e-4;

/// What an area is: a survey area, the default, or an item drawn on the face to be counted, of a category of the
/// project's words (`stone`, `brick`, `painting`, `ornament`…). In files and commands `"area"` or `{"item": category}`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurveyKind {
    #[default]
    Area,
    Item(String),
}

impl SurveyKind {
    pub fn is_area(&self) -> bool {
        *self == SurveyKind::Area
    }

    /// As listings and schedules show it: `Survey Area`, or the item's category.
    pub fn label(&self) -> &str {
        match self {
            SurveyKind::Area => "Survey Area",
            SurveyKind::Item(c) => c,
        }
    }
}

/// How a survey was made.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// By eye, on site.
    #[default]
    Visual,
    /// By a thermal camera.
    Thermography,
    /// From a laser scan or photogrammetry.
    Scan,
    /// Another, in the project's own words.
    Custom(String),
}

impl Method {
    pub const NAMED: [Method; 3] = [Method::Visual, Method::Thermography, Method::Scan];

    pub fn label(&self) -> &str {
        match self {
            Method::Visual => "visual",
            Method::Thermography => "thermography",
            Method::Scan => "scan",
            Method::Custom(t) => t,
        }
    }

    /// One the list names, by its word in any case; else none.
    pub fn named(text: &str) -> Option<Method> {
        let t = text.trim().to_ascii_lowercase();
        Method::NAMED.into_iter().find(|m| m.label() == t)
    }
}

/// A survey campaign: when (`date`, `YYYY-MM-DD`, or empty for not said) and how it was made. Its name is its key in
/// `Model::surveys`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Survey {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub date: String,
    #[serde(default)]
    pub method: Method,
}

impl Survey {
    /// What's wrong with survey `name`, if anything.
    pub fn check(&self, name: &str) -> Result<(), String> {
        if name.trim().is_empty() || name.chars().count() > MAX_TEXT || name.chars().any(char::is_control) {
            return Err(format!("survey `{name}`: a name is one line of 1 to {MAX_TEXT} characters"));
        }
        if !self.date.is_empty() && !is_date(&self.date) {
            return Err(format!("survey `{name}`: its date `{}` is a day as YYYY-MM-DD", self.date));
        }
        if let Method::Custom(t) = &self.method
            && (t.trim().is_empty() || t.chars().count() > MAX_TEXT || t.chars().any(char::is_control))
        {
            return Err(format!("survey `{name}`: a method of its own is one line of 1 to {MAX_TEXT} characters"));
        }
        Ok(())
    }
}

/// Whether `t` is a day of the calendar as `YYYY-MM-DD`.
pub fn is_date(t: &str) -> bool {
    let b = t.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| t.get(r).filter(|s| s.bytes().all(|c| c.is_ascii_digit())).and_then(|s| s.parse::<u32>().ok());
    let (Some(y), Some(m), Some(d)) = (num(0..4), num(5..7), num(8..10)) else { return false };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}

/// A survey area or a drawn item: a closed outline, with holes, on face `face` of element `host`, in that face's `(u, v)`,
/// and what the survey says of it. Its place and its quantities are its host's, worked out when asked for (the module's
/// notes).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurveyArea {
    /// The element it is drawn on: a wall, a floor, a ceiling, a roof or a column ([`FaceId::HOSTS`]). It goes when its
    /// host does.
    pub host: Guid,
    pub face: FaceId,
    #[serde(default, skip_serializing_if = "SurveyKind::is_area")]
    pub kind: SurveyKind,
    /// Its outline, in the face's `(u, v)`, mm: a closed ring (the last point joins the first).
    pub outline: Vec<P2>,
    /// Rings inside it left out of it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<P2>>,
    /// How it has decayed (one of the 26 terms, or the project's own).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Decay>,
    /// How badly: 1 (slight) to 5 (the worst) ([`SEVERITY`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<u8>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// What is to be done to it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub intervention: String,
    /// How deep it goes into its host, mm, which gives it a volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<f64>,
    /// The survey that found it, by name ([`Survey`]; its GUID in a file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub survey: Option<String>,
    /// Photos of it: paths of files beside the project, relative to its folder, never held in it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub photos: Vec<String>,
    /// Its values of the project's parameters, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, serde_json::Value>,
}

/// An area as its host stands now ([`Model::survey_resolved`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    /// Its outline and holes in space, mm, points along a curve close enough to follow it; none where its face is gone.
    pub outline: Vec<P3>,
    pub holes: Vec<Vec<P3>>,
    /// Its area on the face (the true surface, developed on a curved wall), openings and all, mm².
    pub area: f64,
    /// What the openings cutting it take out of that, mm².
    pub cut: f64,
    /// Its area less what openings take out, mm².
    pub net: f64,
    /// The share of its area openings take out, 0 to 1.
    pub cut_share: f64,
    /// Its net area by its depth, mm³, when it has one.
    pub volume: Option<f64>,
    /// How much of it lies off its face, mm² in `(u, v)`.
    pub outside: f64,
    /// What's wrong with where it is, when anything is: off its face, wholly or in part, or a face its host no longer has.
    /// It keeps its data whatever this says.
    pub conflict: Option<String>,
}

/// The longest a chord along a curved face may be, mm, as an outline is followed in space.
const STEP: f64 = 100.0;

impl Model {
    /// Area `a` as its host stands now: where it is, what it measures and whether it is in conflict ([`Resolved`]).
    pub fn survey_resolved(&self, a: &SurveyArea) -> Resolved {
        self.survey_resolved_on(a, self.face_chart(a.host, a.face).as_ref())
    }

    /// [`Model::survey_resolved`] on `chart`, its host's face as [`Model::face_chart`] gave it: what a listing or a schedule
    /// of many areas on a few faces asks, working each face out once ([`crate::rooms::Rooms::chart`]).
    pub fn survey_resolved_on(&self, a: &SurveyArea, chart: Option<&Chart>) -> Resolved {
        let own = (geom::area(&a.outline).abs() - a.holes.iter().map(|h| geom::area(h).abs()).sum::<f64>()).max(0.0);
        let Some(chart) = chart else {
            return Resolved {
                outline: Vec::new(),
                holes: Vec::new(),
                area: 0.0,
                cut: 0.0,
                net: 0.0,
                cut_share: 0.0,
                volume: None,
                outside: own,
                conflict: Some(format!("its host {} has no {} face now", self.shown(a.host), a.face.name())),
            };
        };
        let m: Measure = chart.measure(&a.outline, &a.holes);
        let net = m.net();
        let conflict = if m.outside > OFF_FACE.max(own * OFF_SHARE) {
            Some(if m.area <= 0.0 {
                format!("it lies off its host's {} face", a.face.name())
            } else {
                format!("{:.0} mm² of it lies off its host's {} face", m.outside, a.face.name())
            })
        } else {
            None
        };
        Resolved {
            outline: chart.lift(&a.outline, STEP),
            holes: a.holes.iter().map(|h| chart.lift(h, STEP)).collect(),
            area: m.area,
            cut: m.cut,
            net,
            cut_share: if m.area > 0.0 { m.cut / m.area } else { 0.0 },
            volume: a.depth.map(|d| net * d),
            outside: m.outside,
            conflict,
        }
    }
}

/// What's wrong with a ring of an area's, if anything: 3 to [`MAX_POINTS`] points within [`MAX_COORD`], an area to it,
/// and no edge crossing another.
fn check_ring(who: &str, what: &str, ring: &[P2]) -> Result<(), String> {
    if ring.len() < 3 || ring.len() > MAX_POINTS {
        return Err(format!("{who}: its {what} is a closed ring of 3 to {MAX_POINTS} points [u, v]"));
    }
    if ring.iter().any(|q| !q.is_finite() || q.0.abs() > MAX_COORD || q.1.abs() > MAX_COORD) {
        return Err(format!("{who}: its {what}'s points are numbers within {MAX_COORD} mm"));
    }
    // Edges that cross, compared pairwise: an outline drawn by hand has few points.
    let edges: Vec<(P2, P2)> = geom::ring_edges(ring).collect();
    let n = edges.len();
    if n <= 512 {
        for i in 0..n {
            for j in i + 2..n {
                if i == 0 && j == n - 1 {
                    continue;
                }
                let ((a, b), (c, d)) = (edges[i], edges[j]);
                if crosses(a, b, c, d) {
                    return Err(format!("{who}: its {what} crosses itself (edges {i} and {j}); draw it as a ring that doesn't"));
                }
            }
        }
    }
    if geom::area(ring).abs() < 1.0 {
        return Err(format!("{who}: its {what} has no area (its points lie on a line or on one another)"));
    }
    Ok(())
}

/// Whether segments `a`–`b` and `c`–`d` cross or touch.
fn crosses(a: P2, b: P2, c: P2, d: P2) -> bool {
    let side = |p: P2, q: P2, r: P2| (q - p).cross(r - p);
    let (d1, d2, d3, d4) = (side(c, d, a), side(c, d, b), side(a, b, c), side(a, b, d));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0)) {
        return true;
    }
    let on = |p: P2, q: P2, r: P2| geom::dist_to_segment(r, p, q) < 1e-9;
    on(c, d, a) || on(c, d, b) || on(a, b, c) || on(a, b, d)
}

fn one_line(t: &str, max: usize) -> bool {
    t.chars().count() <= max && !t.chars().any(char::is_control)
}

/// What's wrong with survey area `a`, `who` as messages name it, if anything ([`Model::check`]). Its host's being there
/// is the references' check; that it has a face of this kind is this one's. Where its outline lies on the face is not:
/// off the face, it is in conflict, never refused.
pub fn check(m: &Model, who: &str, a: &SurveyArea) -> Result<(), String> {
    if let Some(host) = m.elements.get(a.host) {
        let category = host.category();
        if !FaceId::HOSTS.contains(&category) {
            return Err(format!("{who}: a {category} has no faces to survey; its host is a wall, a floor, a ceiling, a roof or a column"));
        }
        if !a.face.of(category) {
            return Err(format!("{who}: a {category} has no {} face", a.face.name()));
        }
    }
    check_ring(who, "outline", &a.outline)?;
    if a.holes.len() > MAX_POINTS {
        return Err(format!("{who}: up to {MAX_POINTS} holes"));
    }
    for (i, h) in a.holes.iter().enumerate() {
        check_ring(who, &format!("hole {i}"), h)?;
        if !h.iter().all(|q| {
            geom::contains(&a.outline, *q)
                || a.outline.iter().zip(a.outline.iter().cycle().skip(1)).any(|(p, r)| geom::dist_to_segment(*q, *p, *r) < 1e-6)
        }) {
            return Err(format!("{who}: its hole {i} isn't inside its outline"));
        }
    }
    if let SurveyKind::Item(c) = &a.kind
        && (c.trim().is_empty() || !one_line(c, MAX_TEXT))
    {
        return Err(format!("{who}: a drawn item's category is one line of 1 to {MAX_TEXT} characters (stone, brick, painting…)"));
    }
    if let Some(Decay::Custom(t)) = &a.condition
        && (t.trim().is_empty() || !one_line(t, MAX_TEXT))
    {
        return Err(format!("{who}: a condition of the project's own is one line of 1 to {MAX_TEXT} characters"));
    }
    if let Some(n) = a.severity
        && !SEVERITY.contains(&n)
    {
        return Err(format!("{who}: its severity is 1 (slight) to 5 (the worst), not {n}"));
    }
    for (what, t) in [("description", &a.description), ("intervention", &a.intervention)] {
        if t.chars().count() > MAX_NOTE || t.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
            return Err(format!("{who}: its {what} is text of up to {MAX_NOTE} characters"));
        }
    }
    if let Some(d) = a.depth
        && !(d.is_finite() && d > 0.0 && d <= MAX_COORD)
    {
        return Err(format!("{who}: its depth is a length over 0 mm, or none"));
    }
    if let Some(s) = &a.survey
        && !m.surveys.contains_key(s)
    {
        return Err(format!("{who}: it names survey `{s}`, which the project hasn't; add it with `survey`"));
    }
    if a.photos.len() > MAX_PHOTOS {
        return Err(format!("{who}: up to {MAX_PHOTOS} photos"));
    }
    for p in &a.photos {
        if !photo_path(p) {
            return Err(format!(
                "{who}: photo `{p}` is the path of a file beside the project, relative to its folder (`photos/north-01.jpg`), with no `..`"
            ));
        }
    }
    Ok(())
}

/// Whether `p` is a path relative to a project's folder that stays inside it: no root, no drive, no `..`.
pub fn photo_path(p: &str) -> bool {
    !p.trim().is_empty()
        && one_line(p, MAX_TEXT)
        && !p.starts_with(['/', '\\'])
        && !p.contains(':')
        && p.split(['/', '\\']).all(|part| part != ".." && !part.is_empty())
}

/// What an element's listing says of area `a` beside its own fields: what it measures where its host stands now (mm²,
/// mm³), the share openings take, and its conflict, if any.
pub fn quantities_json(m: &Model, a: &SurveyArea, chart: Option<&Chart>) -> Vec<(String, serde_json::Value)> {
    use serde_json::json;
    let r = m.survey_resolved_on(a, chart);
    let mut out = vec![
        ("area".to_string(), json!(r.area)),
        ("cut".to_string(), json!(r.cut)),
        ("net_area".to_string(), json!(r.net)),
        ("cut_share".to_string(), json!(r.cut_share)),
        ("outside".to_string(), json!(r.outside)),
        ("conflict".to_string(), json!(r.conflict)),
        ("level".to_string(), json!(m.elements.get(a.host).and_then(|h| m.level_of(h)))),
        // As schedules read them: m², m³ and words.
        ("area_m2".to_string(), json!(r.net / 1e6)),
        ("gross_m2".to_string(), json!(r.area / 1e6)),
        ("cut_m2".to_string(), json!(r.cut / 1e6)),
        ("kind_name".to_string(), json!(a.kind.label())),
        ("condition_name".to_string(), json!(a.condition.as_ref().map(Decay::label))),
        ("face_name".to_string(), json!(a.face.name())),
    ];
    if let Some(v) = r.volume {
        out.push(("volume".to_string(), json!(v)));
        out.push(("volume_m3".to_string(), json!(v / 1e9)));
    }
    out
}
