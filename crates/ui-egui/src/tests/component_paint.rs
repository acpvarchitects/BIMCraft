//! FE-16c-2: Paint (PT) and Remove Paint on a placed component in a project's 3D view: a click paints the face clicked (its
//! form, the nested instances down to it and its key), not all of it; Remove Paint takes that face's paint off, and where the
//! face has none of its own but the whole component is painted, takes that off as before.
use super::*;
use bimcraft_core::family::FormFace;
use bimcraft_core::ids::Guid;

const FORM: &str = "fe16c200-0000-4000-8000-000000000100";

/// A box 2000 by 1000, 600 high, and a component of it at the origin; a red material.
fn script() -> String {
    let l =
        |n: u32, a: [i32; 2], b: [i32; 2]| format!(r#"{{"guid": "fe16c200-0000-4000-8000-00000000000{n}", "start": {a:?}, "end": {b:?}}}"#);
    format!(
        "material {{\"name\": \"Red\", \"color\": [220, 30, 30]}}\nfamily {{\"name\": \"Box\", \"category\": \"generic\", \"types\": {{\"T\": {{}}}}, \"forms\": [{{\"guid\": \"{FORM}\", \"sketch\": [{}, {}, {}, {}], \"from\": 0, \"to\": 600, \"material\": \"Concrete\"}}]}}\ncomponent {{\"family\": \"Box\", \"type\": \"T\", \"at\": [0, 0]}}",
        l(1, [0, 0], [2000, 0]),
        l(2, [2000, 0], [2000, 1000]),
        l(3, [2000, 1000], [0, 1000]),
        l(4, [0, 1000], [0, 0]),
    )
}

fn the_component(h: &Harness<BimApp>) -> (Id, bimcraft_core::model::Component) {
    h.state()
        .session
        .model
        .elements
        .iter()
        .find_map(|(id, e)| match e {
            Element::Component(c) => Some((*id, c.clone())),
            _ => None,
        })
        .expect("the component")
}

#[test]
fn paint_paints_the_face_of_a_component_clicked_in_3d_and_remove_paint_takes_it_off() {
    let mut h = app(&script());
    press(&mut h, "Default 3D View");
    topmost(&mut h, "Modify");
    h.state_mut().paint_material = "Red".into();
    press(&mut h, "Paint");
    assert_eq!(h.state().tool, Tool::Paint);
    let top = h.state().screen3([500.0, 500.0, 600.0]);
    click_at(&mut h, top);
    let (id, c) = the_component(&h);
    let painted: Vec<(Guid, Vec<Guid>, FormFace, String)> =
        c.face_paint.iter().map(|p| (p.form, p.nested.clone(), p.face.clone(), p.material.clone())).collect();
    assert_eq!(painted, [(Guid::parse(FORM).unwrap(), vec![], FormFace::End, "Red".to_string())], "{}", h.state().status.text);
    assert!(field(&h, id, "params").is_null(), "the face, not all of it");
    assert_eq!(h.state().session.history().last().copied(), Some("Paint"));
    // Remove Paint on that face takes its paint off.
    press(&mut h, "Remove Paint");
    click_at(&mut h, top);
    assert!(the_component(&h).1.face_paint.is_empty(), "{}", h.state().status.text);
    // The whole component painted (as a command or a file of before may have it): Remove Paint on a face with no paint of its
    // own takes that off, as it did.
    h.state_mut().session.execute("paint", &json!({"id": id, "face": "all", "material": "Red"})).unwrap();
    h.run();
    click_at(&mut h, top);
    assert!(field(&h, id, "params").is_null(), "{}", h.state().status.text);
}
