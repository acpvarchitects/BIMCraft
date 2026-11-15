//! FE-25: a shape handle tied to a parameter drags it. A side or an end whose place a labelled dimension sets, or whose numbers
//! are formulas of parameters (the starter box's `Width / 2`, an End associated with `Height`), is dragged by changing that
//! parameter for the type previewed (an instance parameter's own value, its default in the family): the value that puts the side
//! or end where the hand is, worked out by the flexed family (the dimension's length after the drag, the End's new number),
//! written as a dimension's drag through its label writes it (`Edit::Parameter`), live and one step of the history. Refused,
//! the draft as it was and the status bar saying why: a parameter worked out by a formula or a reporting one (both greyed, and
//! never lit), one the instance previewed sets, a value no length can be (zero or below), one the constraints can't reach, and
//! one a type can't build with. A lock, an Align lock or EQ holding a side or an end is no parameter: its handle isn't drawn.
use super::document::Draft;
use super::sketch::reader;
use super::*;
use bimcraft_core::family::Form;
use bimcraft_core::family::constraints::Edit;
use bimcraft_core::family::handles::{self as hs, Held};
use bimcraft_core::family::sketch::FormLine;
use bimcraft_core::geom::P2;
use bimcraft_core::ids::Guid;

/// What a handle's drag moves: a side of the profile (its line, the way out in the plane's axes), or the End (true) or the Start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Target {
    Edge(usize, P2),
    End(bool),
}

/// How a handle's drag works: on the numbers themselves, through one of these parameters (the first a drag moves it by), or
/// refused, why; or the handle isn't drawn.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Drive {
    Free,
    Param(Vec<String>),
    Refused(String),
    Hidden,
}

/// A parameter being dragged by a handle: the draft and the family as the drag began (what Escape and a refusal put back),
/// the form's GUID and place, what moves, the parameter, its value and where the side or end stood, and how far the drag has
/// gone (an elevation's End reads it step by step).
#[derive(Clone)]
pub(crate) struct ParamDrag {
    kept: Draft,
    family: Family,
    form: (Guid, usize),
    target: Target,
    name: String,
    v0: f64,
    m0: f64,
    slope: f64,
    total: f64,
}

/// The parameters named in `formulas`, as `f` lists them, the longest names first where one holds another.
fn named(f: &Family, formulas: &[&str]) -> Vec<String> {
    let mut names: Vec<&str> = f.parameters.iter().map(|p| p.name.as_str()).filter(|n| !n.is_empty()).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let mut out: Vec<String> = Vec::new();
    for text in formulas {
        let mut rest = text.to_string();
        for n in &names {
            if rest.contains(n) {
                rest = rest.replace(n, " ");
                if !out.iter().any(|o| o == n) {
                    out.push(n.to_string());
                }
            }
        }
    }
    out
}

fn formulas<'a>(exprs: impl IntoIterator<Item = &'a Expr>) -> Vec<&'a str> {
    exprs
        .into_iter()
        .filter_map(|e| match e {
            Expr::Formula(s) => Some(s.as_str()),
            Expr::Number(_) => None,
        })
        .collect()
}

/// `f` with parameter `name` at `v` for type `ty`: a type parameter in the type, an instance one its own value.
fn with_param(f: &Family, ty: &str, name: &str, v: f64) -> Family {
    let mut g = f.clone();
    let instance = g.parameters.iter().any(|p| p.name == name && p.instance);
    if instance {
        if let Some(p) = g.parameters.iter_mut().find(|p| p.name == name) {
            p.value = serde_json::json!(v);
        }
    } else {
        g.types.entry(ty.to_string()).or_default().insert(name.to_string(), serde_json::json!(v));
    }
    g
}

/// The form of `f` known by `guid` (else at `i`).
fn form_of(f: &Family, (guid, i): (Guid, usize)) -> Option<&Form> {
    if guid.is_nil() { f.forms.get(i) } else { f.forms.iter().find(|x| x.guid == guid) }
}

impl FamilyEditor {
    /// The name of the type previewed.
    fn type_previewed(&self) -> Option<String> {
        self.types.get(self.picked).map(|(n, _)| n.clone())
    }

