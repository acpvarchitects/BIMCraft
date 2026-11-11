//! What a view draws of the parts of building elements (`parts.rs`), as Revit's Parts Visibility: the original element,
//! the parts it was taken apart into, or both.
//!
//! A view's [`Look`] says it, as it says everything else a view hides and paints: a part is hidden unless the view shows
//! parts, and an element the view draws as its parts is hidden in its turn. So a plan, a section, an elevation and a 3D
//! view, which all ask the look whether to draw an element, agree with no code of their own; the 3D scene holds every
//! part's solid beside its host's, whatever any view shows of them.
//!
//! A part is drawn where its recipe holds, it isn't excluded and it wasn't divided into others ([`drawable`]), in a view
//! that draws its host. It is painted as its host is, then as the view filters that pick it say, then by the view's
//! [`Theme`]: the colour of what the part holds of one value (how it has decayed, when it was built), which makes a
//! thematic drawing of a view that shows parts, with a [`legend`] of the values, their colours, counts and areas.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{Applied, Graphics, Look};
use crate::model::{Element, Id, Key, MAX_TEXT, Model};
use crate::parts::{BuildingPart, Decay, Heritage};

/// Which of an element and its parts a view draws, as Revit's Parts Visibility.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartsVisibility {
    /// Show Original: every element whole, and no part. What a view does unless it says.
    #[default]
    Original,
    /// Show Parts: an element that has parts as its parts, and no more of it; one that has none, whole.
    Parts,
    /// Show Both: every element whole, and its parts too.
    Both,
}

impl PartsVisibility {
    pub const ALL: [PartsVisibility; 3] = [PartsVisibility::Original, PartsVisibility::Parts, PartsVisibility::Both];

    /// Its name in commands and files.
    pub fn name(self) -> &'static str {
        match self {
            PartsVisibility::Original => "original",
            PartsVisibility::Parts => "parts",
            PartsVisibility::Both => "both",
        }
    }

    /// As Revit's Properties name it.
    pub fn label(self) -> &'static str {
        match self {
            PartsVisibility::Original => bimcraft_i18n::ui_msg!("Show Original"),
            PartsVisibility::Parts => bimcraft_i18n::ui_msg!("Show Parts"),
            PartsVisibility::Both => bimcraft_i18n::ui_msg!("Show Both"),
        }
    }

    /// By its name or its label, whatever its case, spaces, hyphens or underscores (`parts`, `Show Parts`, `show_parts`).
    pub fn parse(text: &str) -> Option<PartsVisibility> {
        let key: String = text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect();
        let key = key.strip_prefix("show").unwrap_or(&key);
        PartsVisibility::ALL.into_iter().find(|v| v.name() == key)
    }

    /// Whether a view draws parts at all.
    pub fn shows_parts(self) -> bool {
        self != PartsVisibility::Original
    }
}

/// Whether the project has parts: from the lookup when it is made, else from among every element.
pub fn any(m: &Model) -> bool {
    match m.derived.made(&m.elements) {
        Some(look) => look.any_parts(),
        None => m.elements.values().any(|e| matches!(e, Element::Part(_))),
    }
}

/// Whether part `p`'s recipe still applies to its host, as far as its host's type and what it was divided from say: the
/// conflicts [`Model::part_conflict`] finds without making its solid (a host with no solid there is the one it leaves out,
/// and then there is nothing to draw of the host either). What a view asks each time it is drawn.
pub fn holds(m: &Model, p: &BuildingPart) -> bool {
    let mut up = p.parent;
    let mut depth = 0;
    while let Some(g) = up {
        let Some(Element::Part(parent)) = m.elements.get(g) else { return false };
        if m.part_layer(parent).is_err() {
            return false;
        }
        depth += 1;
        if depth > crate::parts::MAX_DEPTH {
            return false;
        }
        up = parent.parent;
    }
    match (m.part_layer(p), m.elements.get(p.host)) {
        (Ok(layer), Some(host)) => layer.is_none_or(|(_, l)| l.thickness > 0.0) && crate::parts::HOSTS.contains(&host.category()),
        _ => false,
    }
}

