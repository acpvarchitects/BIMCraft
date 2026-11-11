//! Parts of elements in IFC (`parts.rs`, `ifc.rs`): each an `IfcBuildingElementPart` its host is the sum of, read back from
//! the STEP text the export writes.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::engine::Session;
use crate::ifc;
use crate::model::Id;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

/// The file's entities by number: each one's class and its attributes, split where its own commas are.
fn read(f: &str) -> BTreeMap<u64, (String, Vec<String>)> {
    f.lines()
        .filter_map(|l| {
            let (n, rest) = l.strip_prefix('#')?.split_once('=')?;
            let (class, args) = rest.strip_suffix(");")?.split_once('(')?;
            Some((n.parse().ok()?, (class.to_string(), split(args))))
        })
        .collect()
}

/// `args` split at the commas that are in no bracket and no string.
fn split(args: &str) -> Vec<String> {
    let (mut out, mut depth, mut quoted, mut at) = (Vec::new(), 0, false, String::new());
    for c in args.chars() {
        match c {
            '\'' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                out.push(std::mem::take(&mut at));
                continue;
            }
            _ => {}
        }
        at.push(c);
    }
    out.push(at);
    out
}

/// The numbers a list of references `(#1,#2)`, or one reference `#1`, names.
fn refs(list: &str) -> Vec<u64> {
    list.trim_matches(|c| c == '(' || c == ')').split(',').filter_map(|r| r.trim().strip_prefix('#')?.parse().ok()).collect()
}

struct File {
    entities: BTreeMap<u64, (String, Vec<String>)>,
}

impl File {
    fn of(s: &Session) -> File {
        let text = ifc::export(&s.model);
        let entities = read(&text);
        // Every reference points at an entity of the file.
        for (n, (class, args)) in &entities {
            for r in args.iter().flat_map(|a| a.split(['(', ')', ','])).filter_map(|r| r.trim().strip_prefix('#')?.parse::<u64>().ok()) {
                assert!(entities.contains_key(&r), "#{n} {class} points at #{r}, which the file hasn't");
            }
        }
        File { entities }
    }

    fn args(&self, n: u64) -> &[String] {
        &self.entities[&n].1
    }

    fn class(&self, n: u64) -> &str {
        &self.entities[&n].0
    }

    /// The entities of `class`, in the order written.
    fn all(&self, class: &str) -> Vec<u64> {
        self.entities.iter().filter(|(_, (c, _))| c == class).map(|(n, _)| *n).collect()
    }

    /// The product whose tag is element `id`.
    fn product(&self, id: Id) -> u64 {
        let tag = format!("'{id}'");
        let found: Vec<u64> = self
            .entities
            .iter()
            .filter(|(_, (class, args))| args.get(7) == Some(&tag) && !class.starts_with("IFCREL") && class != "IFCOPENINGELEMENT")
            .map(|(n, _)| *n)
            .collect();
        assert_eq!(found.len(), 1, "one product tagged {id}: {found:?}");
        found[0]
    }

    /// The identifiers of the representations product `n` has: `Axis`, `Body`; none for one with no representation.
    fn representations(&self, n: u64) -> Vec<String> {
        let shape = &self.args(n)[6];
        if shape == "$" {
            return Vec::new();
        }
        refs(&self.args(refs(shape)[0])[2]).into_iter().map(|r| self.args(r)[1].trim_matches('\'').to_string()).collect()
    }

    /// What product `n` is the sum of, by the relation that says so.
    fn parts_of(&self, n: u64) -> Vec<u64> {
        let relations: Vec<u64> = self.all("IFCRELAGGREGATES").into_iter().filter(|r| refs(&self.args(*r)[4]) == [n]).collect();
        assert!(relations.len() <= 1, "one relation says what #{n} is the sum of");
        relations.first().map_or_else(Vec::new, |r| refs(&self.args(*r)[5]))
    }