    /// Why parameter `name` of `f` can't be set by a drag, if it can't.
    fn unsettable(&self, f: &Family, name: &str) -> Option<String> {
        let Some(p) = f.parameters.iter().find(|p| p.name == name) else {
            return Some(t("This edge is tied to a parameter or worked out by a formula: change it in Family Types.").to_string());
        };
        if p.formula.is_some() {
            return Some(tf("{} is worked out by a formula: change the formula in Family Types.", &[name]));
        }
        if p.reporting {
            return Some(tf("{} is a reporting parameter: it reads a dimension and never drives one.", &[name]));
        }
        if p.instance && self.preview_params.contains_key(name) {
            return Some(tf("{} is set by the instance previewed: change it there.", &[name]));
        }
        None
    }

    /// How a drag of form `i`'s handle on `target` works, in the family `drawn` (as typed).
    pub(crate) fn drive(&self, drawn: &Family, i: usize, target: Target) -> Drive {
        let Some(form) = drawn.forms.get(i) else { return Drive::Hidden };
        if self.pinned_refusal(&[super::select_box::Picked::Form(i)]).is_some() {
            return Drive::Hidden;
        }
        let values = self.values();
        let mut num = reader(&values);
        let (held, exprs): (Option<Held>, Vec<&Expr>) = match target {
            Target::Edge(k, out) => {
                let Some(line) = form.sketch.get(k) else { return Drive::Hidden };
                let held = if form.guid.is_nil() { None } else { drawn.edge_held(form.guid, line.guid) };
                if held.is_none() && super::shape_handles::greyed(&form.sketch, k, out, &mut num).is_none() {
                    return Drive::Free;
                }
                // The line's own formulas first, then its neighbours' (whose ends a drag moves too).
                let own = line.exprs();
                let rest = form.sketch.iter().flat_map(FormLine::exprs);
                (held, own.into_iter().chain(rest).collect())
            }
            Target::End(end) => {
                let extent = if end { bimcraft_core::family::Extent::End } else { bimcraft_core::family::Extent::Start };
                let held = if form.guid.is_nil() { None } else { drawn.end_held(form.guid, extent) };
                let e = if end { &form.to } else { &form.from };
                if held.is_none() && matches!(e, Expr::Number(_)) {
                    return Drive::Free;
                }
                (held, vec![e])
            }
        };
        let names = match held {
            Some(Held::Label(p)) => vec![p],
            Some(Held::Locked | Held::Equal) => return Drive::Hidden,
            None => named(drawn, &formulas(exprs)),
        };
        let free: Vec<String> = names.iter().filter(|n| self.unsettable(drawn, n).is_none()).cloned().collect();
        match (free.is_empty(), names.first()) {
            (false, _) => Drive::Param(free),
            (true, Some(n)) => Drive::Refused(self.unsettable(drawn, n).unwrap_or_default()),
            (true, None) => {
                Drive::Refused(t("This edge is tied to a parameter or worked out by a formula: change it in Family Types.").to_string())
            }
        }
    }

    /// Where `target` of the form `form` stands in `f` as type `ty` flexes it, along the handle's way: a side's middle along its
    /// way out, the End's number, the Start's less.
    fn measure(&self, f: &Family, ty: &str, form: (Guid, usize), target: Target) -> Option<f64> {
        let values = f.values(ty, &self.preview_params).ok()?;
        let flexed = f.flexed(&values).ok()?;
        let x = form_of(&flexed, form)?;
        let mut num = reader(&values);
        let m = match target {
            Target::Edge(k, out) => {
                let h = hs::edge_handles(&x.sketch, &mut num).into_iter().find(|h| h.line == k)?;
                h.at.0 * out.0 + h.at.1 * out.1
            }
            Target::End(true) => num(&x.to).ok()?,
            Target::End(false) => -num(&x.from).ok()?,
        };
        m.is_finite().then_some(m)
    }

    /// A drag of form `i`'s `target` through one of `names` begun: the first parameter that moves it, or why none can.
    pub(crate) fn start_param_drag(&mut self, i: usize, target: Target, names: &[String]) -> Result<(), String> {
        let f = self.family()?;
        let ty = self.type_previewed().ok_or_else(String::new)?;
        let form = (f.forms.get(i).map(|x| x.guid).unwrap_or_default(), i);
        let values = self.values();
        let m0 = self.measure(&f, &ty, form, target).ok_or_else(|| said_unreachable(names))?;
        for name in names {
            let Some(v0) = values.get(name).and_then(|v| match v {
                bimcraft_core::family::Val::Number(x) => Some(*x),
                _ => None,
            }) else {
                continue;
            };
            let step = (v0.abs() * 0.01).max(1.0);
            let Some(m1) = self.measure(&with_param(&f, &ty, name, v0 + step), &ty, form, target) else { continue };
            let slope = (m1 - m0) / step;
            if slope.abs() > 1e-6 {
                self.views.param_drag =
                    Some(ParamDrag { kept: self.draft(), family: f, form, target, name: name.clone(), v0, m0, slope, total: 0.0 });
                return Ok(());
            }
        }
        Err(said_unreachable(names))
    }

