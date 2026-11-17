//! FE-16: what cuts what among a family's forms beyond the list's order (`family/cutting.rs`): `cut_by`, `uncut_by` and
//! `joined_by`, how they build, what refuses them, and how removing and copying forms treat them.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::engine::Session;
use crate::family::constraints::Copying;
use crate::family::{Family, Piece};
use crate::ids::Guid;

fn gs(n: u32) -> String {
    format!("fe160000-0000-4000-8000-{n:012}")
}

fn g(n: u32) -> Guid {
    serde_json::from_value(json!(gs(n))).unwrap()
}

const SLAB: u32 = 1;
const HOLE: u32 = 2;
const BLOCK: u32 = 3;

/// 1000 x 1000 x 100, from the floor.
fn slab(n: u32) -> Value {
    json!({"guid": gs(n), "profile": [[0, 0], [1000, 0], [1000, 1000], [0, 1000]], "from": 0, "to": 100, "material": "Timber"})
}

/// 200 x 200 through the middle of the slab, past its top and bottom.
fn hole(n: u32) -> Value {
    json!({"guid": gs(n), "profile": [[400, 400], [600, 400], [600, 600], [400, 600]], "from": -10, "to": 110, "void": true})
}

/// 200 x 200 x 100 in the middle of the slab, from half way up it.
fn block(n: u32) -> Value {
    json!({"guid": gs(n), "profile": [[400, 400], [600, 400], [600, 600], [400, 600]], "from": 50, "to": 150, "material": "Steel"})
}

fn family(forms: Vec<Value>) -> Family {
    serde_json::from_value(json!({"types": {"A": {}}, "forms": forms})).unwrap()
}

fn volumes(f: &Family) -> Vec<f64> {
    let pieces: Vec<Piece> = f.pieces(&f.values("A", &BTreeMap::new()).unwrap()).unwrap();
    pieces.iter().map(|p| crate::ifc_geometry_tests::volume(&p.tris).round()).collect()
}

fn with(mut form: Value, key: &str, list: &[u32]) -> Value {
    form[key] = json!(list.iter().map(|n| gs(*n)).collect::<Vec<_>>());
    form
}

const WHOLE: f64 = 1e8;
const HOLED: f64 = 1e8 - 200.0 * 200.0 * 100.0;

#[test]
fn a_void_listed_before_a_solid_cuts_it_only_through_cut_by() {
    let mut f = family(vec![slab(SLAB), hole(HOLE)]);
    assert_eq!(volumes(&f), [HOLED], "a void cuts what is listed before it, as it always did");
    let mut f2 = family(vec![hole(HOLE), slab(SLAB)]);
    assert_eq!(volumes(&f2), [WHOLE], "listed before the slab, it leaves it whole");
    f2.cut(1, 0).unwrap();
    assert_eq!(f2.forms[1].cut_by, [g(HOLE)]);
    assert!(f2.check(&|_| true).is_ok());
    assert_eq!(volumes(&f2), [HOLED]);
    f2.uncut(1, 0).unwrap();
    assert!(f2.forms[1].cut_by.is_empty());
    assert_eq!(volumes(&f2), [WHOLE]);
    f.forms.swap(0, 1);
    assert_eq!(volumes(&f), [WHOLE]);
}

#[test]
fn uncut_by_stops_a_void_listed_after_the_solid_from_cutting_it() {
    let mut f = family(vec![slab(SLAB), hole(HOLE)]);
    f.uncut(0, 1).unwrap();
    assert_eq!(f.forms[0].uncut_by, [g(HOLE)]);
    f.check(&|_| true).unwrap();
    assert_eq!(volumes(&f), [WHOLE], "restored");
    f.cut(0, 1).unwrap();
    assert!(f.forms[0].uncut_by.is_empty());
    assert_eq!(volumes(&f), [HOLED]);
    assert!(f.cut(0, 1).unwrap_err().contains("already"));
    assert!(f.uncut(0, 1).is_ok() && f.uncut(0, 1).unwrap_err().contains("doesn't cut"));
}