/// Whether part `id` was divided into other parts, which stand for it.
pub fn divided(m: &Model, id: Id) -> bool {
    let Some(guid) = m.elements.guid(id) else { return false };
    m.lookup().referrers(&guid).any(|other| matches!(m.elements.get(other), Some(Element::Part(child)) if child.parent == Some(guid)))
}

/// Whether part `id`, `p`, is one a view that shows parts draws: its recipe holds, it isn't excluded, and it wasn't
/// divided into others. The parts the 3D scene makes solids of.
pub fn drawable(m: &Model, id: Id, p: &BuildingPart) -> bool {
    !p.excluded && holds(m, p) && !divided(m, id)
}

impl Look {
    /// The parts of the project, `parts`, as a view with graphics `g` shows them, once the look says what it shows of every
    /// other element. A part is hidden unless the view shows parts, it is one to draw ([`drawable`]) and the view draws its
    /// host; shown, it is painted as its host is, then as each of the `applied` filters that pick it says, the first
    /// prevailing, then filled in its colour of the view's theme. Showing parts alone, an element with a part to draw is
    /// hidden, its parts standing for it, but one with a part in conflict, which stays whole so that nothing of it goes
    /// missing.
    pub(super) fn parts(&mut self, m: &Model, g: &Graphics, parts: &[(Id, &BuildingPart)], applied: &[(&Applied, BTreeSet<Id>)]) {
        let mode = g.parts_visibility.unwrap_or_default();
        let themed = g.theme.as_ref().filter(|_| mode.shows_parts()).map(|t| (t, t.colours(m, parts.iter().map(|(_, p)| *p))));
        let (mut stood_for, mut broken) = (BTreeSet::new(), BTreeSet::new());
        for &(id, p) in parts {
            let host = m.elements.id(p.host);
            let holds = holds(m, p);
            if !holds && !p.excluded {
                broken.extend(host);
            }
            let Some(host) = host.filter(|h| mode.shows_parts() && holds && !p.excluded && !self.hidden.contains(h) && !divided(m, id))
            else {
                self.hidden.insert(id);
                continue;
            };
            stood_for.insert(host);
            let mut over = self.paint.get(&host).cloned().unwrap_or_default();
            for (a, _) in applied.iter().rev().filter(|(_, picked)| picked.contains(&id)) {
                over = a.over.over(&over);
            }
            if over.hidden {
                self.hidden.insert(id);
                continue;
            }
            // The view's theme, over all the rest: the colour of what the part holds, where it is cut and where it is seen.
            if let Some((theme, colours)) = &themed {
                let colour = Some(theme.colour(colours, theme.value(m, p).as_deref()));
                (over.projection.fill, over.cut.fill) = (colour, colour);
            }
            if !over.is_empty() {
                self.paint.insert(id, over);
            }
        }
        if mode == PartsVisibility::Parts {
            for host in stood_for.difference(&broken) {
                self.hidden.insert(*host);
                self.paint.remove(host);
            }
        }
    }
}

/// What a theme may colour parts by, besides the heritage set and the project's parameters for parts: what each is made of.
pub const MATERIAL: &str = "Material";

/// The colour of a part that holds no value of what its view's theme goes by: a neutral grey, unless the view chooses.
pub const NOT_SET: [u8; 3] = [200, 200, 200];

/// The most values a theme chooses colours for.
pub const MAX_COLOURS: usize = 256;

/// How `Decay_Severity` is coloured, 1 (slight) to 5 (the worst): pale yellow through orange to dark red.
const RAMP: [[u8; 3]; 5] = [[255, 235, 150], [255, 190, 90], [250, 140, 50], [225, 70, 30], [165, 15, 25]];

/// The colours of the named decays, in the list's order ([`Decay::NAMED`]): 26 picked one by one, each the farthest in
/// CIELAB from those before it and from [`NOT_SET`]'s grey, among the colours neither too dark for ink to read on nor too
/// pale for paper. Any two are told apart at a glance, whichever decays a survey uses: a palette of hues in steps gives
/// neighbours in the list colours far apart but brings others, Biological colonization and Detachment, to one green.
const DECAYS: [[u8; 3]; 26] = [
    [230, 25, 75],
    [0, 0, 255],
    [0, 255, 0],
    [255, 204, 0],
    [0, 85, 187],
    [0, 102, 0],
    [255, 51, 221],
    [119, 68, 0],
    [0, 238, 187],
    [0, 85, 85],
    [136, 68, 102],
    [255, 119, 17],
    [170, 238, 102],
    [255, 170, 255],
    [0, 187, 255],
    [255, 153, 136],
    [153, 85, 255],
    [187, 170, 85],
    [136, 17, 136],
    [238, 68, 153],
    [0, 238, 255],
    [255, 0, 17],
    [102, 170, 136],
    [136, 136, 187],
    [0, 170, 0],
    [119, 102, 85],
];