    /// The box round the corners of the face set that is product `n`'s body, in its own axes.
    fn extent(&self, n: u64) -> ([f64; 3], [f64; 3]) {
        let shape = refs(&self.args(n)[6])[0];
        let rep = refs(&self.args(shape)[2])[0];
        assert_eq!((self.args(rep)[1].as_str(), self.args(rep)[2].as_str()), ("'Body'", "'Tessellation'"));
        let set = refs(&self.args(rep)[3])[0];
        assert_eq!((self.class(set), self.args(set)[2].as_str()), ("IFCTRIANGULATEDFACESET", ".T."), "a closed face set");
        let points = &self.args(refs(&self.args(set)[0])[0])[0];
        let numbers: Vec<f64> = points.split(['(', ')', ',']).filter_map(|x| x.trim().parse().ok()).collect();
        numbers.chunks(3).fold(([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]), |(lo, hi), p| {
            (std::array::from_fn(|k| lo[k].min(p[k])), std::array::from_fn(|k| hi[k].max(p[k])))
        })
    }

    /// The single values of the property set, or the quantities of the quantity set, named `set` that is product `n`'s:
    /// each name with its value as written.
    fn values(&self, n: u64, set: &str) -> Vec<(String, String)> {
        let named = format!("'{set}'");
        self.all("IFCRELDEFINESBYPROPERTIES")
            .into_iter()
            .filter(|r| refs(&self.args(*r)[4]).contains(&n))
            .map(|r| refs(&self.args(r)[5])[0])
            .filter(|d| self.args(*d)[2] == named)
            .flat_map(|d| refs(self.args(d).last().map_or("", String::as_str)))
            .map(|v| {
                let a = self.args(v);
                (a[0].trim_matches('\'').to_string(), if self.class(v) == "IFCPROPERTYSINGLEVALUE" { a[2].clone() } else { a[3].clone() })
            })
            .collect()
    }

    /// The name of the material product `n` is associated with.
    fn material(&self, n: u64) -> Option<String> {
        let relation = self.all("IFCRELASSOCIATESMATERIAL").into_iter().find(|r| refs(&self.args(*r)[4]).contains(&n))?;
        let material = refs(&self.args(relation)[5])[0];
        (self.class(material) == "IFCMATERIAL").then(|| self.args(material)[0].trim_matches('\'').to_string())
    }

    /// What is contained in a storey.
    fn contained(&self) -> Vec<u64> {
        self.all("IFCRELCONTAINEDINSPATIALSTRUCTURE").into_iter().flat_map(|r| refs(&self.args(r)[4])).collect()
    }
}

