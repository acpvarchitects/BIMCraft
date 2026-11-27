//! The drawing and picking of a hosted window follow the model when it moves away from the origin.

use super::*;

const POSITIONS: [f64; 3] = [0.0, 2_000_000.0, -2_000_000.0];

/// The sill crosses both sides of the cut frame. It is higher than the frame's base, so a face click in a
/// jamb needs its real cut cap to pick the frame. The wall opening may be rectangular or cut by an authored void.
fn window_at(x: f64, explicit: bool) -> (Session, Family, Id) {
    let mut forms = vec![
        json!({"profile":[[-800,-15],[800,-15],[800,15],[-800,15]],"from":900,"to":940}),
        json!({"sketch":[
            {"start":[-480,-75],"end":[480,-75]}, {"start":[480,-75],"end":[480,75]},
            {"start":[480,75],"end":[-480,75]}, {"start":[-480,75],"end":[-480,-75]},
            {"start":[-350,-35],"end":[-350,35]}, {"start":[-350,35],"end":[350,35]},
            {"start":[350,35],"end":[350,-35]}, {"start":[350,-35],"end":[-350,-35]}
        ],"from":100,"to":1500}),
    ];
    if explicit {
        forms.push(json!({"profile":[[-600,-500],[600,-500],[600,500],[-600,500]],"from":-10,"to":1700,"void":true,"cuts":"host"}));
    }
    let family: Family = serde_json::from_value(json!({
        "category":"window","parameters":[{"name":"Width","value":1200},{"name":"Height","value":1800}],"types":{"T":{}},"forms":forms,
    }))
    .unwrap();
    let mut session = Session::new();
    session.model_mut().families.insert("Window".into(), family.clone());
    session.execute("wall", &json!({"points":[[x-2500.,0],[x+2500.,0]]})).unwrap();
    let window = session.execute("window", &json!({"at":[x,0],"family":"Window","type":"T","sill":0})).unwrap()["id"].as_u64().unwrap();
    (session, family, window)
}

#[test]
fn the_raw_outlines_and_coverage_of_a_horizontal_cut_follow_its_world_position() {
    for x in POSITIONS {
        let (session, _, window) = window_at(x, false);
        let pieces = session.model.opening_pieces(session.model.elements.get(window).unwrap());
        assert_eq!(pieces.len(), 2);
        let mesh = &pieces[1].tris;
        let mut areas: Vec<_> = plan_cut_outlines(mesh, 1200.0).iter().map(|r| geom::area(r).abs().round() as i64).collect();
        areas.sort_unstable();
        assert_eq!(areas, [700 * 70, 960 * 150], "inner and outer boundaries at {x}");
        for regions in [plan_cut(mesh, 1200.0), view3d::level_slice(mesh, 1200.0)] {
            let area = regions.iter().map(|r| geom::area(r).abs()).sum::<f64>();
            assert!((area - 95_000.0).abs() < 0.01, "frame area at {x} was {area}");
            for (dx, inside) in [(15.0, false), (400.0, true), (540.0, false)] {
                assert_eq!(regions.iter().any(|r| geom::contains(r, P2(x + dx, 3.0))), inside, "cut at {x}+{dx}");
            }
        }
    }
}

#[test]
fn a_hosted_plan_draws_and_picks_the_same_frame_and_sill_at_each_world_position() {
    for explicit in [false, true] {
        for x in POSITIONS {
            let (session, family, window) = window_at(x, explicit);
            let view = View { cx: x, cy: 0.0, width: 600, height: 400, scale: 0.2, angle: 0.0 };
            let drawing = render::draw(&session.model, 100000, view);
            let edges = seen_lines(&drawing, window);
            let values = family.values("T", &BTreeMap::new()).unwrap();
            let mut staged = stage(std::sync::Arc::unwrap_or_clone(session.model), &family, &values, &|_| true);
            staged.draw_lines_at(Which::Plan, view, 1.0, P2(0., 0.)).unwrap();
            // Inside the frame's hole, on the jamb, inside the wall opening past the frame, and beyond the opening.
            for (dx, face) in [(15., Some(0)), (400., Some(1)), (540., Some(0)), (700., None)] {
                let rim = P2(x + dx, 15.0);
                let visible = edges.iter().any(|(a, b)| geom::dist_to_segment(rim, *a, *b) < 0.5);
                assert_eq!(visible, face == Some(0), "project sill at {x}+{dx}, explicit={explicit}");
                let picked = form_at(&family, &values, &staged, Which::Plan, P2(x + dx, 3.0));
                assert_eq!(picked, face, "face at {x}+{dx}, explicit={explicit}");
                let edge = form_edge_at(&family, &values, &staged, Which::Plan, rim, 0.5);
                assert_eq!(edge, face.filter(|i| *i == 0), "sill edge at {x}+{dx}, explicit={explicit}");
            }
        }
    }
}