/// How many colours the palette has: 24 hues, 15 degrees apart, in three bands of lightness.
const SLOTS: usize = 72;

/// Colour `slot` of the palette, of [`SLOTS`]: a hue seven steps of 15 degrees on from the last, so neighbours are far
/// apart (red, green, blue, magenta, yellow…), at full strength for the first 24, then lighter, then darker.
fn palette(slot: usize) -> [u8; 3] {
    let hue = (slot * 7 % 24) as f64 * 15.0;
    let (s, l) = [(0.75, 0.50), (0.60, 0.72), (0.70, 0.32)][slot / 24 % 3];
    // HSL to RGB.
    let c = (1.0 - (2.0 * l - 1.0f64).abs()) * s;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match (hue / 60.0) as usize {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r, g, b].map(|v| ((v + l - c / 2.0) * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// A number of a text that is the same on every machine and in every run (FNV-1a): where in the palette a value starts.
fn hashed(text: &str) -> usize {
    let h = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3));
    (h % SLOTS as u64) as usize
}

/// A view's theme: its parts filled in the colour of what each holds of one value, as a thematic drawing is (a decay
/// map, a drawing of the building's periods), where the view shows parts.
///
/// The colours are the same in every view and in every run. `Decay_Pathology`'s named decays have each their own, far
/// from every other's ([`DECAYS`], by their place in the list); `Decay_Severity` is a ramp from pale yellow, 1, to dark red,
/// 5; any other value (a period, a material, a decay of the project's own) takes the colour its own text comes to (a
/// hash of it), so it keeps it as the survey grows, and where two of the project's values come to one colour the later in
/// order takes the next the palette has free. `colors` gives a value another; a part with no value is [`NOT_SET`], or
/// what `colors` gives the value `""`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// What the parts are coloured by: one of the heritage set's names ([`Heritage::NAMES`]), [`MATERIAL`], or a project
    /// parameter of parts.
    pub by: String,
    /// The colours the view chooses, by value; `""`, of the parts that hold none.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, [u8; 3]>,
    /// Whether the view draws the theme's key in its corner: each value, its colour, how many parts and their net area.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub legend: bool,
}

impl Theme {
    /// What project `m`'s parts can be coloured by: the heritage set, their material, and the project's parameters for parts.
    pub fn names(m: &Model) -> Vec<String> {
        let own = Heritage::NAMES.into_iter().chain([MATERIAL]).map(String::from);
        own.chain(m.parameters.iter().filter(|(_, p)| p.categories.iter().any(|c| c == "part")).map(|(n, _)| n.clone())).collect()
    }

    /// Whether it is sound, whatever project it is in: a name, and no more colours than a theme holds. What it names is
    /// the command's to check: a theme by a value no part has colours every part as not set.
    pub fn check(&self) -> Result<(), String> {
        if self.by.trim().is_empty() || self.by.chars().count() > MAX_TEXT {
            return Err(format!(
                "its theme colours parts by a value's name, 1 to {MAX_TEXT} characters (Decay_Pathology, Historical_Period…)"
            ));
        }
        if self.colors.len() > MAX_COLOURS || self.colors.keys().any(|k| k.chars().count() > MAX_TEXT) {
            return Err(format!("its theme chooses up to {MAX_COLOURS} colours, each for a value of up to {MAX_TEXT} characters"));
        }
        Ok(())
    }

