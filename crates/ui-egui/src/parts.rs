//! Parts on the desktop: Create Parts on the Modify tab while walls, floors, roofs, ceilings or components are selected,
//! Exclude Parts and Restore Parts while parts are, and a part's Properties: what it is a part of and measures, what it is
//! made of, its kind and the heritage set. Each runs `create_parts` or `part`; how a view colours its parts is in its own
//! Properties (`graphics.rs`). Divide Parts, a sketch on a part's face, is to come ([parts](../../../docs/areas/parts.md)).

use bimcraft_i18n::msg;

use bimcraft_core::decay::Decay;
use bimcraft_core::model::{Element, Id, Model};
use bimcraft_core::parts::{BuildingPart, HOSTS, Heritage, PartType};
use egui::{RichText, Ui, WidgetInfo, WidgetType};
use serde_json::{Map, Value, json};

use crate::icons::Icon;
use crate::lang::{category, t};
use crate::properties::{combo, grid, group_header};
use crate::ribbon::panel;
use crate::style::pal;
use crate::{BimApp, Status, Tool, big_button};

/// A part's kinds, as IFC has them, and their labels.
const KINDS: [(PartType, &str); 4] = [
    (PartType::Insulation, msg!("Insulation")),
    (PartType::PrecastPanel, msg!("Precast Panel")),
    (PartType::UserDefined, msg!("User Defined")),
    (PartType::NotDefined, msg!("Not Defined")),
];

/// What a text row of the heritage set reads of it.
type Said = fn(&Heritage) -> String;

/// What a layer does, as a part's Layer row names it.
fn function_label(function: &str) -> &'static str {
    match function {
        "structure" => msg!("Structure"),
        "substrate" => msg!("Substrate"),
        "thermal" => msg!("Thermal"),
        "membrane" => msg!("Membrane"),
        _ => msg!("Finish"),
    }
}

/// What every one of `parts` says of `f`, or none where they differ.
fn shared<T: PartialEq>(parts: &[&BuildingPart], f: impl Fn(&BuildingPart) -> T) -> Option<T> {
    let mut values = parts.iter().map(|p| f(p));
    let first = values.next()?;
    values.all(|v| v == first).then_some(first)
}

/// What every one of `all` holds at `key`, or null where they differ.
fn shared_json(all: &[std::sync::Arc<Value>], key: &str) -> Value {
    let first = all.first().map_or(Value::Null, |v| v[key].clone());
    if all.iter().all(|v| v[key] == first) { first } else { Value::Null }
}

impl BimApp {
    /// The selection's elements that can be taken apart into parts.
    fn selected_hosts(&self) -> Vec<Id> {
        let m = &self.session.model;
        self.selection.iter().copied().filter(|id| m.elements.get(*id).is_some_and(|e| HOSTS.contains(&e.category()))).collect()
    }

    /// The selection's parts, and whether some are excluded and some not.
    fn selected_parts(&self) -> (Vec<Id>, bool, bool) {
        let m = &self.session.model;
        let parts: Vec<(Id, bool)> = self
            .selection
            .iter()
            .filter_map(|id| match m.elements.get(*id) {
                Some(Element::Part(p)) => Some((*id, p.excluded)),
                _ => None,
            })
            .collect();
        let (some_out, some_in) = (parts.iter().any(|(_, out)| *out), parts.iter().any(|(_, out)| !*out));
        (parts.into_iter().map(|(id, _)| id).collect(), some_in, some_out)
    }

    /// The Modify tab's panels for parts, as Revit's: Create Parts while what can be taken apart is selected, Exclude Parts
    /// and Restore Parts while parts are.
    pub(crate) fn parts_panels(&mut self, ui: &mut Ui) {
        if self.tool != Tool::Modify {
            return;
        }
        let hosts = self.selected_hosts();
        if !hosts.is_empty() {
            panel(ui, msg!("Create"), |ui| {
                let tip = msg!(
                    "Create Parts: take the selected walls, floors, roofs, ceilings and components apart, a part for each layer of their types"
                );
                if big_button(ui, Icon::CreateParts, msg!("Create Parts"), false, tip).clicked() {
                    self.create_parts(&hosts);
                }
            });
        }
        let (parts, some_in, some_out) = self.selected_parts();
        if !parts.is_empty() {
            let mut excluded = None;
            panel(ui, msg!("Part"), |ui| {
                let tip = msg!("Exclude Parts: leave the selected parts out of what is counted, scheduled and exported");
                if ui.add_enabled_ui(some_in, |ui| big_button(ui, Icon::ExcludeParts, msg!("Exclude Parts"), false, tip)).inner.clicked() {
                    excluded = Some(true);
                }
                let tip = msg!("Restore Parts: count, schedule and export the selected parts again");
                if ui.add_enabled_ui(some_out, |ui| big_button(ui, Icon::RestoreParts, msg!("Restore Parts"), false, tip)).inner.clicked() {
                    excluded = Some(false);
                }
            });
            if let Some(excluded) = excluded {
                self.run("part", json!({"ids": parts, "excluded": excluded}));
            }
        }
    }