#[test]
fn a_solid_cuts_a_solid_and_a_join_cuts_and_draws_both() {
    let mut f = family(vec![slab(SLAB), block(BLOCK)]);
    assert_eq!(volumes(&f), [WHOLE, 4e6], "no cutting");
    f.cut(0, 1).unwrap();
    assert_eq!(volumes(&f), [WHOLE - 200.0 * 200.0 * 50.0, 4e6], "the slab is cut, the block whole");
    f.uncut(0, 1).unwrap();
    f.join(0, 1).unwrap();
    assert_eq!(f.forms[0].joined_by, [g(BLOCK)]);
    let joined = volumes(&f);
    assert_eq!(joined, [WHOLE - 200.0 * 200.0 * 50.0, 4e6]);
    let pieces = f.pieces(&f.values("A", &BTreeMap::new()).unwrap()).unwrap();
    assert_eq!((pieces[0].material.as_str(), pieces[1].material.as_str()), ("Timber", "Steel"), "both are drawn");
    // Switched, the slab cuts the block: the overlap keeps the slab's material.
    f.switch_join_order(0, 1).unwrap();
    assert_eq!(f.forms[1].joined_by, [g(SLAB)]);
    assert!(f.forms[0].joined_by.is_empty());
    assert_eq!(volumes(&f), [WHOLE, 2e6]);
    f.unjoin(1, 0).unwrap();
    assert_eq!(volumes(&f), [WHOLE, 4e6], "unjoined, as they were");
    assert!(f.unjoin(0, 1).unwrap_err().contains("aren't joined"));
}

#[test]
fn a_host_void_still_cuts_no_form() {
    let mut host = hole(HOLE);
    host["cuts"] = json!("host");
    let f = family(vec![slab(SLAB), host.clone()]);
    assert_eq!(volumes(&f), [WHOLE]);
    let mut f = family(vec![host, slab(SLAB)]);
    assert!(f.cut(1, 0).unwrap_err().contains("host alone"));
    f.forms[1].cut_by = vec![g(HOLE)];
    assert!(f.check(&|_| true).unwrap_err().contains("host alone"));
    assert_eq!(volumes(&f), [WHOLE], "and a family that holds it builds as before");
}

#[test]
fn a_hidden_form_before_the_target_does_not_shift_what_cuts_what() {
    let mut hidden = block(10);
    hidden["visible"] = json!(0);
    // Hidden, then the slab, then the hole that cuts it by the order: one piece, though the slab is the second form.
    let mut f = family(vec![hidden.clone(), slab(SLAB), hole(HOLE)]);
    assert_eq!(volumes(&f), [HOLED]);
    // The hole before the slab, named by it: the slab is the third form and the first piece.
    f = family(vec![hidden.clone(), hole(HOLE), with(slab(SLAB), "cut_by", &[HOLE])]);
    assert_eq!(volumes(&f), [HOLED]);
    // `uncut_by` is by GUID too.
    f = family(vec![hidden.clone(), with(slab(SLAB), "uncut_by", &[HOLE]), hole(HOLE)]);
    assert_eq!(volumes(&f), [WHOLE]);
    // A solid cutter hidden cuts nothing; shown, it does, whatever shifts the pieces.
    let mut cutter = block(BLOCK);
    cutter["visible"] = json!(0);
    f = family(vec![hidden, with(slab(SLAB), "joined_by", &[BLOCK]), cutter]);
    assert_eq!(volumes(&f), [WHOLE], "the hidden cutter cuts nothing");
    f.forms[2].visible = None;
    assert_eq!(volumes(&f), [WHOLE - 2e6, 4e6]);
}

