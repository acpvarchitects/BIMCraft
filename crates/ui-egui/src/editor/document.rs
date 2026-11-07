//! Conventional family documents. Their draft stays separate from the project's Session until Load into Project.
use super::*;
use crate::dock::Docking;
use egui::{CentralPanel, Frame, Panel};

#[derive(Clone, Copy, Default, PartialEq)]
enum RibbonTab {
    #[default]
    Create,
    Insert,
    View,
    Manage,
    Modify,
}

pub(super) struct State {
    docking: Docking,
    tab: RibbonTab,
    dialog: Option<Dialog>,
    pub(super) preview_size: [u32; 2],
    pub(super) association: Option<super::association::Choice>,
    undo: Vec<Draft>,
    redo: Vec<Draft>,
    transaction: Option<Draft>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            docking: Docking::revit(),
            tab: RibbonTab::Create,
            dialog: None,
            association: None,
            undo: Vec::new(),
            redo: Vec::new(),
            transaction: None,
            preview_size: [360, 300],
        }
    }
}

/// Text snapshots include invalid intermediate input; project history is never consulted.
#[derive(Clone, PartialEq)]
struct Draft {
    name: String,
    category: Category,
    params: Vec<ParamText>,
    types: Vec<(String, BTreeMap<String, String>)>,
    forms: Vec<FormText>,
    symbols: Vec<SymbolText>,
    operation: Option<Operation>,
    picked: usize,
}
impl Draft {
    fn of(ed: &FamilyEditor) -> Self {
        Self {
            name: ed.name.clone(),
            category: ed.category,
            params: ed.params.clone(),
            types: ed.types.clone(),
            forms: ed.forms.clone(),
            symbols: ed.symbols.clone(),
            operation: ed.operation,
            picked: ed.picked,
        }
    }
    fn restore(self, ed: &mut FamilyEditor) {
        ed.name = self.name;
        ed.category = self.category;
        ed.params = self.params;
        ed.types = self.types;
        ed.forms = self.forms;
        ed.symbols = self.symbols;
        ed.operation = self.operation;
        ed.picked = self.picked;
        ed.preview = None;
        ed.problem = None;
        ed.document.dialog = None;
        ed.document.association = None;
    }
}

enum Dialog {
    Types { params: Vec<ParamText>, types: Vec<(String, BTreeMap<String, String>)>, picked: usize },
    Category { name: String, category: Category, operation: Option<Operation> },
}

#[derive(Default)]
struct Asked {
    apply: bool,
    undo: bool,
    redo: bool,
    close_after_load: bool,
    close: bool,
    load: bool,
    save: bool,
    favorite: bool,
}

impl BimApp {
    pub(crate) fn family_doc(&self) -> bool {
        self.family_editor.is_some()
    }

    pub(crate) fn family_title(&self) -> Option<String> {
        let ed = self.family_editor.as_ref()?;
        let name = if ed.name.trim().is_empty() { t("New Family") } else { ed.name.trim() };
        Some(format!("{name} · {{3D}}"))
    }