    /// Create Parts on `hosts`, in one undo step: the parts made selected, and the view showing parts unless it does or its
    /// template says what it shows, as Revit's.
    fn create_parts(&mut self, hosts: &[Id]) {
        let mut commands = vec![("create_parts", json!({"ids": hosts}))];
        if let Some(view) = self.graphic_view() {
            let own = self.graphics_now();
            let shows = self.graphics_shown().parts_visibility.unwrap_or_default().shows_parts();
            if own.parts_controlled_by(&self.session.model).is_none() && !shows {
                commands.push(("graphics", json!({"view": view, "parts_visibility": "parts"})));
            }
        }
        match self.exec_batch(msg!("Create Parts"), &commands) {
            Ok(done) => {
                let made = done.first().and_then(|v| v["ids"].as_array());
                self.selection = made.map(|ids| ids.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
                self.status = Status::default();
            }
            Err(e) => self.status = Status { text: e, error: true },
        }
    }

    /// The selected parts' Properties, while the palette shows parts alone: what they are parts of and measure, read only;
    /// what they are made of, their kind, whether they are excluded, and the heritage set, each set on all of them by `part`.
    pub(crate) fn part_parameters(&mut self, ui: &mut Ui, model: &Model, ids: &[Id]) {
        let parts: Vec<&BuildingPart> = ids
            .iter()
            .filter_map(|id| match model.elements.get(*id) {
                Some(Element::Part(p)) => Some(p),
                _ => None,
            })
            .collect();
        let (Some(&id), true) = (ids.first(), !parts.is_empty() && parts.len() == ids.len()) else { return };
        let all = self.elements_json(model, ids);
        let units = model.units;
        let mut change: Option<(&str, Value)> = None;
        let muted = |ui: &mut Ui, text: String| {
            ui.label(RichText::new(text).color(pal().muted));
        };
        let none = t("<None>").to_string();
        // What they are of, what they are made of and what kind they are.
        if self.part_group(ui, msg!("Identity Data")) {
            grid(ui, ("part-identity", id), |ui| {
                ui.label(t("Original Category"));
                muted(ui, shared_json(&all, "host_category").as_str().map(|c| category(c, false)).unwrap_or_default());
                ui.end_row();
                ui.label(t("Original Type"));
                muted(ui, shared_json(&all, "host_type").as_str().unwrap_or_default().to_string());
                ui.end_row();
                // Its layer of its host's type, and what that layer does; a part of all of its host has none.
                if let Some(n) = shared_json(&all, "layer").as_u64() {
                    ui.label(t("Layer"));
                    let function = shared_json(&all, "function");
                    muted(ui, format!("{n} ({})", t(function_label(function.as_str().unwrap_or_default()))));
                    ui.end_row();
                }
                // Its own material, or its layer's (By Original), as Revit's Material By Original.
                let by_original = t("By Original").to_string();
                let label = ui.label(t("Material"));
                let resolved = shared_json(&all, "material");
                let now = match (shared(&parts, |p| p.material.clone()), resolved.as_str()) {
                    (Some(Some(own)), _) => own,
                    (Some(None), Some(m)) => format!("{m} ({by_original})"),
                    (Some(None), None) => by_original.clone(),
                    (None, _) => String::new(),
                };
                let options: Vec<String> = std::iter::once(by_original.clone()).chain(model.materials.keys().cloned()).collect();
                if let Some(picked) = combo(ui, (id, "part-material"), label.id, &now, &options) {
                    change = Some(("material", if picked == by_original { Value::Null } else { json!(picked) }));
                }
                ui.end_row();
                let label = ui.label(t("Kind"));
                let now = shared(&parts, |p| p.predefined_type)
                    .and_then(|k| KINDS.iter().find(|(kind, _)| *kind == k))
                    .map_or(String::new(), |(_, name)| t(name).to_string());
                let options: Vec<String> = KINDS.iter().map(|(_, name)| t(name).to_string()).collect();
                if let Some(picked) = combo(ui, (id, "part-kind"), label.id, &now, &options)
                    && let Some((kind, _)) = KINDS.iter().find(|(_, name)| t(name) == picked)
                {
                    change = Some(("predefined_type", json!(kind.name())));
                }
                ui.end_row();
                let label = ui.label(t("Object Type"));
                let now = shared(&parts, |p| p.object_type.clone()).unwrap_or_default();
                if let Some(typed) = self.field(ui, id, "object_type", now, label.id, "") {
                    change = Some(("object_type", text_or_null(&typed)));
                }
                ui.end_row();
                let label = ui.label(t("Excluded"));
                let mut on = shared(&parts, |p| p.excluded).unwrap_or(false);
                let r = ui.checkbox(&mut on, "");
                r.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, on, t("Excluded")));
                if r.labelled_by(label.id).changed() {
                    change = Some(("excluded", json!(on)));
                }
                ui.end_row();
            });
        }
        // What they measure, worked out from their hosts as they stand: a divided part is counted in its pieces.
        if self.part_group(ui, msg!("Dimensions")) {
            grid(ui, ("part-dimensions", id), |ui| {
                if let Some(mm) = shared_json(&all, "thickness").as_f64() {
                    ui.label(t("Thickness"));
                    muted(ui, units.length(mm));
                    ui.end_row();
                }
                if let Some(m2) = shared_json(&all, "area_m2").as_f64() {
                    ui.label(t("Area"));
                    let (area, unit) = units.area(m2 * 1e6);
                    muted(ui, format!("{area:.2} {unit}"));
                    ui.end_row();
                }
                if let Some(m3) = shared_json(&all, "volume_m3").as_f64() {
                    ui.label(t("Volume"));
                    let (volume, unit) = units.volume(m3 * 1e9);
                    muted(ui, format!("{volume:.3} {unit}"));
                    ui.end_row();
                }
            });
        }
        // The heritage set, by the names schedules, themes and IFC give it.
        let [su, period, decay, severity, intervention] = Heritage::NAMES;
        if self.part_group(ui, msg!("Heritage")) {
            grid(ui, ("part-heritage", id), |ui| {
                let texts: [(&str, &str, Said); 2] =
                    [(su, "su_code", |h| h.su_code.clone()), (period, "historical_period", |h| h.historical_period.clone())];
                for (name, key, of) in texts {
                    let label = ui.label(name);
                    let now = shared(&parts, |p| of(&p.heritage)).unwrap_or_default();
                    if let Some(typed) = self.field(ui, id, key, now, label.id, "") {
                        change = Some((key, text_or_null(&typed)));
                    }
                    ui.end_row();
                }
                // One of the list's words; a decay in the project's own words is typed in the row under it.
                let label = ui.label(decay);
                let said = shared(&parts, |p| p.heritage.decay_pathology.clone());
                let now = match &said {
                    Some(Some(d @ Decay::Custom(_))) => d.label().to_string(),
                    Some(Some(d)) => t(d.label()).to_string(),
                    Some(None) => none.clone(),
                    None => String::new(),
                };
                let options: Vec<String> =
                    std::iter::once(none.clone()).chain(Decay::NAMED.iter().map(|d| t(d.label()).to_string())).collect();
                if let Some(picked) = combo(ui, (id, "part-decay"), label.id, &now, &options) {
                    let word = Decay::NAMED.iter().find(|d| t(d.label()) == picked).map(|d| json!(d));
                    change = Some(("decay_pathology", word.unwrap_or(Value::Null)));
                }
                ui.end_row();
                let label = ui.label(t("Other Decay"));
                let own = match &said {
                    Some(Some(Decay::Custom(text))) => text.clone(),
                    _ => String::new(),
                };
                if let Some(typed) = self.field(ui, id, "decay_custom", own, label.id, "") {
                    let typed = typed.trim();
                    change = Some(("decay_pathology", if typed.is_empty() { Value::Null } else { json!({"custom": typed}) }));
                }
                ui.end_row();
                // From 1, slight, to 5, the worst.
                let label = ui.label(severity);
                let now = match shared(&parts, |p| p.heritage.decay_severity) {
                    Some(Some(n)) => n.to_string(),
                    Some(None) => none.clone(),
                    None => String::new(),
                };
                let options: Vec<String> = std::iter::once(none.clone()).chain((1..=5).map(|n: u8| n.to_string())).collect();
                if let Some(picked) = combo(ui, (id, "part-severity"), label.id, &now, &options) {
                    change = Some(("decay_severity", picked.parse::<u8>().map_or(Value::Null, |n| json!(n))));
                }
                ui.end_row();
                let label = ui.label(intervention);
                let now = shared(&parts, |p| p.heritage.intervention_type.clone()).unwrap_or_default();
                if let Some(typed) = self.field(ui, id, "intervention_type", now, label.id, "") {
                    change = Some(("intervention_type", text_or_null(&typed)));
                }
                ui.end_row();
            });
        }
        if let Some((key, value)) = change {
            let mut p = Map::new();
            p.insert("ids".into(), json!(ids));
            p.insert(key.into(), value);
            self.run("part", Value::Object(p));
        }
    }

    /// A group's header in the palette: whether it is open.
    fn part_group(&mut self, ui: &mut Ui, group: &'static str) -> bool {
        let open = !self.collapsed.contains(group);
        if group_header(ui, t(group), open) && !self.collapsed.remove(group) {
            self.collapsed.insert(group);
        }
        open
    }
}

/// What was typed, or null for nothing: a value cleared.
fn text_or_null(typed: &str) -> Value {
    let typed = typed.trim();
    if typed.is_empty() { Value::Null } else { json!(typed) }
}
