//! A door's or window's opening across phases (T25). An opening cuts its wall only where it stands: made after a view's phase,
//! or taken down before it, it isn't there and the wall is whole; there but in a status the view's phase filter doesn't
//! display (a new door in a demolition plan, a demolished one in a plan of what is built), the wall is whole too, as Revit
//! fills a demolished door's opening with an infill. A view's Visibility/Graphics hiding a door still leaves its opening.
//! Nothing is stored: the infill is worked out from the opening's phases and its wall's.

use super::{CREATED, DEMOLISHED, PhaseFilter, Presentation, Status, index, value};
use crate::model::{Element, Model};

/// When an element stands, as places in the project's phases: the phase it is made in, and the one it comes down in (none:
/// never). A door or window with no phase of its own takes its wall's ([`value`]).
pub(crate) type Lifetime = (usize, Option<usize>);

pub(crate) fn lifetime(m: &Model, e: &Element) -> Lifetime {
    let created = value(m, e, CREATED).and_then(|n| index(m, n)).unwrap_or(0);
    (created, value(m, e, DEMOLISHED).and_then(|n| index(m, n)))
}

/// Whether two openings stand in their wall at once: neither comes down in a phase before the other is made, nor in the one
/// it is made in, which is how a door replaces another (the old one demolished, the new one made, both in that phase).
pub(crate) fn coexist(a: Lifetime, b: Lifetime) -> bool {
    let gone_by = |x: Lifetime, y: Lifetime| x.1.is_some_and(|d| d <= y.0);
    !gone_by(a, b) && !gone_by(b, a)
}

/// Marks an infill's `vis` ([`crate::view3d`]'s triangles and solids): a family form's Visibility Settings set bit 31 and
/// never this one, and 0 is drawn everywhere.
const INFILL: u32 = 1 << 30;
const SETTINGS: u32 = 1 << 31;
/// No phase, in the eight bits that hold one.
const NEVER: u32 = 0xff;

/// The `vis` of opening `e`'s infill: where the opening stands, packed (made in the low eight bits, taken down in the next
/// eight); none where it stands whenever its wall does, so it never needs one, or past the 254 phases eight bits hold.
pub(crate) fn infill_bits(m: &Model, e: &Element, wall: &Element) -> Option<u32> {
    let (c, d) = lifetime(m, e);
    let (wc, wd) = lifetime(m, wall);
    let gone_with_wall = match (d, wd) {
        (None, _) => true,
        (Some(d), Some(wd)) => wd <= d,
        (Some(_), None) => false,
    };
    if c <= wc && gone_with_wall {
        return None;
    }
    let c = u32::try_from(c).ok().filter(|c| *c < NEVER)?;
    let d = match d {
        Some(d) => u32::try_from(d).ok().filter(|d| *d < NEVER)?,
        None => NEVER,
    };
    Some(INFILL | c | d << 8)
}

/// Where an infill's opening stands, from its `vis`; none for anything else's.
pub(crate) fn infill_of(bits: u32) -> Option<Lifetime> {
    if bits & INFILL == 0 || bits & SETTINGS != 0 {
        return None;
    }
    let d = (bits >> 8) & 0xff;
    Some(((bits & 0xff) as usize, (d != NEVER).then_some(d as usize)))
}

