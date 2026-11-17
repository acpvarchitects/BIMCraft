use monstertruck::meshing::prelude::*;
use monstertruck::modeling::builder::SweepAngle;
use monstertruck::modeling::*;
use std::collections::{BTreeMap, HashSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

fn poly_face(pts: &[Point3]) -> std::result::Result<Face, String> {
    let vs: Vec<Vertex> = pts.iter().map(|p| builder::vertex(*p)).collect();
    let n = vs.len();
    let w: Wire = (0..n).map(|i| builder::line(&vs[i], &vs[(i + 1) % n])).collect();
    builder::try_attach_plane(vec![w]).map_err(|e| format!("{e:?}"))
}
fn p(x: f64, y: f64, z: f64) -> Point3 { Point3::new(x, y, z) }
fn cuboid(a: Point3, b: Point3) -> Solid { primitive::cuboid(BoundingBox::from_iter([a, b])) }

fn report(name: &str, f: impl FnOnce() -> std::result::Result<Solid, String>) {
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(f));
    let dt = t.elapsed();
    let s = match r {
        Err(_) => { println!("| {name} | PANIC | | | {:.2} ms | |", dt.as_secs_f64() * 1e3); return; }
        Ok(Err(e)) => { println!("| {name} | ERR: {} | | | {:.2} ms | |", e.chars().take(200).collect::<String>(), dt.as_secs_f64() * 1e3); return; }
        Ok(Ok(s)) => s,
    };
    let mut fk: BTreeMap<String, usize> = BTreeMap::new();
    let mut ek: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen = HashSet::new();
    let mut nf = 0;
    for sh in s.boundaries() {
        for f in sh.face_iter() {
            nf += 1;
            let k = format!("{:?}", f.surface()); let k = k.split('(').next().unwrap_or("").to_string();
            *fk.entry(k).or_default() += 1;
        }
        for e in sh.edge_iter() {
            if seen.insert(e.id()) {
                let k = format!("{:?}", e.curve()); let k = k.split('(').next().unwrap_or("").to_string();
                *ek.entry(k).or_default() += 1;
            }
        }
    }
    let t2 = Instant::now();
    let m = catch_unwind(AssertUnwindSafe(|| s.triangulation(0.5).to_polygon()));
    let dt2 = t2.elapsed();
    let ms = match m {
        Ok(pm) => format!("{} tris + {} quads + {} other, {:.2} ms", pm.tri_faces().len(), pm.quad_faces().len(), pm.other_faces().len(), dt2.as_secs_f64() * 1e3),
        Err(_) => "mesh PANIC".into(),
    };
    println!("| {name} | ok | {nf} faces {fk:?} | {} edges {ek:?} | {:.2} ms | {ms} |", seen.len(), dt.as_secs_f64() * 1e3);
}

