//! Associated arrays use the native copy/transform rebuild; the draft keeps real forms and nested instances.
use super::select_box::{How, Picked};
use super::*;
use bimcraft_core::family::arrays::{ArrayLayout, ArrayPattern, FamilyArray, MoveTo};
use bimcraft_core::family::constraints::Transform;
use bimcraft_core::ids::Guid;
#[derive(Clone)]
pub(crate) struct Options {
    pub active: bool,
    pub associate: bool,
    pub number: String,
    pub move_to: MoveTo,
    pub radial: bool,
    pub constrain: bool,
    pub label: String,
    pub seed: Option<Guid>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            active: false,
            associate: true,
            number: "2".into(),
            move_to: MoveTo::Second,
            radial: false,
            constrain: false,
            label: String::new(),
            seed: None,
        }
    }
}
impl FamilyEditor {
    pub(crate) fn array_allowed(&self) -> bool {
        self.adaptive.is_none()
            && !self.sketching()
            && self.views.shown.is_some()
            && !self.selection.items().is_empty()
            && self.selection.items().iter().all(|p| matches!(p, Picked::Form(_) | Picked::Nested(_)))
    }
    pub(crate) fn array_tool(&mut self) -> Result<(), String> {
        if !self.array_allowed() {
            return Err(t("Array needs forms or nested instances selected in a plan or elevation.").into());
        }
        self.modify_tool_on(super::modify::Tool::Move { copy: true, from: None })?;
        self.array_options.active = true;
        self.views.modify_options.multiple = false;
        self.views.modify_options.constrain = self.array_options.constrain;
        self.views.modify_options.rotate_copy = true;
        Ok(())
    }
    pub(crate) fn sync_arrays(&mut self) -> Result<(), String> {
        if self.arrays.is_empty() {
            return Ok(());
        }
        let family = self.family()?;
        let values = self.values();
        let rebuilt = family.rebuild_arrays(&values)?;
        if rebuilt.forms == family.forms && rebuilt.nested_instances == family.nested_instances && rebuilt.dimensions == family.dimensions {
            return Ok(());
        }
        self.install_array_family(rebuilt);
        Ok(())
    }
    fn install_array_family(&mut self, family: Family) {
        let selected = self
            .selection
            .items()
            .iter()
            .filter_map(|p| match *p {
                Picked::Form(i) => self.forms.get(i).map(|f| (false, f.original.guid)),
                Picked::Nested(i) => self.nested_instances.get(i).map(|n| (true, n.guid)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let old = self.forms.iter().map(|f| (f.original.guid, f.tag)).collect::<BTreeMap<_, _>>();
        self.forms = family
            .forms
            .iter()
            .map(|f| match old.get(&f.guid) {
                Some(tag) => FormText::again(f, *tag),
                None => FormText::of(f),
            })
            .collect();
        self.nested_instances = family.nested_instances;
        self.arrays = family.arrays;
        self.dimensions = family.dimensions;
        let selected = selected
            .into_iter()
            .filter_map(|(nested, g)| {
                if nested {
                    self.nested_instances.iter().position(|n| n.guid == g).map(Picked::Nested)
                } else {
                    self.forms.iter().position(|f| f.original.guid == g).map(Picked::Form)
                }
            })
            .collect::<Vec<_>>();
        self.selection.apply(&selected, How::Replace);
        self.prune_selection();
    }
    pub(crate) fn apply_array_layout(&mut self, items: &[Picked], layout: ArrayLayout) -> Result<(), String> {
        let mut family = self.family()?;
        let values = self.values();
        let guid = Guid::new();
        let mut members = Vec::new();
        for item in items {
            match *item {
                Picked::Form(i) => {
                    let f = family.forms.get_mut(i).ok_or("Array form no longer exists")?;
                    if f.array.is_some() {
                        return Err(t("Edit the existing array, or ungroup it before creating another array.").into());
                    }
                    f.array = Some((guid, 0));
                    members.push(f.guid);
                }
                Picked::Nested(i) => {
                    let n = family.nested_instances.get_mut(i).ok_or("Array nested instance no longer exists")?;
                    if n.array.is_some() {
                        return Err(t("Edit the existing array, or ungroup it before creating another array.").into());
                    }
                    n.array = Some((guid, 0));
                    members.push(n.guid);
                }
                _ => return Err(t("Array takes forms and nested instances.").into()),
            }
        }
        if members.is_empty() {
            return Err(t("Select an array seed first.").into());
        }
        let number = if self.array_options.associate && !self.array_options.label.is_empty() {
            Expr::Formula(self.array_options.label.clone())
        } else {
            self.array_options.number.parse::<f64>().map(Expr::Number).map_err(|_| t("Array Number must be a whole number.").to_string())?
        };
        family.arrays.push(FamilyArray {
            guid,
            members,
            pattern: ArrayPattern { number, move_to: self.array_options.move_to, layout },
            constrain: self.array_options.constrain,
            ..Default::default()
        });
        let mut rebuilt = if self.array_options.associate {
            family.rebuild_arrays(&values)?
        } else {
            let array = family.arrays.pop().ok_or("Array definition was lost")?;
            let existing = std::mem::replace(&mut family.arrays, vec![array]);
            let mut rebuilt = family.plain_array_copies(&values)?;
            rebuilt.arrays.extend(existing);
            rebuilt
        };
        if !self.array_options.associate {
            rebuilt.arrays.retain(|a| a.guid != guid);
            for f in &mut rebuilt.forms {
                if f.array.is_some_and(|(g, _)| g == guid) {
                    f.array = None;
                }
            }
            for n in &mut rebuilt.nested_instances {
                if n.array.is_some_and(|(g, _)| g == guid) {
                    n.array = None;
                }
            }
        }
        rebuilt.check(&|_| true)?;
        self.install_array_family(rebuilt);
        self.array_options.active = false;
        Ok(())
    }
    pub(crate) fn array_transform(&mut self, items: &[Picked], transform: Transform) -> Result<(), String> {
        match transform {
            Transform::Rotate { centre, axis, degrees } => self.apply_array_layout(items, ArrayLayout::Radial { centre, axis, degrees }),
            _ => Err("An array rotation needs a centre and axis".into()),
        }
    }
    fn selected_array(&self) -> Option<usize> {
        let group = self.selection.items().iter().find_map(|p| match *p {
            Picked::Form(i) => self.forms.get(i)?.original.array.map(|a| a.0),
            Picked::Nested(i) => self.nested_instances.get(i)?.array.map(|a| a.0),
            _ => None,
        })?;
        self.arrays.iter().position(|a| a.guid == group)
    }
    pub(crate) fn array_selection(&mut self) {
        let Some(index) = self.selected_array() else { return };
        let g = self.arrays[index].guid;
        if self.array_options.seed == Some(g) {
            return;
        }
        let mut selected = self.selection.items().to_vec();
        selected.extend(
            self.forms.iter().enumerate().filter(|(_, f)| f.original.array.is_some_and(|(a, _)| a == g)).map(|(i, _)| Picked::Form(i)),
        );
        selected.extend(
            self.nested_instances.iter().enumerate().filter(|(_, n)| n.array.is_some_and(|(a, _)| a == g)).map(|(i, _)| Picked::Nested(i)),
        );
        selected.sort();
        selected.dedup();
        if selected != self.selection.items() {
            self.selection.apply(&selected, How::Replace);
        }
    }
    pub(crate) fn array_properties(&mut self, ui: &mut Ui) {
        let Some(index) = self.selected_array() else { return };
        ui.label(egui::RichText::new(t("Array")).strong());
        let mut number = expr_text(&self.arrays[index].pattern.number);
        ui.horizontal(|ui| {
            let label = ui.label(t("Number:"));
            if ui.text_edit_singleline(&mut number).labelled_by(label.id).changed() {
                self.arrays[index].pattern.number = number.parse().map(Expr::Number).unwrap_or(Expr::Formula(number));
            }
        });
        if ui.button(if self.array_options.seed.is_some() { t("Finish Array Seed") } else { t("Edit Array Seed") }).clicked() {
            let g = self.arrays[index].guid;
            self.array_options.seed = if self.array_options.seed == Some(g) { None } else { Some(g) };
            let items = self
                .forms
                .iter()
                .enumerate()
                .filter(|(_, f)| f.original.array == Some((g, 0)))
                .map(|(i, _)| Picked::Form(i))
                .chain(self.nested_instances.iter().enumerate().filter(|(_, n)| n.array == Some((g, 0))).map(|(i, _)| Picked::Nested(i)))
                .collect::<Vec<_>>();
            self.selection.apply(&items, How::Replace);
        }
        if ui.button(t("Ungroup Array")).clicked() {
            let g = self.arrays[index].guid;
            self.arrays.remove(index);
            for f in &mut self.forms {
                if f.original.array.is_some_and(|(a, _)| a == g) {
                    f.original.array = None;
                }
            }
            for n in &mut self.nested_instances {
                if n.array.is_some_and(|(a, _)| a == g) {
                    n.array = None;
                }
            }
        }
        ui.separator();
    }
}
pub(crate) fn options_bar(ui: &mut Ui, ed: &mut FamilyEditor) {
    let integer_names = ed.params.iter().filter(|p| p.kind == Holds::Integer).map(|p| p.name.clone()).collect::<Vec<_>>();
    let values = ed.values();
    let o = &mut ed.array_options;
    ui.checkbox(&mut o.associate, t("Group and Associate"));
    let label = ui.label(t("Number:"));
    if o.associate && !o.label.is_empty() {
        let mut computed = match values.get(&o.label) {
            Some(bimcraft_core::family::Val::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        ui.add_enabled(false, egui::TextEdit::singleline(&mut computed).desired_width(40.)).labelled_by(label.id);
    } else {
        ui.add(egui::TextEdit::singleline(&mut o.number).desired_width(40.)).labelled_by(label.id);
    }
    egui::ComboBox::from_id_salt("array-move-to")
        .selected_text(if o.move_to == MoveTo::Second { t("Move To: 2nd") } else { t("Move To: Last") })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut o.move_to, MoveTo::Second, t("Move To: 2nd"));
            ui.selectable_value(&mut o.move_to, MoveTo::Last, t("Move To: Last"));
        });
    ui.checkbox(&mut o.constrain, t("Constrain"));
    let before = o.radial;
    ui.selectable_value(&mut o.radial, false, t("Linear"));
    ui.selectable_value(&mut o.radial, true, t("Radial"));
    if o.associate {
        egui::ComboBox::from_id_salt("array-integer-label")
            .selected_text(if o.label.is_empty() { t("Label: None") } else { &o.label })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut o.label, String::new(), t("Label: None"));
                for name in integer_names {
                    ui.selectable_value(&mut o.label, name.clone(), name);
                }
            });
    }
    ed.views.modify_options.constrain = o.constrain;
    ed.views.modify_options.rotate_copy = true;
    if before != o.radial
        && let Some(m) = ed.views.modify.as_mut()
    {
        m.tool = if o.radial {
            super::modify::Tool::Rotate { centre: None, start: None, placing: true }
        } else {
            super::modify::Tool::Move { copy: true, from: None }
        };
    }
    if o.radial
        && ui.button(t("Place Centre")).clicked()
        && let Some(m) = ed.views.modify.as_mut()
    {
        m.tool = super::modify::Tool::Rotate { centre: None, start: None, placing: true };
    }
}
