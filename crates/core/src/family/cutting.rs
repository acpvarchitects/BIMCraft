//! FE-16: what cuts what among a family's forms, beyond the list's order. A void cuts the solid forms listed before it
//! (and never one that cuts the host alone, [`Cuts::Host`]); three optional lists of GUIDs on a solid form change that:
//! `cut_by` (more cutters: a void listed before it, which then cuts it, or a solid form, so a solid cuts a solid),
//! `uncut_by` (voids listed after it that must not cut it) and `joined_by` (solid forms joined to it: each listed form cuts it and both are drawn, so
//! where they overlap the listed form's material stays, as a project's `joined_by` does). With all three empty a family builds
//! as it always did. A void is never cut, so it has none of them.
//!
//! A cutter that isn't built (its `visible` says it isn't there) cuts nothing, as a hidden void never did; a hidden form is
//! cut by nothing, as nothing of it is built. A solid cuts as it stands before any cutting, never as what is left of it, so
//! two solids that each cut the other would empty both: a loop of `cut_by` and `joined_by` is refused ([`check_cutting`]).
//!
//! Removing a form takes its GUID from every list ([`Family::forget_cuts`]). A copy ([`Family::copies`]) names the copies
//! where both ends are copied and drops what reaches past them, as an Align lock does; but a copy lands after the family's
//! own forms, so the voids listed after the original, which cut it, would be listed before its copy and cut it no longer:
//! they are named in the copy's `cut_by`, so a copy is cut by what cut the original.

use super::*;
use crate::ids::Guid;
use crate::model::MAX_JOINS;

/// The most forms each of the lists may name, as a project's `joined_by` is limited.
pub const MAX_CUTTERS: usize = MAX_JOINS;

fn shown(i: usize) -> String {
    format!("form {}", i + 1)
}