    /// What part `p` holds of what the theme goes by, as it is shown; none where it holds none. As
    /// `schedule::field_text` reads the same names, without working out what the part measures.
    pub fn value(&self, m: &Model, p: &BuildingPart) -> Option<String> {
        let h = &p.heritage;
        let [su, period, decay, severity, intervention] = Heritage::NAMES;
        let text = |s: &str| Some(s.trim()).filter(|s| !s.is_empty()).map(String::from);
        match self.by.as_str() {
            n if n == su => text(&h.su_code),
            n if n == period => text(&h.historical_period),
            n if n == decay => h.decay_pathology.as_ref().and_then(|d| text(d.label())),
            n if n == severity => h.decay_severity.map(|s| s.to_string()),
            n if n == intervention => text(&h.intervention_type),
            MATERIAL => m.part_material(p).and_then(text),
            name => match p.params.get(name)? {
                serde_json::Value::String(s) => text(s),
                serde_json::Value::Null => None,
                serde_json::Value::Bool(b) => Some(if *b { "Yes" } else { "No" }.to_string()),
                other => Some(other.to_string()),
            },
        }
    }

    /// The colour of each value `parts` (the project's) hold, as the type's notes say: its own where the view chose one.
    pub fn colours<'a>(&self, m: &Model, parts: impl Iterator<Item = &'a BuildingPart>) -> BTreeMap<String, [u8; 3]> {
        let values: BTreeSet<String> = parts.filter_map(|p| self.value(m, p)).collect();
        let mut out = BTreeMap::new();
        let mut taken = [false; SLOTS];
        if self.by == Heritage::NAMES[2] {
            for (d, c) in Decay::NAMED.iter().zip(DECAYS) {
                out.insert(d.label().to_string(), c);
            }
        }
        if self.by == Heritage::NAMES[3] {
            for (n, c) in RAMP.iter().enumerate() {
                out.insert((n + 1).to_string(), *c);
            }
        }
        for v in values {
            if out.contains_key(&v) {
                continue;
            }
            // Its own text's colour, or the next one free; with none free, its own text's after all.
            let first = hashed(&v);
            let slot = (0..SLOTS).map(|k| (first + k) % SLOTS).find(|k| taken.get(*k) == Some(&false)).unwrap_or(first);
            if let Some(t) = taken.get_mut(slot) {
                *t = true;
            }
            out.insert(v, palette(slot));
        }
        for (v, c) in &self.colors {
            out.insert(v.clone(), *c);
        }
        out
    }

    /// The colour of a part that holds `value`, among `colours` ([`Theme::colours`]).
    pub fn colour(&self, colours: &BTreeMap<String, [u8; 3]>, value: Option<&str>) -> [u8; 3] {
        match value {
            Some(v) => colours.get(v).copied().unwrap_or(NOT_SET),
            None => self.colors.get("").copied().unwrap_or(NOT_SET),
        }
    }

    /// Where a value comes among the others in a legend: a named decay by its place in the list, a number by what it
    /// is, then the rest in order; the parts that hold none, last.
    fn place(&self, value: &Option<String>) -> (usize, f64, String) {
        let Some(v) = value else { return (3, 0.0, String::new()) };
        if self.by == Heritage::NAMES[2]
            && let Some(k) = Decay::NAMED.iter().position(|d| d.label() == v)
        {
            return (0, k as f64, String::new());
        }
        match v.parse::<f64>() {
            Ok(n) if n.is_finite() => (1, n, String::new()),
            _ => (2, 0.0, v.clone()),
        }
    }
}

/// A row of a theme's legend: a value (none: the parts that hold none), its colour, how many of the view's parts hold
/// it and the net area of their main faces together, mm² ([`Model::part_quantities`]).
#[derive(Clone, Debug, PartialEq)]
pub struct LegendRow {
    pub value: Option<String>,
    pub color: [u8; 3],
    pub count: usize,
    pub area: f64,
}

/// The legend of a view's theme: what it colours by, whether the view draws it, and a row for each value among the
/// parts the view draws.
#[derive(Clone, Debug, PartialEq)]
pub struct Legend {
    pub by: String,
    pub drawn: bool,
    pub rows: Vec<LegendRow>,
}