#[test]
fn every_family_builds_as_it_did_with_the_lists_empty() {
    use crate::language::Language;
    let mut families: Vec<(String, Family)> = crate::family::library(Language::English).into_iter().collect();
    for (name, def) in crate::fixtures::library(Language::English) {
        if let Ok(f) = serde_json::from_value::<Family>(def) {
            families.push((name, f));
        }
    }
    families.push(("slab".into(), family(vec![slab(SLAB), hole(HOLE), block(BLOCK), hole(7)])));
    let mut host = hole(8);
    host["cuts"] = json!("host");
    let mut hidden = hole(9);
    hidden["visible"] = json!(0);
    let mut both = hole(10);
    both["cuts"] = json!("both");
    families.push(("host void between solids".into(), family(vec![slab(SLAB), host, block(BLOCK)])));
    families.push(("hidden void".into(), family(vec![slab(SLAB), hidden, block(BLOCK)])));
    families.push(("void first".into(), family(vec![hole(HOLE), slab(SLAB), block(BLOCK)])));
    families.push(("void cuts both".into(), family(vec![slab(SLAB), both])));
    families.push(("a void two solids".into(), family(vec![slab(SLAB), block(BLOCK), hole(HOLE)])));
    assert!(families.len() > 10);
    let (mut with_voids, mut drilled, mut strict) = (0, 0, 0);
    for (name, f) in &families {
        if f.forms.is_empty() || !f.nested_instances.is_empty() || f.adaptive.is_some() {
            continue;
        }
        with_voids += usize::from(f.forms.iter().any(|x| x.void));
        for t in f.types.keys() {
            let Ok(values) = f.values(t, &BTreeMap::new()) else { continue };
            // The rule it had: a void cuts the visible solids listed before it.
            let mut read = std::collections::HashMap::new();
            let mut cuts = crate::csg::Meter::default();
            let mut want: Vec<(usize, Vec<[crate::view3d::P3; 3]>)> = Vec::new();
            let mut voids = Vec::new();
            for i in 0..f.forms.len() {
                let Ok(Some(p)) = f.solid(i, &values, &mut read) else { continue };
                if f.forms[i].void {
                    if f.forms[i].cuts().forms() {
                        voids.push((want.len(), p.tris));
                    }
                } else {
                    want.push((i, p.tris));
                }
            }
            for (before, void) in &voids {
                for (_, tris) in want.iter_mut().take(*before) {
                    *tris = crate::csg::subtract(tris, void, &mut cuts).unwrap();
                }
            }
            // The same triangles, bit for bit, but where a void drilling straight through an extrusion is a hole of its
            // profile now (GEO-2e), triangulated otherwise than cut: there the same solid, its volume and box.
            let solid = |tris: &[[crate::view3d::P3; 3]]| {
                let lo = tris.iter().flatten().fold([f64::INFINITY; 3], |a, p| std::array::from_fn(|k| a[k].min(p[k])));
                let hi = tris.iter().flatten().fold([f64::NEG_INFINITY; 3], |a, p| std::array::from_fn(|k| a[k].max(p[k])));
                let r = |x: f64| (x * 1e3).round() / 1e3;
                (crate::ifc_geometry_tests::volume(tris).round(), lo.map(r), hi.map(r))
            };
            let built: std::collections::HashSet<usize> = (0..f.forms.len())
                .filter(|&i| f.forms[i].void && f.forms[i].cuts().forms() && matches!(f.solid(i, &values, &mut read), Ok(Some(_))))
                .collect();
            let got = f.pieces(&values).unwrap_or_else(|e| panic!("{name}: type {t}: {e}"));
            let want: Vec<_> = want.into_iter().filter(|(_, t)| !t.is_empty()).collect();
            assert_eq!(got.len(), want.len(), "{name}: type {t}");
            for (p, (i, w)) in got.iter().zip(&want) {
                if f.drill(*i, &|v| built.contains(&v), &values, &mut read).is_some() {
                    drilled += 1;
                    assert_eq!(solid(&p.tris), solid(w), "{name}: type {t}, form {}", i + 1);
                } else {
                    strict += 1;
                    assert!(p.tris == *w, "{name}: type {t}, form {}: not the triangles it had", i + 1);
                }
            }
        }
    }
    assert!(with_voids > 0, "some of them hold voids");
    assert!(drilled > 0 && strict > 0, "pieces drilled ({drilled}) and pieces cut as before ({strict})");
}