fn main() {
    println!("| test | result | faces | edges | op time | tess 0.5mm |\n|---|---|---|---|---|---|");
    report("1a extrude rect", || Ok(builder::extrude(&poly_face(&[p(0.,0.,0.), p(1000.,0.,0.), p(1000.,600.,0.), p(0.,600.,0.)])?, Vector3::new(0., 0., 750.))));
    report("1b extrude L", || Ok(builder::extrude(&poly_face(&[p(0.,0.,0.), p(1000.,0.,0.), p(1000.,200.,0.), p(200.,200.,0.), p(200.,600.,0.), p(0.,600.,0.)])?, Vector3::new(0., 0., 750.))));
    let ring_face = || poly_face(&[p(100.,0.,0.), p(300.,0.,0.), p(300.,0.,500.), p(100.,0.,500.)]);
    for div in [2usize, 4] {
        report(&format!("2 revolve ring (division {div})"), || Ok(builder::revolve(&ring_face()?, Point3::origin(), Vector3::unit_z(), SweepAngle::Closed, div)));
    }
    report("2x revolve division 0 (hostile)", || Ok(builder::revolve(&ring_face()?, Point3::origin(), Vector3::unit_z(), SweepAngle::Closed, 0)));
    report("3a L path: no sweep-along-path builder; fuse 2 boxes", || {
        let a = cuboid(p(0.,-100.,-100.), p(1100.,100.,100.)); let b = cuboid(p(900.,-100.,-100.), p(1100.,800.,100.));
        monstertruck::solid::or(&a, &b, 0.05).map_err(|e| format!("{e:?}")) });
    report("3b arc sweep as partial revolve 90deg", || {
        let f = poly_face(&[p(0.,-100.,-100.), p(0.,100.,-100.), p(0.,100.,100.), p(0.,-100.,100.)])?;
        Ok(builder::revolve(&f, p(0.,1000.,0.), Vector3::unit_z(), SweepAngle::Partial(Rad(std::f64::consts::FRAC_PI_2)), 1)) });
    report("4 loft square->circle (skin + caps)", || {
        let vs: Vec<Vertex> = [p(500.,0.,0.), p(500.,500.,0.), p(-500.,500.,0.), p(-500.,-500.,0.), p(500.,-500.,0.)].iter().map(|q| builder::vertex(*q)).collect();
        // square split at (500,0) so it has 4 edges starting at angle 0: corners at 45,135,225,315 deg; make 4 edges each spanning 90 deg
        let sqv: Vec<Vertex> = [p(500.,500.,0.), p(-500.,500.,0.), p(-500.,-500.,0.), p(500.,-500.,0.)].iter().map(|q| builder::vertex(*q)).collect();
        let _ = vs;
        let w0: Wire = (0..4).map(|i| builder::line(&sqv[i], &sqv[(i + 1) % 4])).collect();
        let r = 200.0; let c = std::f64::consts::FRAC_1_SQRT_2 * r;
        let cv: Vec<Vertex> = [p(c,c,900.), p(-c,c,900.), p(-c,-c,900.), p(c,-c,900.)].iter().map(|q| builder::vertex(*q)).collect();
        let mids = [p(0.,r,900.), p(-r,0.,900.), p(0.,-r,900.), p(r,0.,900.)];
        let w1: Wire = (0..4).map(|i| builder::circle_arc(&cv[i], &cv[(i + 1) % 4], mids[i])).collect();
        let side: Shell = builder::try_skin_wires(&[w0.clone(), w1.clone()]).map_err(|e| format!("{e:?}"))?;
        let mut bottom: Face = builder::try_attach_plane(vec![w0]).map_err(|e| format!("{e:?}"))?;
        bottom.invert();
        let top: Face = builder::try_attach_plane(vec![w1]).map_err(|e| format!("{e:?}"))?;
        let mut sh = side; sh.push(bottom); sh.push(top);
        Solid::try_new(vec![sh]).map_err(|e| format!("{e:?}")) });
    report("6a box - cylinder", || {
        let a = cuboid(p(0.,0.,0.), p(1000.,600.,750.));
        let seed = builder::vertex(p(600.,300.,-100.));
        let rim: Wire = builder::revolve(&seed, p(500.,300.,-100.), Vector3::unit_z(), SweepAngle::Closed, 4);
        let base = builder::try_attach_plane(vec![rim]).map_err(|e| format!("{e:?}"))?;
        let cyl: Solid = builder::extrude(&base, Vector3::new(0.,0.,950.));
        monstertruck::solid::difference(&a, &cyl, 0.05).map_err(|e| format!("{e:?}")) });
    report("6b box - coplanar box", || { let a = cuboid(p(0.,0.,0.), p(1000.,600.,750.)); let b = cuboid(p(200.,0.,0.), p(800.,300.,750.)); monstertruck::solid::difference(&a, &b, 0.05).map_err(|e| format!("{e:?}")) });
    report("6c fuse boxes sharing face", || { let a = cuboid(p(0.,0.,0.), p(1000.,600.,750.)); let b = cuboid(p(1000.,0.,0.), p(1500.,600.,750.)); monstertruck::solid::or(&a, &b, 0.05).map_err(|e| format!("{e:?}")) });
    report("6d fuse partly shared face", || { let a = cuboid(p(0.,0.,0.), p(1000.,600.,750.)); let b = cuboid(p(1000.,100.,0.), p(1500.,400.,500.)); monstertruck::solid::or(&a, &b, 0.05).map_err(|e| format!("{e:?}")) });
    report("6e ring(revolve) - box", || { let r = builder::revolve(&ring_face()?, Point3::origin(), Vector3::unit_z(), SweepAngle::Closed, 4); let b = cuboid(p(0.,-400.,250.), p(400.,400.,600.)); monstertruck::solid::difference(&r, &b, 0.05).map_err(|e| format!("{e:?}")) });
    report("hostile: NaN face", || Ok(builder::extrude(&poly_face(&[p(0.,0.,0.), p(f64::NAN,0.,0.), p(1.,1.,0.)])?, Vector3::new(0.,0.,750.))));
    report("hostile: self-intersecting face", || Ok(builder::extrude(&poly_face(&[p(0.,0.,0.), p(10.,10.,0.), p(10.,0.,0.), p(0.,10.,0.)])?, Vector3::new(0.,0.,5.))));
}