/// A historic wall (lime plaster, rubble masonry, lime plaster: 20 + 460 + 20 mm) from (1000, 2000) running north, 4 m
/// long and 3 m tall, taken apart: the session, the wall, and its three parts from its exterior side in.
fn historic() -> (Session, Id, Vec<Id>) {
    let mut s = Session::new();
    for (name, color) in [("Lime Plaster", "#e8e0d0"), ("Rubble Masonry", "#9a8f80")] {
        run(&mut s, "material", json!({"name": name, "color": color}));
    }
    run(
        &mut s,
        "type",
        json!({"category": "wall", "name": "Historic - 500mm", "layers": [
            {"function": "finish", "material": "Lime Plaster", "thickness": 20},
            {"function": "structure", "material": "Rubble Masonry", "thickness": 460},
            {"function": "finish", "material": "Lime Plaster", "thickness": 20},
        ]}),
    );
    let wall = run(&mut s, "wall", json!({"points": [[1000, 2000], [1000, 6000]], "type": "Historic - 500mm", "height": 3000}))["ids"][0]
        .as_u64()
        .unwrap();
    let parts = run(&mut s, "create_parts", json!({"id": wall}))["ids"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    (s, wall, parts)
}

#[test]
fn a_wall_taken_apart_goes_out_as_the_sum_of_its_parts() {
    let (mut s, wall, parts) = historic();
    run(&mut s, "window", json!({"at": [1000, 4000]}));
    run(
        &mut s,
        "part",
        json!({"id": parts[0], "predefined_type": "userdefined", "object_type": "USR", "su_code": "USR 3", "historical_period": "18th century",
            "decay_pathology": "efflorescence", "decay_severity": 4, "intervention_type": "Cleaning"}),
    );
    let f = File::of(&s);
    let written = f.all("IFCBUILDINGELEMENTPART");
    assert_eq!(written.len(), 3);
    let (w, of): (u64, Vec<u64>) = (f.product(wall), parts.iter().map(|id| f.product(*id)).collect());
    assert_eq!(of, written, "in the order of the wall's layers");
    // The wall is the sum of them: its axis is its own, its body theirs; they are in no storey but through it.
    assert_eq!(f.class(w), "IFCWALL");
    assert_eq!(f.representations(w), ["Axis"]);
    assert_eq!(f.parts_of(w), of);
    assert!(f.contained().contains(&w) && of.iter().all(|p| !f.contained().contains(p)));
    for (k, (&p, &id)) in of.iter().zip(&parts).enumerate() {
        let a = f.args(p);
        // Its GlobalId its own GUID, its tag its id; placed where the wall is, in the wall's own axes.
        assert_eq!(a[0], format!("'{}'", ifc::compress(s.model.guid(id).as_u128())));
        assert_eq!(a[7], format!("'{id}'"));
        assert_eq!(refs(&f.args(refs(&a[5])[0])[0]), refs(&f.args(w)[5]), "its placement is relative to the wall's");
        assert_eq!(f.representations(p), ["Body"]);
        // Along the wall from its start, across it to its left (its exterior side), up from its base.
        let (lo, hi) = f.extent(p);
        let across = [(230.0, 250.0), (-230.0, 230.0), (-250.0, -230.0)][k];
        assert_eq!(([lo[0], lo[2]], [hi[0], hi[2]]), ([0.0, 0.0], [4000.0, 3000.0]), "part {id}");
        assert!((lo[1] - across.0).abs() < 1e-6 && (hi[1] - across.1).abs() < 1e-6, "part {id}: {} to {}", lo[1], hi[1]);
    }
    // Each named by what it is made of, and made of it; IFC's kind of part, and the project's word where it has one.
    let names: Vec<&str> = of.iter().map(|p| f.args(*p)[2].as_str()).collect();
    assert_eq!(names, ["'Lime Plaster'", "'Rubble Masonry'", "'Lime Plaster'"]);
    assert_eq!(f.material(of[1]).as_deref(), Some("Rubble Masonry"));
    assert_eq!((f.args(of[0])[4].as_str(), f.args(of[0])[8].as_str()), ("'USR'", ".USERDEFINED."));
    assert_eq!((f.args(of[1])[4].as_str(), f.args(of[1])[8].as_str()), ("$", ".NOTDEFINED."));
    // What it measures, net of the window; and what the survey says of it, those said only.
    let window = 1200.0 * 1500.0;
    let net = (4000.0 * 3000.0 - window) / 1e6;
    let quantities = f.values(of[1], ifc::PART_QUANTITIES);
    let value = |name: &str| quantities.iter().find(|(n, _)| n == name).map(|(_, v)| v.parse::<f64>().unwrap()).unwrap();
    assert_eq!(quantities.len(), 3);
    assert!((value("Width") - 460.0).abs() < 1e-9 && (value("NetArea") - net).abs() < 1e-9, "{quantities:?}");
    assert!((value("NetVolume") - net * 0.46).abs() < 1e-9, "{quantities:?}");
    assert_eq!(
        f.values(of[0], crate::parts::Heritage::SET),
        [
            ("SU_Code".to_string(), "IFCTEXT('USR 3')".to_string()),
            ("Historical_Period".to_string(), "IFCTEXT('18th century')".to_string()),
            ("Decay_Pathology".to_string(), "IFCTEXT('Efflorescence')".to_string()),
            ("Decay_Severity".to_string(), "IFCINTEGER(4)".to_string()),
            ("Intervention_Type".to_string(), "IFCTEXT('Cleaning')".to_string()),
        ]
    );
    assert!(f.values(of[1], crate::parts::Heritage::SET).is_empty(), "nothing said, no set");
    // The window still fills an opening that voids the wall.
    assert_eq!(f.all("IFCRELVOIDSELEMENT").iter().filter(|r| refs(&f.args(**r)[4]) == [w]).count(), 1);
}

#[test]
fn a_part_excluded_or_in_conflict_is_not_written_and_a_wall_with_none_written_keeps_its_body() {
    let (mut s, wall, parts) = historic();
    // One left out, as Revit's Exclude Parts: the wall is the sum of the other two.
    run(&mut s, "part", json!({"id": parts[0], "excluded": true}));
    let f = File::of(&s);
    assert_eq!(f.parts_of(f.product(wall)), [f.product(parts[1]), f.product(parts[2])]);
    assert_eq!(f.representations(f.product(wall)), ["Axis"]);
    // All of them left out: the wall is itself again.
    run(&mut s, "part", json!({"ids": parts, "excluded": true}));
    let f = File::of(&s);
    assert!(f.all("IFCBUILDINGELEMENTPART").is_empty() && f.all("IFCRELAGGREGATES").len() == 3, "the project, the site, the building");
    assert_eq!(f.representations(f.product(wall)), ["Body", "Axis"]);
    // Its type swapped, its parts are in conflict: none is written, and the wall is whole.
    run(&mut s, "part", json!({"ids": parts, "excluded": false}));
    run(&mut s, "modify", json!({"id": wall, "type": "Generic - 200mm"}));
    let f = File::of(&s);
    assert!(f.all("IFCBUILDINGELEMENTPART").is_empty());
    assert_eq!(f.representations(f.product(wall)), ["Body", "Axis"]);
}

#[test]
fn a_floor_a_roof_a_ceiling_and_a_component_are_the_sum_of_their_parts_too() {
    let mut s = Session::new();
    s.script(
        r#"type {"category": "floor", "name": "Layered", "layers": [{"function": "finish", "thickness": 20}, {"function": "thermal", "thickness": 80}, {"function": "structure", "thickness": 200}]}
           floor {"boundary": [[0, 0], [4000, 0], [4000, 3000], [0, 3000]], "type": "Layered", "level": "Level 2", "openings": [[[1000, 1000], [2000, 1000], [2000, 2000], [1000, 2000]]]}
           ceiling {"boundary": [[0, 0], [4000, 0], [4000, 3000], [0, 3000]]}
           roof {"boundary": [[-500, -500], [4500, -500], [4500, 3500], [-500, 3500]], "slope": 30}
           component {"family": "WC", "at": [2000, 2000], "angle": 90}"#,
    )
    .unwrap();
    let find = |s: &Session, cat: &str| *s.model.elements.iter().find(|(_, e)| e.category() == cat).unwrap().0;
    let (floor, ceiling, roof, wc) = (find(&s, "floor"), find(&s, "ceiling"), find(&s, "roof"), find(&s, "component"));
    let whole = File::of(&s);
    let had: Vec<Vec<String>> = [floor, ceiling, roof, wc].iter().map(|id| whole.representations(whole.product(*id))).collect();
    assert!(had.iter().all(|r| r == &["Body"]), "{had:?}");
    let made = run(&mut s, "create_parts", json!({"ids": [floor, ceiling, roof, wc]}));
    let ids: Vec<Id> = made["ids"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    assert_eq!(ids.len(), 6);
    let f = File::of(&s);
    for host in [floor, ceiling, roof, wc] {
        let h = f.product(host);
        assert!(f.representations(h).is_empty(), "{} {host} has no body of its own", f.class(h));
        assert_eq!(f.parts_of(h).len(), if host == floor { 3 } else { 1 });
    }
    // The floor's layers from its top down, on Level 2's storey: heights over the level, the insulation IFC's own kind.
    let layers: Vec<u64> = ids[..3].iter().map(|id| f.product(*id)).collect();
    for (p, (z0, z1)) in layers.iter().zip([(-20.0, 0.0), (-100.0, -20.0), (-300.0, -100.0)]) {
        let (lo, hi) = f.extent(*p);
        assert_eq!(([lo[0], lo[1]], [hi[0], hi[1]]), ([0.0, 0.0], [4000.0, 3000.0]));
        assert!((lo[2] - z0).abs() < 1e-6 && (hi[2] - z1).abs() < 1e-6, "{} to {}", lo[2], hi[2]);
    }
    assert_eq!(f.args(layers[1])[8], ".INSULATION.");
    // No material said, it is named by its host's type, and made of nothing the file names.
    assert_eq!((f.args(layers[0])[2].as_str(), f.material(layers[0])), ("'Layered'", None));
    let insulation = f.values(layers[1], ifc::PART_QUANTITIES);
    assert_eq!(insulation[0], ("Width".to_string(), "80.".to_string()));
    assert_eq!(insulation[1], ("NetArea".to_string(), "11.".to_string()));
    assert_eq!(insulation[2], ("NetVolume".to_string(), "0.88".to_string()));
    // A component's part is all of it, in the component's own axes: no thickness and no face to measure, a volume.
    let fixture = f.product(ids[5]);
    let Some(crate::model::Element::Component(c)) = s.model.elements.get(wc) else { panic!("no component {wc}") };
    assert_eq!(f.args(fixture)[2], format!("'{}:{}'", c.family, c.type_name));
    let measures = f.values(fixture, ifc::PART_QUANTITIES);
    assert_eq!(measures.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["NetVolume"]);
    let (lo, hi) = f.extent(fixture);
    assert!(lo[2] >= -1e-6 && hi[2] > 100.0 && lo[0].abs() < 1000.0 && hi[0].abs() < 1000.0, "{lo:?} {hi:?}");
}

#[test]
fn a_part_that_parts_were_divided_from_is_their_sum_and_has_no_body() {
    let (mut s, wall, parts) = historic();
    // Two parts of the exterior plaster, as dividing it would leave them (each all of it, until parts are divided).
    let source = json!(crate::commands::element_json(&s.model, parts[0])["source"]);
    let division = |s: &mut Session| {
        let (host, parent) = (s.model.guid(wall), s.model.guid(parts[0]));
        let p = crate::parts::BuildingPart {
            parent: Some(parent),
            ..crate::parts::BuildingPart::new(host, serde_json::from_value(source.clone()).unwrap())
        };
        s.model_mut().insert(crate::model::Element::Part(p))
    };
    let first = division(&mut s);
    assert_eq!(s.model.check(), Ok(()));
    run(&mut s, "part", json!({"id": first, "su_code": "USR 3a"}));
    let f = File::of(&s);
    let (w, plaster, piece) = (f.product(wall), f.product(parts[0]), f.product(first));
    assert_eq!(f.parts_of(w), [plaster, f.product(parts[1]), f.product(parts[2])], "the wall's parts are its layers'");
    assert_eq!(f.parts_of(plaster), [piece]);
    assert!(f.representations(plaster).is_empty() && f.values(plaster, ifc::PART_QUANTITIES).is_empty());
    assert_eq!(f.representations(piece), ["Body"]);
    assert_eq!(f.values(piece, ifc::PART_QUANTITIES).len(), 3);
    assert_eq!(f.values(piece, crate::parts::Heritage::SET), [("SU_Code".to_string(), "IFCTEXT('USR 3a')".to_string())]);
    // The part left out, what was divided from it is left out with it.
    run(&mut s, "part", json!({"id": parts[0], "excluded": true}));
    let f = File::of(&s);
    assert_eq!(f.all("IFCBUILDINGELEMENTPART").len(), 2);
}

#[test]
fn a_parts_own_phase_and_its_values_of_the_projects_parameters_go_out_as_any_elements() {
    let (mut s, _, parts) = historic();
    run(&mut s, "parameter", json!({"name": "Surveyed By", "categories": ["part"]}));
    run(&mut s, "modify", json!({"id": parts[0], "params": {"Surveyed By": "A. Rossi", "Phase Demolished": "New Construction"}}));
    let f = File::of(&s);
    let p = f.product(parts[0]);
    assert_eq!(f.values(p, ifc::PARAMETERS), [("Surveyed By".to_string(), "IFCTEXT('A. Rossi')".to_string())]);
    // It says what it holds, and no phase it was made in: with none of its own it is in its wall's.
    assert_eq!(f.values(p, ifc::PHASING), [("Phase Demolished".to_string(), "IFCTEXT('New Construction')".to_string())]);
    assert!(f.values(f.product(parts[1]), ifc::PHASING).is_empty());
}

#[test]
fn a_materials_weight_and_heat_go_out_as_its_property_sets_in_units_the_file_names() {
    let (mut s, _, parts) = historic();
    let before = File::of(&s);
    assert!(before.all("IFCMATERIALPROPERTIES").is_empty() && before.all("IFCDERIVEDUNIT").is_empty(), "nothing said, nothing written");
    run(&mut s, "material", json!({"name": "Rubble Masonry", "density": 2100, "conductivity": 1.4, "specific_heat": 840}));
    run(&mut s, "material", json!({"name": "Lime Plaster", "density": 1600}));
    let f = File::of(&s);
    // Each material's sets, of the properties it says.
    let sets = |name: &str| -> Vec<(String, Vec<(String, String)>)> {
        f.all("IFCMATERIALPROPERTIES")
            .into_iter()
            .filter(|p| f.args(refs(&f.args(*p)[3])[0])[0] == format!("'{name}'"))
            .map(|p| {
                let props = refs(&f.args(p)[2]).into_iter().map(|v| (f.args(v)[0].trim_matches('\'').to_string(), f.args(v)[2].clone()));
                (f.args(p)[0].trim_matches('\'').to_string(), props.collect())
            })
            .collect()
    };
    assert_eq!(
        sets("Rubble Masonry"),
        [
            ("Pset_MaterialCommon".to_string(), vec![("MassDensity".to_string(), "IFCMASSDENSITYMEASURE(2100.)".to_string())]),
            (
                "Pset_MaterialThermal".to_string(),
                vec![
                    ("ThermalConductivity".to_string(), "IFCTHERMALCONDUCTIVITYMEASURE(1.4)".to_string()),
                    ("SpecificHeatCapacity".to_string(), "IFCSPECIFICHEATCAPACITYMEASURE(840.)".to_string()),
                ]
            ),
        ]
    );
    assert_eq!(
        sets("Lime Plaster"),
        [("Pset_MaterialCommon".to_string(), vec![("MassDensity".to_string(), "IFCMASSDENSITYMEASURE(1600.)".to_string())])]
    );
    // The project's units say what they are measured in: kg/m³, W/(m·K), J/(kg·K), whatever its lengths are in.
    let assignment = f.all("IFCUNITASSIGNMENT")[0];
    let derived: Vec<String> =
        refs(&f.args(assignment)[0]).into_iter().filter(|u| f.class(*u) == "IFCDERIVEDUNIT").map(|u| f.args(u)[1].clone()).collect();
    assert_eq!(derived, [".MASSDENSITYUNIT.", ".THERMALCONDUCTANCEUNIT.", ".SPECIFICHEATCAPACITYUNIT."]);
    // The part made of it is associated with the material that holds them.
    assert_eq!(f.material(f.product(parts[1])).as_deref(), Some("Rubble Masonry"));
}