#[test]
fn each_rule_refuses_with_a_reason() {
    let base = |forms: Vec<Value>| family(forms).check(&|_| true);
    let said = |r: Result<(), String>, word: &str| {
        let e = r.unwrap_err();
        assert!(e.contains(word), "{e} lacks {word}");
    };
    said(base(vec![with(slab(SLAB), "cut_by", &[99]), hole(HOLE)]), "isn't a form");
    said(base(vec![with(slab(SLAB), "cut_by", &[SLAB]), hole(HOLE)]), "itself");
    said(base(vec![with(slab(SLAB), "cut_by", &[BLOCK, BLOCK]), block(BLOCK)]), "twice");
    said(base(vec![with(slab(SLAB), "cut_by", &[HOLE]), hole(HOLE)]), "cuts it already");
    said(base(vec![with(hole(HOLE), "cut_by", &[BLOCK]), block(BLOCK)]), "never cut");
    said(base(vec![with(hole(HOLE), "joined_by", &[BLOCK]), block(BLOCK)]), "never cut");
    said(base(vec![with(slab(SLAB), "joined_by", &[HOLE]), hole(HOLE)]), "a void");
    said(base(vec![with(slab(SLAB), "uncut_by", &[BLOCK]), block(BLOCK)]), "isn't a void");
    // A void listed before the form, or that cuts the host alone, wouldn't cut it anyway.
    said(base(vec![hole(HOLE), with(slab(SLAB), "uncut_by", &[HOLE])]), "anyway");
    let mut host = hole(HOLE);
    host["cuts"] = json!("host");
    said(base(vec![with(slab(SLAB), "uncut_by", &[HOLE]), host]), "anyway");
    // Loops: two, and a longer one.
    let two = vec![with(slab(SLAB), "joined_by", &[BLOCK]), with(block(BLOCK), "joined_by", &[SLAB])];
    said(base(two), "loop");
    let three = vec![with(slab(SLAB), "joined_by", &[BLOCK]), with(block(BLOCK), "cut_by", &[4]), with(block(4), "joined_by", &[SLAB])];
    said(base(three), "loop");
    // A chain is no loop.
    assert!(base(vec![with(slab(SLAB), "joined_by", &[BLOCK]), with(block(BLOCK), "joined_by", &[4]), block(4)]).is_ok());
}

#[test]
fn a_form_has_at_most_the_most_joins_a_project_element_may() {
    let n = crate::model::MAX_JOINS as u32;
    let mut forms = vec![slab(SLAB)];
    forms.extend((0..=n).map(|k| block(100 + k)));
    forms[0] = with(slab(SLAB), "joined_by", &(0..=n).map(|k| 100 + k).collect::<Vec<_>>());
    assert!(family(forms).check(&|_| true).unwrap_err().contains("more than"));
}

#[test]
fn the_api_refuses_a_form_with_no_guid_a_void_and_itself() {
    let mut nameless = block(BLOCK);
    nameless.as_object_mut().unwrap().remove("guid");
    let mut f = family(vec![slab(SLAB), nameless, hole(HOLE)]);
    assert!(f.cut(0, 1).unwrap_err().contains("no GUID"));
    assert!(f.join(1, 0).unwrap_err().contains("no GUID"));
    assert!(f.cut(0, 0).unwrap_err().contains("itself"));
    assert!(f.cut(2, 0).unwrap_err().contains("void"));
    assert!(f.join(0, 2).unwrap_err().contains("void"));
    assert!(f.cut(0, 9).unwrap_err().contains("isn't a form"));
    // A list holding a nil GUID is refused whole.
    f.forms[0].cut_by = vec![Guid::default()];
    assert!(f.check(&|_| true).is_err());
}

#[test]
fn a_join_that_would_close_a_loop_is_refused_and_leaves_the_forms_alone() {
    let mut f = family(vec![slab(SLAB), block(BLOCK), block(4)]);
    f.join(0, 1).unwrap();
    f.join(1, 2).unwrap();
    let before = f.forms.clone();
    assert!(f.join(2, 0).unwrap_err().contains("loop"));
    assert!(f.join(0, 1).unwrap_err().contains("already"));
    assert_eq!(f.forms, before);
}

