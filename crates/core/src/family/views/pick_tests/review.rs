//! Extra regression cases for family plan and elevation selection.
use super::*;
use crate::family::views;
use crate::graphics::Override;

#[test]
fn distinct_forms_elevation_crossing_and_click_follow_the_visible_part_of_each_edge() {
    let f: Family = serde_json::from_value(json!({
        "category":"generic","types":{"A":{}},"forms":[
            {"guid":"00000000-0000-4000-8000-000000000111","profile":[[-200,-80],[140,-80],[140,-50],[-200,-50]],"from":-100,"to":140},
            {"guid":"00000000-0000-4000-8000-000000000222","profile":[[-110,210],[110,210],[110,240],[-110,240]],"from":-60,"to":90},
            {"guid":"00000000-0000-4000-8000-000000000333","profile":[[-100,350],[300,350],[300,375],[-100,375]],"from":-75,"to":115}
        ]
    }))
    .unwrap();
    let (mut st, values, owner) = staged(&f, "A");
    for (side, front) in [(Side::Front, true), (Side::Back, false)] {
        let which = Which::Elevation(side);
        let centre = st.to_view(which, [0., 0., 30.]);
        let drawn = zoom_at(&mut st, which, centre, 0.24, (220, 180));
        let picture = drawn.picture.unwrap();
        let (camera, _) = crate::render::section_camera(&st.model, st.elevation(side).unwrap(), drawn.view).unwrap();
        // Form 1 is wholly behind form 0 in Front. Form 2 projects past its right side.
        // In Back form 2 now covers most of form 1, leaving only its leftmost 10 mm visible.
        let samples = [
            (1, [-110., 220., 22.], if front { -80. } else { 240. }, !front),
            (2, [30., 360., 115.], if front { -80. } else { 375. }, !front),
            (2, [200., 360., 115.], if front { 350. } else { 375. }, true),
        ];
        for (i, point, depth, visible) in samples {
            let rim = st.to_view(which, point);
            // Sample the solid just inside its silhouette; the lattice of pixels can straddle the exact edge.
            let inside = if i == 1 { [-105., point[1], point[2]] } else { [point[0], point[1], 108.] };
            let (x, y) = drawn.view.px(st.to_view(which, inside));
            assert_eq!(picture.pick(f64::from(x), f64::from(y)), Some(owner));
            let hit = picture.point(&camera, f64::from(x), f64::from(y)).unwrap();
            assert!((hit[1] - depth).abs() < 1.0, "{side:?} {point:?}: {hit:?}");
            let crossing = crate::geom::SelectBox::dragged(rim + P2(1., 1.), rim - P2(1., 1.));
            assert_eq!(crossing.takes(&views::outline_edges(&f, &values, &st, which, i)), visible, "{side:?} form {i} crossing {point:?}");
            assert_eq!(views::form_edge_at(&f, &values, &st, which, rim, 0.5), visible.then_some(i), "{side:?} form {i} click {point:?}");
        }
        // The cut point on the long horizontal rear rim is set by the front panel edge, not a sampled subdivision.
        if front {
            assert!(views::outline_edges(&f, &values, &st, which, 1).is_empty());
        }
        let outline = views::outline_edges(&f, &values, &st, which, 2);
        let boundary = st.to_view(which, [140., 350., 115.]);
        if front {
            assert!(outline.iter().flat_map(|(a, b)| [*a, *b]).any(|p| p.dist(boundary) < 1e-6));
        }
    }
}

// The jamb is inside the 1200 mm aperture. Its separate rear flange continues past the right jamb, behind the wall.
fn window() -> (Family, views::Staged, Id, Id) {
    let f: Family = serde_json::from_value(json!({
        "category":"window", "parameters":[{"name":"Width","value":1200},{"name":"Height","value":1500}],
        "types":{"A":{}}, "forms":[
            {"guid":"00000000-0000-4000-8000-000000000501", "profile":[[440,-30],[560,-30],[560,30],[440,30]], "from":400,"to":1000},
            {"guid":"00000000-0000-4000-8000-000000000502", "profile":[[420,240],[860,240],[860,280],[420,280]], "from":400,"to":1000}
        ]
    }))
    .unwrap();
    let mut s = Session::new();
    s.model_mut().families.insert("W".into(), f.clone());
    s.execute("wall", &json!({"points":[[-2500,0],[2500,0]]})).unwrap();
    let wall = s.model.elements.iter().find_map(|(id, e)| matches!(e, Element::Wall(_)).then_some(*id)).unwrap();
    let window = s.execute("window", &json!({"at":[0,0],"family":"W","type":"A","sill":0})).unwrap()["id"].as_u64().unwrap();
    let values = f.values("A", &Default::default()).unwrap();
    let staged = views::stage(std::sync::Arc::unwrap_or_clone(s.model), &f, &values, &|_| true);
    (f, staged, wall, window)
}