/// The parts view `view` draws, as `look` (its own) shows them: a plan's, those its cut plane cuts and those it sees from
/// it, in its crop; a section's or an elevation's, those that reach from its line to its far clip, between its ends and
/// its crop's heights where it is cropped, hidden behind others or not; a 3D view's, those in its section box.
pub fn drawn_in(m: &Model, view: Id, look: &Look) -> Vec<Id> {
    let shown = m.elements.iter().filter_map(|(id, e)| match e {
        Element::Part(p) if look.shows(*id) => Some((*id, p)),
        _ => None,
    });
    // The box round a part's solid: its lowest and highest corners.
    let reach = |p: &BuildingPart| {
        let tris = m.part_solid(p).ok()?;
        let lo = tris.iter().flatten().fold([f64::MAX; 3], |a, q| std::array::from_fn(|k| a[k].min(q[k])));
        let hi = tris.iter().flatten().fold([f64::MIN; 3], |a, q| std::array::from_fn(|k| a[k].max(q[k])));
        Some((lo, hi))
    };
    match m.elements.get(view) {
        Some(Element::Level(_) | Element::PlanView(_)) => {
            let seen = crate::render::parts_in_plan(m, view, look);
            shown.filter(|(id, _)| seen.contains(id)).map(|(id, _)| id).collect()
        }
        Some(Element::Section(v) | Element::Elevation(v)) => {
            let (u, len) = ((v.end - v.start).unit(), v.start.dist(v.end));
            shown
                .filter(|(_, p)| {
                    let Some((lo, hi)) = reach(p) else { return false };
                    let corners =
                        [(lo[0], lo[1]), (hi[0], lo[1]), (hi[0], hi[1]), (lo[0], hi[1])].map(|(x, y)| crate::geom::P2(x, y) - v.start);
                    let span = |f: &dyn Fn(crate::geom::P2) -> f64| {
                        corners.iter().fold((f64::MAX, f64::MIN), |(a, z), q| (a.min(f(*q)), z.max(f(*q))))
                    };
                    let (past, along) = (span(&|q| q.dot(v.looking())), span(&|q| q.dot(u)));
                    past.1 >= -crate::model::TOL
                        && v.depth.is_none_or(|far| past.0 <= far)
                        && v.crop.is_none_or(|[bottom, top]| along.1 >= 0.0 && along.0 <= len && hi[2] >= bottom && lo[2] <= top)
                })
                .map(|(id, _)| id)
                .collect()
        }
        _ => shown
            .filter(|(_, p)| match (look.section_box(), reach(p)) {
                (Some((min, max)), Some((lo, hi))) => (0..3).all(|k| hi[k] >= min[k] && lo[k] <= max[k]),
                (None, _) => true,
                (Some(_), None) => false,
            })
            .map(|(id, _)| id)
            .collect(),
    }
}

/// The legend of view `view`'s theme: none for a view with no theme (or what is no view). A view that shows only the
/// originals draws no part, so its legend has no rows.
pub fn legend(m: &Model, view: impl Key) -> Option<Legend> {
    let view = m.elements.id(view)?;
    let g = super::graphics_of(m, view)?.resolved(m);
    let theme = g.theme.as_ref()?;
    let look = Look::of(m, view);
    let ids = drawn_in(m, view, &look);
    let parts = |ids: &[Id]| {
        ids.iter()
            .filter_map(|id| match m.elements.get(*id) {
                Some(Element::Part(p)) => Some(p),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let every: Vec<&BuildingPart> = m
        .elements
        .values()
        .filter_map(|e| match e {
            Element::Part(p) => Some(p),
            _ => None,
        })
        .collect();
    let colours = theme.colours(m, every.into_iter());
    let mut rows: Vec<LegendRow> = Vec::new();
    for p in parts(&ids) {
        let value = theme.value(m, p);
        let area = m.part_quantities(p).ok().and_then(|(area, _)| area).unwrap_or(0.0);
        match rows.iter_mut().find(|r| r.value == value) {
            Some(row) => (row.count, row.area) = (row.count + 1, row.area + area),
            None => {
                let color = theme.colour(&colours, value.as_deref());
                rows.push(LegendRow { value, color, count: 1, area });
            }
        }
    }
    rows.sort_by(|a, b| {
        let (x, y) = (theme.place(&a.value), theme.place(&b.value));
        x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)).then(x.2.cmp(&y.2))
    });
    Some(Legend { by: theme.by.clone(), drawn: theme.legend, rows })
}