    /// Whether a parameter is being dragged by a handle.
    pub(crate) fn dragging_param(&self) -> bool {
        self.views.param_drag.is_some()
    }

    /// The parameter dragged a step `d` further.
    pub(crate) fn param_by(&mut self, d: f64) -> Result<(), String> {
        let total = self.views.param_drag.as_ref().map_or(0.0, |p| p.total) + d;
        self.param_to(total)
    }

    /// The parameter dragged so its side or end stands `d` along the handle's way from where it began: the value worked out
    /// and written for the type previewed, live; refused, the draft as the drag began, saying why.
    pub(crate) fn param_to(&mut self, d: f64) -> Result<(), String> {
        let Some(p) = self.views.param_drag.as_mut() else { return Ok(()) };
        if !d.is_finite() {
            return Ok(());
        }
        p.total = d;
        let p = p.clone();
        let ty = self.type_previewed().unwrap_or_default();
        self.put_back(p.kept.clone());
        if d.abs() < 1e-9 {
            return Ok(());
        }
        // Where the side or end goes, by the parameter: a step along the slope, then one more from where it lands.
        let want = p.m0 + d;
        let mut v = p.v0 + d / p.slope;
        if let Some(m) = self.measure(&with_param(&p.family, &ty, &p.name, v), &ty, p.form, p.target)
            && (m - want).abs() > 0.1
        {
            v += (want - m) / p.slope;
        }
        let refuse = |me: &mut FamilyEditor, why: String| {
            me.put_back(p.kept.clone());
            Err(why)
        };
        // To a hundredth of a millimetre, as a value is typed.
        let v = (v * 100.0).round() / 100.0;
        if !v.is_finite() {
            return refuse(self, said_unreachable(std::slice::from_ref(&p.name)));
        }
        let length = p.family.parameters.iter().find(|x| x.name == p.name).is_some_and(|x| {
            matches!(x.kind, bimcraft_core::family::Kind::Length | bimcraft_core::family::Kind::Area | bimcraft_core::family::Kind::Volume)
        });
        if length && v <= 1e-6 {
            return refuse(self, tf("{} would be {}: a length must be above zero.", &[&p.name, &number_text(v)]));
        }
        let g = with_param(&p.family, &ty, &p.name, v);
        match self.measure(&g, &ty, p.form, p.target) {
            Some(m) if (m - want).abs() <= 0.5 => {}
            _ => return refuse(self, said_unreachable(std::slice::from_ref(&p.name))),
        }
        // Every type that built before still built with it, as Finish builds a form.
        let built = |f: &Family, name: &str| {
            f.values(name, &BTreeMap::new()).and_then(|values| {
                f.solve(&values).map_err(|e| e.to_string())?;
                f.pieces(&values).map(|_| ())
            })
        };
        for name in g.types.keys() {
            if let (Ok(()), Err(e)) = (built(&p.family, name), built(&g, name)) {
                return refuse(self, format!("{name}: {e}"));
            }
        }
        self.apply_edits(vec![Edit::Parameter(p.name.clone(), v)]);
        self.views.said = Some(tf("{} = {}", &[&p.name, &number_text(v)]));
        Ok(())
    }

    /// The parameter's drag let go: the value stays, a step of the history once the button is up.
    pub(crate) fn let_param_go(&mut self) {
        self.views.param_drag = None;
    }

    /// The parameter's drag given up: the draft as it began. Whether there was one.
    pub(super) fn cancel_param(&mut self) -> bool {
        let Some(p) = self.views.param_drag.take() else { return false };
        self.put_back(p.kept);
        true
    }
}

/// Why none of `names` can move the handle's side or end where the drag takes it.
fn said_unreachable(names: &[String]) -> String {
    tf("Constraints are not satisfied: {} can't put it where the drag takes it.", &[&names.join(", ")])
}