fn draw(st: &mut views::Staged, which: Which) -> views::Lines {
    let middle = st.to_view(which, [620., 0., 700.]);
    let view = View { cx: middle.0, cy: middle.1, width: 480, height: 320, scale: 0.6, angle: 0.0 };
    st.draw_lines_at(which, view, 1.0, P2(0., 0.)).unwrap()
}

#[test]
fn hosted_elevation_picks_the_jamb_and_the_flange_only_where_the_picture_exposes_it() {
    let (f, mut st, wall, window) = window();
    let which = Which::Elevation(Side::Front);
    let elevation = st.elevation(Side::Front).unwrap();
    let values = f.values("A", &Default::default()).unwrap();
    for (hidden, transparency) in [(false, 0), (true, 0), (false, 100)] {
        let Element::Elevation(e) = st.model.elements.get_mut(elevation).unwrap() else { panic!("expected elevation") };
        e.graphics.categories.insert("wall".into(), Override { hidden, transparency, ..Default::default() });
        let drawn = draw(&mut st, which);
        let picture = drawn.picture.unwrap();
        for (x, expected_owner, expected_form) in [(500., window, Some(0)), (750., if hidden { window } else { wall }, hidden.then_some(1))]
        {
            let q = st.to_view(which, [x, 0., 700.]);
            let px = drawn.view.px(q);
            assert_eq!(picture.pick(px.0.into(), px.1.into()), Some(expected_owner), "hidden={hidden}, transparency={transparency}, x={x}");
            assert_eq!(views::form_at(&f, &values, &st, which, q), expected_form, "hidden={hidden}, transparency={transparency}, x={x}");
        }
        let rim = st.to_view(which, [860., 0., 700.]);
        assert_eq!(views::form_edge_at(&f, &values, &st, which, rim, 2.0), hidden.then_some(1));
        let q = st.to_view(which, [750., 0., 700.]);
        let boxed = crate::geom::SelectBox::dragged(q + P2(4., -4.), q + P2(-4., 4.));
        assert_eq!(views::face_in_box(&f, &values, &st, which, views::Part::Form(1), &boxed), hidden);
    }
}

#[test]
fn a_hosted_plan_sees_a_sill_between_closed_parts_and_outlines_an_open_skin() {
    // A single closed form split in two by a horizontal void, and a single open triangle crossing the same cut plane.
    let disconnected = vec![
        json!({"profile":[[-300,-80],[300,-80],[300,80],[-300,80]],"from":100,"to":1700}),
        json!({"profile":[[-500,-150],[500,-150],[500,150],[-500,150]],"from":800,"to":1450,"void":true,"cuts":"forms"}),
    ];
    let open_skin = vec![json!({"mesh":[[[-300,-80,100],[300,-80,1700],[0,80,1700]]]})];
    for (mut forms, open) in [(disconnected, false), (open_skin, true)] {
        // The sill lies above the lower part and below the plan cut.
        forms.push(json!({"profile":[[-450,-15],[450,-15],[450,15],[-450,15]],"from":950,"to":1000}));
        let family = serde_json::from_value(json!({
            "category":"window","parameters":[{"name":"Width","value":1200},{"name":"Height","value":1800}],
            "types":{"T":{}},"forms":forms
        }))
        .unwrap();
        let mut s = Session::new();
        s.model_mut().families.insert("W".into(), family);
        s.execute("wall", &json!({"points":[[-2500,0],[2500,0]]})).unwrap();
        let window = s.execute("window", &json!({"at":[0,0],"family":"W","type":"T","sill":0})).unwrap()["id"].as_u64().unwrap();
        let pieces = s.model.opening_pieces(s.model.elements.get(window).unwrap());
        assert_eq!(pieces.len(), 2);
        let view = View { cx: 0.0, cy: 0.0, scale: 0.2, width: 600, height: 400, angle: 0.0 };
        let drawing = render::draw(&s.model, 100000, view);
        let cut = drawing
            .prims
            .iter()
            .zip(&drawing.parts)
            .zip(&drawing.owners)
            .filter(|((p, part), owner)| {
                **part == render::Part::Cut && **owner == Some(window) && matches!(p, render::Prim::Line { closed: true, .. })
            })
            .count();
        assert_eq!(cut, usize::from(open), "only the open crossing uses its hull");
        if !open {
            let probe = P2(0.0, 15.0);
            let visible = drawing.prims.iter().zip(&drawing.parts).zip(&drawing.owners).any(|((p, part), owner)| {
                *part == render::Part::Projection && *owner == Some(window)
                    && matches!(p, render::Prim::Line { pts, .. } if pts.windows(2).any(|w| geom::dist_to_segment(probe, view.mm(w[0]), view.mm(w[1])) < 0.5))
            });
            assert!(visible, "the sill edge stays visible in the gap");
            assert!(drawing.pick_all(view.px(probe), 1.0).contains(&window));
        }
    }
}