/// Whether a view of the phase at `at` through `filter` fills the opening of what stands `life`: where it isn't there, and
/// where the filter doesn't display it as what it is there.
pub(crate) fn filled(life: Lifetime, at: usize, filter: Option<&PhaseFilter>) -> bool {
    let (c, d) = life;
    if c > at || d.is_some_and(|d| d < at) {
        return true;
    }
    let status = match d {
        Some(d) if d == at => {
            if c == at {
                Status::Temporary
            } else {
                Status::Demolished
            }
        }
        _ if c == at => Status::New,
        _ => Status::Existing,
    };
    filter.is_some_and(|f| f.of(status) == Presentation::NotDisplayed)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::engine::Session;
    use crate::geom::P2;
    use crate::graphics::testing::{PAPER, house, owned, pixel, plan, run};
    use crate::graphics::{Graphics, Look};
    use crate::view3d::Scene;

    const NEW: &str = "New Construction";

    fn id(v: Value) -> u64 {
        v["id"].as_u64().unwrap()
    }

    /// A door at x 2000 in the house's south wall, made in phase `made` (none: it follows its wall).
    fn door(s: &mut Session, made: Option<&str>) -> u64 {
        s.phase = made.map(str::to_string);
        let d = id(run(s, "door", json!({"at": [2000, 0]})));
        s.phase = None;
        d
    }

    /// Level 1's plan at `phase` through `filter`.
    fn shows(s: &mut Session, phase: &str, filter: Option<&str>) -> crate::render::Drawing {
        run(s, "graphics", json!({"view": 100000, "phase": phase, "phase_filter": filter}));
        plan(s, 100000)
    }

    /// Whether the plan at `phase` through `filter` leaves the door's opening in the south wall clear.
    fn open(s: &mut Session, phase: &str, filter: Option<&str>) -> bool {
        pixel(&shows(s, phase, filter), P2(2000.0, 0.0)) == PAPER
    }

    #[test]
    fn a_new_door_in_an_existing_wall_is_no_opening_before_it_is_made() {
        let mut s = Session::new();
        house(&mut s);
        door(&mut s, Some(NEW));
        assert!(!open(&mut s, "Existing", None), "the existing wall is whole: the door isn't made yet");
        assert!(open(&mut s, NEW, None));
        assert!(open(&mut s, NEW, Some("Show Previous + New")));
        assert!(!open(&mut s, NEW, Some("Show Previous + Demolished")), "a demolition plan doesn't show what is new, nor its hole");
    }

    #[test]
    fn a_demolished_door_is_open_where_it_comes_down_and_filled_after() {
        let mut s = Session::new();
        house(&mut s);
        let d = door(&mut s, None);
        run(&mut s, "modify", json!({"id": d, "params": {"Phase Demolished": NEW}}));
        assert!(open(&mut s, "Existing", None), "it stands in Existing");
        assert!(open(&mut s, NEW, Some("Show Previous + Demolished")), "its demolition is drawn, in an open wall");
        assert!(open(&mut s, NEW, Some("Show All")));
        assert!(!open(&mut s, NEW, Some("Show Previous + New")), "the infill: the wall is whole");
        assert!(!open(&mut s, NEW, Some("Show Complete")));
        // The infill is new where the door comes down: drawn by its category, its existing wall in halftone.
        let d = shows(&mut s, NEW, Some("Show Previous + New"));
        let (infill, wall) = (pixel(&d, P2(2000.0, 0.0)), pixel(&d, P2(4000.0, 0.0)));
        assert!(infill.0 < wall.0, "infill {infill:?}, wall {wall:?}");
        run(&mut s, "phase", json!({"name": "Fit-out"}));
        assert!(!open(&mut s, "Fit-out", None), "and stays whole after");
        let d = shows(&mut s, "Fit-out", Some("Show Previous + New"));
        assert_eq!(pixel(&d, P2(2000.0, 0.0)), pixel(&d, P2(4000.0, 0.0)), "after, it is the wall");
    }

    #[test]
    fn a_door_hidden_by_the_view_still_leaves_its_opening() {
        let mut s = Session::new();
        house(&mut s);
        door(&mut s, Some(NEW));
        run(&mut s, "graphics", json!({"view": 100000, "hide": ["doors"]}));
        assert!(open(&mut s, NEW, None));
    }

    #[test]
    fn a_door_made_where_one_comes_down_replaces_it() {
        let mut s = Session::new();
        house(&mut s);
        let old = door(&mut s, None);
        let e = s.execute("door", &json!({"at": [2000, 0]})).unwrap_err();
        assert!(e.contains("overlap"), "two doors standing at once: {e}");
        run(&mut s, "modify", json!({"id": old, "params": {"Phase Demolished": NEW}}));
        s.phase = Some("Existing".into());
        let e = s.execute("door", &json!({"at": [2000, 0]})).unwrap_err();
        assert!(e.contains("overlap"), "both stand in Existing: {e}");
        s.phase = None;
        let new = door(&mut s, Some(NEW));
        assert_ne!(old, new);
        // Both are drawn where they stand together, the old one as demolished, and neither fills the other's opening.
        for filter in ["Show All", "Show Previous + Demolished", "Show Previous + New", "Show Complete"] {
            assert!(open(&mut s, NEW, Some(filter)), "{filter}");
        }
        let d = shows(&mut s, NEW, Some("Show All"));
        assert!(!owned(&d, old).is_empty() && !owned(&d, new).is_empty());
        // Nor in 3D: each keeps its hole, and neither's infill fills the other's.
        let Some(crate::model::Element::Door(o)) = s.model.elements.get(new) else { panic!("{new} is a door") };
        let wall = s.model.elements.id(o.host);
        for (phase, filter) in [("Existing", None), (NEW, Some("Show Previous + New")), (NEW, Some("Show Previous + Demolished"))] {
            let g = Graphics { phase: Some(phase.into()), phase_filter: filter.map(str::to_string), ..Graphics::default() };
            assert!(!filled_in_3d(&s, wall.unwrap(), &Look::with(&s.model, &g)), "{phase} {filter:?}");
        }
        // The replacement can't be taken down before the one it replaces is.
        let e = s.execute("modify", &json!({"id": old, "params": {"Phase Demolished": null}})).unwrap_err();
        assert!(e.contains("overlap"), "{e}");
    }

    /// Whether `look` shows anything of wall `wall` across the door's opening (x 1700 to 2300, 200 to 1800 up).
    fn filled_in_3d(s: &Session, wall: u64, look: &Look) -> bool {
        let scene = Scene::new(&s.model);
        scene.facets(look).any(|f| {
            let c = [0, 1, 2].map(|k| f.corners.iter().map(|q| q[k]).sum::<f64>() / 3.0);
            f.owner == wall && (1700.0..2300.0).contains(&c[0]) && (200.0..1800.0).contains(&c[2]) && c[1].abs() < 150.0
        })
    }

    #[test]
    fn three_d_fills_an_opening_where_the_door_isnt_there() {
        let mut s = Session::new();
        let walls = house(&mut s);
        let d = door(&mut s, None);
        run(&mut s, "modify", json!({"id": d, "params": {"Phase Demolished": NEW}}));
        let look = |phase: &str, filter: Option<&str>| {
            Look::with(&s.model, &Graphics { phase: Some(phase.into()), phase_filter: filter.map(str::to_string), ..Graphics::default() })
        };
        assert!(!filled_in_3d(&s, walls[0], &look("Existing", None)));
        assert!(!filled_in_3d(&s, walls[0], &look(NEW, Some("Show Previous + Demolished"))));
        assert!(filled_in_3d(&s, walls[0], &look(NEW, Some("Show Previous + New"))));
        assert!(!filled_in_3d(&s, walls[0], &look(NEW, None)), "with no filter, what comes down is drawn: open");
    }

    #[test]
    fn a_section_cuts_the_wall_whole_where_the_door_isnt_there() {
        let mut s = Session::new();
        house(&mut s);
        door(&mut s, Some(NEW));
        // Across the south wall through the door's middle: 1500 along it is the wall's centre line, where the door's leaf is
        // drawn, so a point within the wall's thickness beside it.
        let section = id(run(&mut s, "section", json!({"start": [2000, -1500], "end": [2000, 1500]})));
        let mut at = |phase: &str| {
            run(&mut s, "graphics", json!({"view": section, "phase": phase}));
            let view = crate::render::section_fit(&s.model, section, 800, 600);
            let d = crate::render::section(&s.model, section, view);
            if let Ok(dir) = std::env::var("T25_DUMP") {
                std::fs::write(format!("{dir}/section-{phase}.png"), crate::render::png(&d).unwrap()).unwrap();
            }
            pixel(&d, P2(1450.0, 1000.0))
        };
        let (before, after) = (at("Existing"), at(NEW));
        assert_ne!(before, after, "cut whole in Existing, open once the door is made");
        assert_eq!(at("Existing"), before);
    }

    #[test]
    fn a_project_with_no_phases_draws_its_openings_as_before() {
        let mut s = Session::new();
        let walls = house(&mut s);
        door(&mut s, None);
        assert!(open(&mut s, NEW, None));
        assert!(!filled_in_3d(&s, walls[0], &Look::of_3d(&s.model)));
    }

    /// The pictures of T25's PR: a house whose south wall has a door taken down and replaced in New Construction, a new door
    /// in it and a window taken down in the east wall, in plan through each phase filter, a section and the 3D view; written
    /// to the folder `BIMCRAFT_T25_DUMP` names, and nothing without it.
    #[test]
    fn t25_screenshots() {
        let Ok(dir) = std::env::var("BIMCRAFT_T25_DUMP") else { return };
        let mut s = Session::new();
        house(&mut s);
        let old = door(&mut s, None);
        run(&mut s, "modify", json!({"id": old, "params": {"Phase Demolished": NEW}}));
        door(&mut s, Some(NEW));
        s.phase = Some(NEW.into());
        run(&mut s, "door", json!({"at": [5500, 0]}));
        s.phase = None;
        let window = id(run(&mut s, "window", json!({"at": [8000, 3000]})));
        run(&mut s, "modify", json!({"id": window, "params": {"Phase Demolished": NEW}}));
        run(&mut s, "section", json!({"start": [2000, -1500], "end": [2000, 1500]}));
        let shots: [(&str, &str, Option<&str>); 5] = [
            ("existing", "Existing", None),
            ("demolition", NEW, Some("Show Previous + Demolished")),
            ("new", NEW, Some("Show Previous + New")),
            ("complete", NEW, Some("Show Complete")),
            ("all", NEW, Some("Show All")),
        ];
        for (name, phase, filter) in shots {
            for view in ["Level 1", "Section 1", "{3D}"] {
                run(&mut s, "graphics", json!({"view": view, "phase": phase, "phase_filter": filter}));
                let file = format!("{dir}/{}-{name}.png", view.replace(['{', '}', ' '], ""));
                let mut p = json!({"view": view, "width": 1000, "height": 700, "path": file});
                if view == "{3D}" {
                    p["azimuth"] = json!(200);
                    p["elevation"] = json!(15);
                }
                run(&mut s, "render", p);
            }
        }
    }
}