/// Whether `forms` hold sound cutting lists: every GUID named a form of the family with a GUID (a form drawn in the editor
/// may not have one yet), not the form itself, none twice in a list, at most [`MAX_CUTTERS`] in each; a void with none of the
/// lists; `cut_by` naming no void that cuts the host alone; `joined_by` naming solid forms; `uncut_by` naming voids listed
/// after the form that would cut it, and none of them in `cut_by`; and no loop of cutting.
pub fn check_cutting(forms: &[Form]) -> Result<(), String> {
    let find = |g: Guid| forms.iter().position(|f| f.guid == g);
    for (i, form) in forms.iter().enumerate() {
        let who = shown(i);
        let lists = [("cut_by", &form.cut_by), ("uncut_by", &form.uncut_by), ("joined_by", &form.joined_by)];
        if form.void && lists.iter().any(|(_, l)| !l.is_empty()) {
            return Err(format!("{who}: a void is never cut, so `cut_by`, `uncut_by` and `joined_by` are for solid forms; leave them out"));
        }
        for (key, list) in lists {
            if list.len() > MAX_CUTTERS {
                return Err(format!("{who}: `{key}` names {} forms, more than {MAX_CUTTERS}", list.len()));
            }
            let mut seen = BTreeSet::new();
            for g in list {
                if g.is_nil() {
                    return Err(format!("{who}: `{key}` names a form with no GUID; give it one first"));
                }
                let Some(m) = find(*g) else {
                    return Err(format!("{who}: `{key}` names {g}, which isn't a form of this family"));
                };
                if m == i {
                    return Err(format!("{who}: `{key}` names the form itself"));
                }
                if !seen.insert(*g) {
                    return Err(format!("{who}: `{key}` names {} twice", shown(m)));
                }
                let by = &forms[m];
                match key {
                    "cut_by" if by.void && !by.cuts().forms() => {
                        return Err(format!("{who}: `cut_by` names {}, a void that cuts the host alone, which cuts no form", shown(m)));
                    }
                    "joined_by" if by.void => {
                        return Err(format!("{who}: `joined_by` names {}, a void; a void cuts as it is, join solid forms", shown(m)));
                    }
                    "uncut_by" => {
                        if !by.void {
                            return Err(format!(
                                "{who}: `uncut_by` names {}, which isn't a void; only a void listed after it cuts a form on its own",
                                shown(m)
                            ));
                        }
                        if m < i || !by.cuts().forms() {
                            return Err(format!(
                                "{who}: `uncut_by` names {}, which wouldn't cut it anyway (a void cuts the forms listed before it, and not if it cuts the host alone)",
                                shown(m)
                            ));
                        }
                        if form.cut_by.contains(g) {
                            return Err(format!("{who}: {} is in `cut_by` and in `uncut_by`; it can't both cut it and not", shown(m)));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // A loop: each solid and the solid forms that cut it, a walk that comes back to where it began.
    let edges: Vec<Vec<usize>> =
        forms.iter().map(|f| f.cut_by.iter().chain(&f.joined_by).filter_map(|g| find(*g)).filter(|m| !forms[*m].void).collect()).collect();
    let mut state = vec![0u8; forms.len()];
    for start in 0..forms.len() {
        let mut path: Vec<(usize, usize)> = Vec::new();
        if state[start] == 0 {
            state[start] = 1;
            path.push((start, 0));
        }
        while let Some(&(at, k)) = path.last() {
            if let Some(&next) = edges[at].get(k) {
                if let Some(top) = path.last_mut() {
                    top.1 += 1;
                }
                match state[next] {
                    0 => {
                        state[next] = 1;
                        path.push((next, 0));
                    }
                    1 => {
                        let from = path.iter().position(|(x, _)| *x == next).unwrap_or(0);
                        let names: Vec<String> = path[from..].iter().map(|(x, _)| shown(*x)).chain([shown(next)]).collect();
                        return Err(format!(
                            "{}: cut by a loop of forms ({}), which would take what they all share out of every one of them: \
                             take one of them out of the loop",
                            shown(next),
                            names.join(" is cut by ")
                        ));
                    }
                    _ => {}
                }
            } else {
                state[at] = 2;
                path.pop();
            }
        }
    }
    Ok(())
}

impl Family {
    /// The forms that cut form `i`, as their places in the list, in the order they cut it: the voids listed after it that cut
    /// forms, less its `uncut_by`; then its `cut_by`, then its `joined_by`, none twice. None for a void.
    pub fn cutters(&self, i: usize) -> Vec<usize> {
        let Some(form) = self.forms.get(i).filter(|f| !f.void) else { return Vec::new() };
        let place = |g: &Guid| if g.is_nil() { None } else { self.forms.iter().position(|f| f.guid == *g) };
        let skip: Vec<usize> = form.uncut_by.iter().filter_map(place).collect();
        let mut out: Vec<usize> =
            (i + 1..self.forms.len()).filter(|k| self.forms[*k].void && self.forms[*k].cuts().forms() && !skip.contains(k)).collect();
        for m in form.cut_by.iter().chain(&form.joined_by).filter_map(place) {
            if m != i && !out.contains(&m) {
                out.push(m);
            }
        }
        out
    }

    /// Every list of every form without `gone`, as removing that form leaves them.
    pub fn forget_cuts(&mut self, gone: Guid) {
        if gone.is_nil() {
            return;
        }
        for f in &mut self.forms {
            f.cut_by.retain(|g| *g != gone);
            f.uncut_by.retain(|g| *g != gone);
            f.joined_by.retain(|g| *g != gone);
        }
    }

    /// Forms `target` and `cutter` as places in the list, each with a GUID, and different.
    fn pair(&self, target: usize, cutter: usize) -> Result<(Guid, Guid), String> {
        let at = |i: usize| self.forms.get(i).ok_or_else(|| format!("{} isn't a form of this family", shown(i)));
        let (t, c) = (at(target)?, at(cutter)?);
        if target == cutter {
            return Err(format!("{} can't cut itself", shown(target)));
        }
        for (i, f) in [(target, t), (cutter, c)] {
            if f.guid.is_nil() {
                return Err(format!("{} has no GUID yet; give it one first", shown(i)));
            }
        }
        Ok((t.guid, c.guid))
    }

    /// Makes `edit` to the forms, kept where they stay sound ([`check_cutting`]) and left as they were where they don't.
    fn recut(&mut self, edit: impl FnOnce(&mut Vec<Form>)) -> Result<(), String> {
        let mut forms = self.forms.clone();
        edit(&mut forms);
        check_cutting(&forms)?;
        self.forms = forms;
        Ok(())
    }

    /// `cutter` made to cut `target`, a solid form: a void listed after it that was told not to is let cut again; any other
    /// void, or a solid, is named in its `cut_by`. Refused where it cuts it already, or where the lists would not be sound.
    pub fn cut(&mut self, target: usize, cutter: usize) -> Result<(), String> {
        let (t, c) = self.pair(target, cutter)?;
        if self.forms[target].void {
            return Err(format!("{} is a void, and a void is never cut", shown(target)));
        }
        if self.cutters(target).contains(&cutter) {
            return Err(format!("{} cuts {} already", shown(cutter), shown(target)));
        }
        self.recut(|forms| {
            if cutter > target && forms[cutter].void {
                forms[target].uncut_by.retain(|g| *g != c);
            } else {
                forms[target].cut_by.push(c);
            }
        })
        .map_err(|e| format!("{e} (cutting {t} by {c})"))
    }

    /// `cutter` made not to cut `target` any more: a void listed after it is named in its `uncut_by`, any other taken from its
    /// `cut_by`. A joined form is taken off with [`Family::unjoin`]. Refused where it doesn't cut it.
    pub fn uncut(&mut self, target: usize, cutter: usize) -> Result<(), String> {
        let (_, c) = self.pair(target, cutter)?;
        if self.forms[target].joined_by.contains(&c) {
            return Err(format!("{} is joined to {}: unjoin them instead", shown(cutter), shown(target)));
        }
        if !self.cutters(target).contains(&cutter) {
            return Err(format!("{} doesn't cut {}", shown(cutter), shown(target)));
        }
        self.recut(|forms| {
            if forms[target].cut_by.contains(&c) {
                forms[target].cut_by.retain(|g| *g != c);
            } else {
                forms[target].uncut_by.push(c);
            }
        })
    }

    /// Solid form `cutter` joined to solid form `target`: it cuts it and both are drawn. Refused where either is a void or
    /// has no GUID, where they are joined already, where it would close a loop, or where `target` is cut by [`MAX_CUTTERS`]
    /// forms already.
    pub fn join(&mut self, target: usize, cutter: usize) -> Result<(), String> {
        let (_, c) = self.pair(target, cutter)?;
        for i in [target, cutter] {
            if self.forms[i].void {
                return Err(format!("{} is a void; join solid forms", shown(i)));
            }
        }
        let (t, k) = (&self.forms[target], &self.forms[cutter]);
        if t.joined_by.contains(&c) || k.joined_by.contains(&t.guid) {
            return Err(format!("{} and {} are joined already", shown(target), shown(cutter)));
        }
        self.recut(|forms| forms[target].joined_by.push(c))
    }

    /// Forms `a` and `b` not joined any more, whichever cut the other. Refused where they aren't joined.
    pub fn unjoin(&mut self, a: usize, b: usize) -> Result<(), String> {
        let (ga, gb) = self.pair(a, b)?;
        if !self.forms[a].joined_by.contains(&gb) && !self.forms[b].joined_by.contains(&ga) {
            return Err(format!("{} and {} aren't joined", shown(a), shown(b)));
        }
        self.recut(|forms| {
            forms[a].joined_by.retain(|g| *g != gb);
            forms[b].joined_by.retain(|g| *g != ga);
        })
    }

    /// The join of `a` and `b` turned round: the one that cut the other is cut by it. Refused where they aren't joined.
    pub fn switch_join_order(&mut self, a: usize, b: usize) -> Result<(), String> {
        let (ga, gb) = self.pair(a, b)?;
        let a_cut_by_b = self.forms[a].joined_by.contains(&gb);
        if !a_cut_by_b && !self.forms[b].joined_by.contains(&ga) {
            return Err(format!("{} and {} aren't joined", shown(a), shown(b)));
        }
        let (cut, cutter, gcut, gcutter) = if a_cut_by_b { (a, b, ga, gb) } else { (b, a, gb, ga) };
        self.recut(|forms| {
            forms[cut].joined_by.retain(|g| *g != gcutter);
            forms[cutter].joined_by.push(gcut);
        })
    }
}