    pub(crate) fn family_document_ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.painter().rect_filled(ui.max_rect(), 0., crate::canvas::color(bimcraft_core::render::PAPER));
        let before = self.family_editor.as_ref().map(Draft::of);
        let was_modal = self.family_editor.as_ref().is_some_and(|ed| ed.document.dialog.is_some() || ed.document.association.is_some());
        let mut asked = Asked::default();
        if !was_modal && !ctx.egui_wants_keyboard_input() {
            asked.undo = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z));
            asked.redo = ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
                    || i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z)
            });
        }
        Panel::top("family-title").frame(Frame::new().fill(pal().app_bar).inner_margin(8.0)).show(ui, |ui| {
            let bar = ui.max_rect();
            if self.own_bar() {
                crate::titlebar::drag_area(ui, bar);
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(self.family_title().unwrap_or_default()).strong().color(pal().app_bar_text));
            });
            if self.own_bar() {
                crate::titlebar::caption_buttons(ui, bar);
            }
        });
        Panel::top("family-ribbon").show(ui, |ui| {
            let Some(ed) = self.family_editor.as_mut() else { return };
            ui.horizontal(|ui| {
                for (tab, label) in [
                    (RibbonTab::Create, msg!("Create")),
                    (RibbonTab::Insert, msg!("Insert")),
                    (RibbonTab::View, msg!("View")),
                    (RibbonTab::Manage, msg!("Manage")),
                    (RibbonTab::Modify, "Modify"),
                ] {
                    ui.selectable_value(&mut ed.document.tab, tab, t(label));
                }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                if ui.button(t("Family Category and Parameters")).clicked() {
                    ed.document.dialog = Some(Dialog::Category { name: ed.name.clone(), category: ed.category, operation: ed.operation });
                }
                if ui.button(t("Family Types")).clicked() {
                    ed.document.dialog = Some(Dialog::Types { params: ed.params.clone(), types: ed.types.clone(), picked: ed.picked });
                }
                asked.undo |= ui.add_enabled(!ed.document.undo.is_empty(), egui::Button::new(t("Undo"))).clicked();
                asked.redo |= ui.add_enabled(!ed.document.redo.is_empty(), egui::Button::new(t("Redo"))).clicked();
                asked.apply = ui.button(t("Load into Project")).clicked();
                asked.close_after_load = ui.button(t("Load into Project and Close")).clicked();
            });
            ui.horizontal_wrapped(|ui| match ed.document.tab {
                RibbonTab::Create => {
                    ui.label(t("Forms"));
                    for label in [msg!("Extrusion"), msg!("Blend"), msg!("Revolve"), msg!("Sweep"), msg!("Swept Blend"), msg!("Void Forms")]
                    {
                        ui.add_enabled(false, egui::Button::new(t(label))).on_hover_text(t("Not in BIMCraft yet"));
                    }
                }
                RibbonTab::Insert => {
                    asked.load = ui.button(t("Load Family…")).clicked();
                    asked.save = ui.button(t("Save Family…")).clicked();
                    asked.favorite = ui.button(t("Add to Favorites")).clicked();
                }
                RibbonTab::View => {
                    if ui.button(t("Zoom to Fit")).clicked() {
                        ed.look = (AZIMUTH, ELEVATION);
                    }
                }
                RibbonTab::Manage => {
                    ui.label(t("Family Editor"));
                }
                RibbonTab::Modify => {
                    ui.label(t("Edit the selected form in Properties."));
                }
            });
        });
        let Some(ed) = self.family_editor.as_ref() else { return };
        let project_docking = std::mem::replace(&mut self.docking, ed.document.docking.clone());
        self.docks(ui);
        let family_docking = std::mem::replace(&mut self.docking, project_docking);
        if let Some(ed) = self.family_editor.as_mut() {
            ed.document.docking = family_docking;
        }
        CentralPanel::default().frame(Frame::new().fill(pal().panel)).show(ui, |ui| {
            let Some(mut ed) = self.family_editor.take() else { return };
            ui.horizontal(|ui| {
                ui.strong(format!("{} · {{3D}}", ed.name));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    asked.close = ui.button(t("Close Family")).clicked();
                });
            });
            ui.separator();
            let room = ui.available_size();
            let (rect, response) = ui.allocate_exact_size(room, Sense::drag());
            self.view_rect = rect;
            ed.document.preview_size = [rect.width().clamp(64., 1600.) as u32, rect.height().clamp(64., 1200.) as u32];
            if response.dragged_by(PointerButton::Middle) && ctx.input(|i| i.modifiers.shift) {
                let delta = response.drag_delta();
                ed.look = (ed.look.0 - f64::from(delta.x) * 0.5, (ed.look.1 + f64::from(delta.y) * 0.5).clamp(-89., 89.));
            }
            ed.refresh(&ctx, &self.session.model);
            if let Some((_, tile)) = &ed.preview {
                ui.painter().image(tile.id(), rect, Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)), Color32::WHITE);
            }
            if ed.document.dialog.is_none()
                && let Some(problem) = &ed.problem
            {
                // Accessible diagnostics, in the document rather than over the project's view.
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(8.)), |ui| {
                    ui.label(RichText::new(problem).color(pal().error));
                });
            }
            self.family_editor = Some(ed);
        });
        self.dialogs(&ctx);
        if let Some(ed) = self.family_editor.take() {
            self.finish_family_document(&ctx, ed, asked, before, was_modal);
        }
    }

    pub(crate) fn family_properties(&mut self, ui: &mut Ui) {
        let Some(mut ed) = self.family_editor.take() else { return };
        let materials: Vec<String> = self.session.model.materials.keys().cloned().collect();
        field(ui, t("Family name").into(), &mut ed.name, 200.);
        ui.label(category_label(ed.category));
        ui.separator();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut ed.tab, Tab::Forms, t("Forms"));
            ui.selectable_value(&mut ed.tab, Tab::Symbols, t("Symbols"));
        });
        match ed.tab {
            Tab::Forms => forms_tab(&mut ed, ui, &materials),
            Tab::Symbols => symbols_tab(&mut ed, ui),
        }
        self.family_editor = Some(ed);
    }

    pub(crate) fn family_browser(&mut self, ui: &mut Ui) {
        let Some(ed) = self.family_editor.as_mut() else { return };
        CollapsingHeader::new(t("Views (all)")).default_open(true).show(ui, |ui| {
            ui.label("{3D}");
            for label in [msg!("Ref. Level"), msg!("Front"), msg!("Back"), msg!("Left"), msg!("Right")] {
                ui.add_enabled(false, egui::Button::new(t(label))).on_hover_text(t("Not in BIMCraft yet"));
            }
        });
        CollapsingHeader::new(t("Forms")).default_open(true).show(ui, |ui| {
            for (i, form) in ed.forms.iter().enumerate() {
                if ui.button(format!("{}. {}", i + 1, form.describe())).clicked() {
                    ed.tab = Tab::Forms;
                }
            }
        });
        ui.label(tf("Connectors: {}", &[&ed.connectors.len().to_string()]));
    }

    pub(super) fn family_document_dialogs(&mut self, ctx: &Context) {
        let Some(mut ed) = self.family_editor.take() else { return };
        let Some(dialog) = ed.document.dialog.take() else {
            super::association::dialog(ctx, &mut ed);
            self.family_editor = Some(ed);
            return;
        };
        let materials: Vec<String> = self.session.model.materials.keys().cloned().collect();
        let (mut ok, mut cancel) = (false, false);
        match dialog {
            Dialog::Types { mut params, mut types, mut picked } => {
                let r = Modal::new(UiId::new("family-types"), t("Family Types")).width(1000.).show(ctx, |ui| {
                    crate::popup::body(ui, |ui| {
                        parameters_tab(&mut ed, ui, &materials);
                        ui.separator();
                        types_tab(&mut ed, ui, &materials);
                        if let Some(problem) = &ed.problem {
                            ui.label(RichText::new(problem).color(pal().error));
                        }
                    });
                    crate::popup::footer(ui, |ui| {
                        if ui.button(t("Apply")).clicked() && ed.problem.is_none() {
                            params = ed.params.clone();
                            types = ed.types.clone();
                            picked = ed.picked;
                        }
                        ok = ui.button(t("OK")).clicked();
                        cancel = ui.button(t("Cancel")).clicked();
                    });
                });
                if cancel || r.should_close() {
                    ed.params = params;
                    ed.types = types;
                    ed.picked = picked;
                } else if !ok || ed.problem.is_some() {
                    ed.document.dialog = Some(Dialog::Types { params, types, picked });
                }
            }
            Dialog::Category { name, category, operation } => {
                let r = Modal::new(UiId::new("family-category"), t("Family Category and Parameters")).width(520.).show(ctx, |ui| {
                    crate::popup::body(ui, |ui| {
                        field(ui, t("Name").into(), &mut ed.name, 300.);
                        ui.label(t("Category"));
                        ComboBox::from_id_salt("family-category").selected_text(category_label(ed.category)).show_ui(ui, |ui| {
                            for c in CATEGORIES {
                                ui.selectable_value(&mut ed.category, c, category_label(c));
                            }
                        });
                    });
                    crate::popup::footer(ui, |ui| {
                        ok = ui.button(t("OK")).clicked();
                        cancel = ui.button(t("Cancel")).clicked();
                    });
                });
                if cancel || r.should_close() {
                    ed.name = name;
                    ed.category = category;
                    ed.operation = operation;
                } else if !ok {
                    ed.document.dialog = Some(Dialog::Category { name, category, operation });
                }
            }
        }
        super::association::dialog(ctx, &mut ed);
        self.family_editor = Some(ed);
    }

    fn finish_family_document(&mut self, ctx: &Context, mut ed: FamilyEditor, asked: Asked, before: Option<Draft>, was_modal: bool) {
        if asked.undo || asked.redo {
            let previous = if asked.undo { ed.document.undo.pop() } else { ed.document.redo.pop() };
            if let Some(previous) = previous {
                let current = Draft::of(&ed);
                if asked.undo {
                    ed.document.redo.push(current);
                } else {
                    ed.document.undo.push(current);
                }
                previous.restore(&mut ed);
            }
        } else {
            let now_modal = ed.document.dialog.is_some() || ed.document.association.is_some();
            let step = if !was_modal && now_modal {
                ed.document.transaction = before;
                None
            } else if was_modal && !now_modal {
                ed.document.transaction.take().or(before)
            } else if now_modal {
                None
            } else {
                before
            };
            if let Some(before) = step
                && before != Draft::of(&ed)
            {
                if ed.document.undo.len() >= 100 {
                    ed.document.undo.remove(0);
                }
                ed.document.undo.push(before);
                ed.document.redo.clear();
            }
        }
        if asked.close {
            return;
        }
        let base = Arc::clone(&self.session.model);
        if asked.load {
            let dialog = rfd::AsyncFileDialog::new().add_filter(t("BIMCraft family"), &["json"]);
            self.ask_file(ctx, dialog, false, |app, path| match read_family_file(&path) {
                Ok((name, f)) => {
                    if let Some(open) = app.family_editor.as_mut() {
                        *open = FamilyEditor::of(&name, &f);
                        open.original = None;
                    }
                }
                Err(text) => app.status = Status { text, error: true },
            });
        }
        if asked.apply || asked.close_after_load {
            match ed.command(&base) {
                Ok(p) => {
                    if self.run("family", p).is_some() {
                        ed.original = Some(ed.name.trim().to_string());
                        if asked.close_after_load {
                            return;
                        }
                    } else {
                        ed.problem = Some(std::mem::take(&mut self.status).text);
                    }
                }
                Err(e) => ed.problem = Some(e),
            }
        }
        // Save and Favorites use a private Session: neither implicitly loads the draft into the project.
        if asked.save || asked.favorite {
            let prepared = ed.command(&base).and_then(|p| {
                let mut draft = Session::new();
                draft.model = Arc::clone(&base);
                draft.execute("family", &p)?;
                Ok(draft)
            });
            match prepared {
                Ok(mut draft) => {
                    let name = ed.name.trim().to_string();
                    if asked.save {
                        let dialog =
                            rfd::AsyncFileDialog::new().add_filter(t("BIMCraft family"), &["json"]).set_file_name(format!("{name}.json"));
                        self.ask_file(ctx, dialog, true, move |app, path| {
                            if let Err(text) = draft.execute("family", &json!({"name":name,"save":path.to_string_lossy()})) {
                                app.status = Status { text, error: true };
                            }
                        });
                    } else {
                        let p = self.favorite_params("save", "family", &name);
                        if let Err(e) = draft.execute("favorite", &p) {
                            ed.problem = Some(e);
                        }
                    }
                }
                Err(e) => ed.problem = Some(e),
            }
        }
        self.family_editor = Some(ed);
    }
}