#[test]
fn a_family_without_the_lists_is_written_as_it_was_and_one_with_them_reads_back() {
    let plain = family(vec![slab(SLAB), hole(HOLE)]);
    let json = serde_json::to_string(&plain).unwrap();
    assert!(!json.contains("cut_by") && !json.contains("joined_by"), "{json}");
    let again: Family = serde_json::from_str(&json).unwrap();
    assert_eq!(serde_json::to_string(&again).unwrap(), json, "bytes unchanged");
    let mut f = family(vec![hole(HOLE), slab(SLAB), block(BLOCK)]);
    f.cut(1, 0).unwrap();
    f.join(1, 2).unwrap();
    f.forms[1].uncut_by = Vec::new();
    let json = serde_json::to_string(&f).unwrap();
    let back: Family = serde_json::from_str(&json).unwrap();
    assert_eq!(back, f);
    assert!(json.contains("\"cut_by\"") && json.contains("\"joined_by\""));
}

fn session(mut forms: Vec<Value>) -> Session {
    // The project's library holds no materials of these names.
    forms.iter_mut().filter_map(Value::as_object_mut).for_each(|f| drop(f.remove("material")));
    let mut s = Session::new();
    s.execute("family", &json!({"name": "F", "types": {"A": {}}, "forms": forms})).unwrap_or_else(|e| panic!("{e}"));
    s
}

#[test]
fn the_family_command_sets_and_clears_the_lists_and_refuses_unsound_ones() {
    let mut s = session(vec![hole(HOLE), slab(SLAB)]);
    s.execute("family", &json!({"name": "F", "form": {"guid": gs(SLAB), "cut_by": [gs(HOLE)]}})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.model.families["F"].forms[1].cut_by, [g(HOLE)]);
    s.execute("family", &json!({"name": "F", "form": {"guid": gs(SLAB), "cut_by": null}})).unwrap_or_else(|e| panic!("{e}"));
    assert!(s.model.families["F"].forms[1].cut_by.is_empty());
    let r = s.execute("family", &json!({"name": "F", "form": {"guid": gs(SLAB), "joined_by": [gs(HOLE)]}}));
    assert!(r.is_err(), "a void can't be joined");
    assert!(s.model.families["F"].forms[1].joined_by.is_empty(), "rolled back");
}

#[test]
fn deleting_a_form_takes_it_from_every_list() {
    let mut s = session(vec![hole(HOLE), with(with(slab(SLAB), "cut_by", &[HOLE]), "joined_by", &[BLOCK]), block(BLOCK)]);
    s.execute("family", &json!({"name": "F", "delete_form": [gs(BLOCK)]})).unwrap_or_else(|e| panic!("{e}"));
    assert!(s.model.families["F"].forms[1].joined_by.is_empty());
    assert_eq!(s.model.families["F"].forms[1].cut_by, [g(HOLE)]);
    s.execute("family", &json!({"name": "F", "delete_form": [gs(HOLE)]})).unwrap_or_else(|e| panic!("{e}"));
    assert!(s.model.families["F"].forms[0].cut_by.is_empty());
}

fn forms_of(which: &[usize]) -> Copying {
    Copying { forms: which.to_vec(), ..Copying::default() }
}

#[test]
fn a_copy_names_the_copies_where_both_ends_are_copied_and_drops_the_rest() {
    let mut f = family(vec![hole(HOLE), slab(SLAB), block(BLOCK), block(4)]);
    f.forms[1].joined_by = vec![g(BLOCK)];
    f.forms[1].cut_by = vec![g(4)];
    let c = f.copies(&forms_of(&[1, 2])).unwrap();
    let [slab2, block2] = c.forms.as_slice() else { panic!() };
    assert_eq!(slab2.joined_by, [block2.guid], "both copied");
    assert_ne!(block2.guid, g(BLOCK));
    assert!(slab2.cut_by.is_empty(), "only one end copied: the reference is dropped");
    // The slab alone: both of its references reach past it.
    let alone = f.copies(&forms_of(&[1])).unwrap();
    assert!(alone.forms[0].joined_by.is_empty() && alone.forms[0].cut_by.is_empty());
}

#[test]
fn a_copied_solid_is_cut_by_what_cut_the_original_though_it_lands_after_the_voids() {
    let f = family(vec![slab(SLAB), hole(HOLE)]);
    let added = |f: &Family, which: &[usize]| {
        let c = f.copies(&forms_of(which)).unwrap();
        let mut all = f.clone();
        all.forms.extend(c.forms.clone());
        all.tidy_cutting();
        all.check(&|_| true).unwrap();
        (c.forms, all)
    };
    // The slab alone: its copy is after the hole, which would no longer cut it; it is named in its `cut_by`.
    let (copies, all) = added(&f, &[0]);
    assert_eq!(copies[0].cut_by, [g(HOLE)]);
    assert_eq!(volumes(&all), [HOLED, HOLED]);
    // Both, slab then hole: the hole's copy is listed after the slab's, and cuts it by the order.
    let (copies, all) = added(&f, &[0, 1]);
    assert!(copies[0].cut_by.is_empty());
    assert_eq!(volumes(&all), [HOLED, HOLED]);
    // Both, hole then slab: the hole's copy is before the slab's, which names it.
    let (copies, all) = added(&f, &[1, 0]);
    assert_eq!(copies[1].cut_by, [copies[0].guid]);
    assert_eq!(volumes(&all), [HOLED, HOLED]);
    // A void the original was told not to be cut by stays off its copy, and so does the copy of it that is listed after.
    let mut told = f.clone();
    told.forms[0].uncut_by = vec![g(HOLE)];
    let (copies, all) = added(&told, &[0]);
    assert!(copies[0].cut_by.is_empty() && copies[0].uncut_by == [g(HOLE)], "kept, for an editor that lists the copy before the void");
    assert!(all.forms[2].uncut_by.is_empty(), "stale where the copy is listed after the void, so tidied");
    assert_eq!(volumes(&all), [WHOLE, WHOLE]);
    // (A copy of the void is a new cutter, listed after the original slab, which was told nothing about it.)
    let (copies, all) = added(&told, &[0, 1]);
    assert_eq!(copies[0].uncut_by, [copies[1].guid]);
    assert_eq!(volumes(&all), [HOLED, WHOLE]);
}

#[test]
fn pasting_keeps_a_copy_cut_by_the_voids_that_cut_the_original_and_checks() {
    let mut fam = family(vec![slab(SLAB), hole(HOLE)]);
    let clip = fam.clip(&forms_of(&[0])).unwrap();
    let pasted = fam.paste(&clip).unwrap();
    assert_eq!(pasted.forms[0].cut_by, [g(HOLE)]);
    fam.forms.extend(pasted.forms);
    fam.check(&|_| true).unwrap();
    assert_eq!(volumes(&fam), [HOLED, HOLED]);
}

#[test]
fn reordering_forms_leaves_nothing_stale_once_tidied() {
    // The slab told not to be cut by the hole after it; the hole is then moved above it.
    let mut f = family(vec![with(slab(SLAB), "uncut_by", &[HOLE]), hole(HOLE)]);
    f.check(&|_| true).unwrap();
    f.forms.swap(0, 1);
    assert!(f.check(&|_| true).unwrap_err().contains("wouldn't cut"), "check stays strict");
    f.tidy_cutting();
    f.check(&|_| true).unwrap();
    assert!(f.forms[1].uncut_by.is_empty());
    // A `cut_by` void now listed after its solid cuts it by the order: redundant, refused, and tidied.
    let mut g2 = family(vec![hole(HOLE), with(slab(SLAB), "cut_by", &[HOLE])]);
    g2.check(&|_| true).unwrap();
    g2.forms.swap(0, 1);
    assert!(g2.check(&|_| true).unwrap_err().contains("cuts it already"));
    g2.tidy_cutting();
    g2.check(&|_| true).unwrap();
    assert_eq!(volumes(&g2), [HOLED]);
    // Joins and solids in the lists stay as they were.
    let mut j = family(vec![slab(SLAB), block(BLOCK)]);
    j.join(0, 1).unwrap();
    j.forms.swap(0, 1);
    j.tidy_cutting();
    assert_eq!(j.forms[1].joined_by, [g(BLOCK)]);
}

#[test]
fn the_family_command_tidies_a_reordered_list_so_it_saves() {
    let mut s = session(vec![with(slab(SLAB), "uncut_by", &[HOLE]), hole(HOLE)]);
    let forms = s.model.families["F"].forms.clone();
    let reordered = json!([forms[1], forms[0]]);
    s.execute("family", &json!({"name": "F", "forms": reordered})).unwrap_or_else(|e| panic!("{e}"));
    let f = &s.model.families["F"];
    assert!(f.forms[1].uncut_by.is_empty() && f.forms[0].void);
}

#[test]
fn cut_refuses_a_host_void_and_one_listed_after_that_cuts_already() {
    let mut host = hole(HOLE);
    host["cuts"] = json!("host");
    let mut f = family(vec![slab(SLAB), host]);
    assert!(f.cut(0, 1).unwrap_err().contains("host alone"));
    assert!(f.forms[0].cut_by.is_empty());
    // A void listed after cuts it already.
    let mut g2 = family(vec![slab(SLAB), hole(HOLE)]);
    assert!(g2.cut(0, 1).unwrap_err().contains("already"));
    // `check` refuses the redundant `cut_by`, and once tidied `uncut` works.
    g2.forms[0].cut_by = vec![g(HOLE)];
    assert!(g2.check(&|_| true).unwrap_err().contains("cuts it already"));
    g2.tidy_cutting();
    g2.uncut(0, 1).unwrap();
    assert_eq!(g2.forms[0].uncut_by, [g(HOLE)]);
    assert_eq!(volumes(&g2), [WHOLE]);
}

#[test]
fn a_join_refuses_a_pair_that_already_cuts() {
    let mut f = family(vec![slab(SLAB), block(BLOCK)]);
    f.cut(0, 1).unwrap();
    assert!(f.join(0, 1).unwrap_err().contains("already"));
    assert!(f.join(1, 0).unwrap_err().contains("already"));
    f.unjoin(0, 1).unwrap_err();
    f.uncut(0, 1).unwrap();
    f.join(1, 0).unwrap();
}

#[test]
fn a_paste_drops_a_cutter_check_would_refuse() {
    let mut host = hole(HOLE);
    host["cuts"] = json!("host");
    let mut fam = family(vec![slab(SLAB), hole(HOLE)]);
    let clip = fam.clip(&forms_of(&[0])).unwrap();
    // Pasted where the hole now cuts the host alone, the copy isn't named as cut by it.
    fam.forms[1].cuts = Some(crate::family::Cuts::Host);
    let pasted = fam.paste(&clip).unwrap();
    assert!(pasted.forms[0].cut_by.is_empty());
    fam.forms.extend(pasted.forms);
    fam.check(&|_| true).unwrap();
    // And a family without the hole takes none of its GUIDs.
    let mut other = family(vec![slab(5)]);
    let pasted = other.paste(&clip).unwrap();
    other.forms.extend(pasted.forms);
    other.check(&|_| true).unwrap();
    let _ = host;
}

#[test]
fn a_void_that_cuts_nothing_is_not_warned_of_where_a_cut_by_gives_it_a_solid() {
    let f = family(vec![hole(HOLE), with(slab(SLAB), "cut_by", &[HOLE])]);
    let (_, warnings) = f.problems(&|_| true);
    assert!(warnings.iter().all(|w| !w.contains("cuts nothing")), "{warnings:?}");
    let g2 = family(vec![hole(HOLE), slab(SLAB)]);
    let (_, warnings) = g2.problems(&|_| true);
    assert!(warnings.iter().any(|w| w.contains("cuts nothing")), "{warnings:?}");
    // Told not to, it cuts nothing.
    let h = family(vec![with(slab(SLAB), "uncut_by", &[HOLE]), hole(HOLE)]);
    let (_, warnings) = h.problems(&|_| true);
    assert!(warnings.iter().any(|w| w.contains("cuts nothing")), "{warnings:?}");
}
