//! Hard cases for CadCore (git, pinned): one case per run, named by argv[1]; one JSON line out.
//! Cases and answers: harness/hardcases.py. CadCore has boxes, circles swept along polylines
//! (cylinders), an N-way union and a half-space (plane) cut; a case that needs any other
//! operation is reported "no_op", never approximated.
use cadcore::geom::Plane3;
use cadcore::math::{Point3, UnitVec3, Vec3};
use cadcore::mesh::{mesh_brep, mesh_volume, ImportOptions};
use cadcore::ops::{build_solid_box, sweep_circle_along_polyline, SweepOptions};
use cadcore::topo::{BRep, FaceId, ShellId};
use cadcore::union::{half_space_cut, validate_shell, Diagnostics, UnionConfig};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

const KNIT: f64 = 1e-6;
enum Res { Done(BRep, ShellId), NoOp(&'static str) }
type Out = Result<Res, String>;

fn faces_of(b: &BRep) -> (Vec<FaceId>, ShellId) {
    let sid = b.solids.keys().next().expect("a solid");
    let sh = b.solids[sid].shells[0];
    (b.shells[sh].faces.clone(), sh)
}
fn bx(a: [f64; 3], c: [f64; 3]) -> Result<BRep, String> {
    let mut b = BRep::new();
    build_solid_box(&mut b, a[0], c[0], a[1], c[1], a[2], c[2], None).map_err(|e| format!("{e:?}"))?;
    Ok(b)
}
fn cyl(a: [f64; 3], c: [f64; 3], r: f64) -> Result<BRep, String> {
    let mut b = BRep::new();
    sweep_circle_along_polyline(&mut b, &[Point3::new(a[0], a[1], a[2]), Point3::new(c[0], c[1], c[2])], r, &SweepOptions::default()).map_err(|e| format!("{e:?}"))?;
    Ok(b)
}
fn done(b: BRep) -> Out { let (_, s) = faces_of(&b); Ok(Res::Done(b, s)) }
fn union(a: &BRep, b: &BRep) -> Out {
    let (fa, _) = faces_of(a); let (fb, _) = faces_of(b);
    match cadcore::union::boolean::union::union_n(&[(a, &fa), (b, &fb)], KNIT) { Some((o, s)) => Ok(Res::Done(o, s)), None => Err("union gave nothing".into()) }
}
fn plane_cut(a: &BRep, origin: [f64; 3], keep_normal: [f64; 3]) -> Out {
    let (fa, _) = faces_of(a);
    let n = UnitVec3::try_from_vec(Vec3::new(keep_normal[0], keep_normal[1], keep_normal[2])).ok_or("bad normal")?;
    let p = Plane3::from_origin_normal(Point3::new(origin[0], origin[1], origin[2]), n);
    match half_space_cut(a, &fa, &p, KNIT) { Some(c) => Ok(Res::Done(c.brep, c.shell)), None => Err("cut gave nothing".into()) }
}

fn run(name: &str) -> Option<Out> {
    let s01 = 0.1f64.to_radians().tan();
    let t30 = 30f64.to_radians().tan();
    let no = |w: &'static str| Ok(Res::NoOp(w));
    Some(match name {
        "1a loft rect to apex" | "1b loft side shrinks to zero" | "3g box minus a thin NURBS panel" | "4b ruled loft minus a cylinder" | "4c two lofts fused" => no("no loft"),
        "1c revolve profile touching axis" | "1d revolve profile crossing axis" | "1e revolve cone apex on axis" | "1f cone cut through its apex" | "1g cone apex inside a box fuse" | "4d revolved ring cut by an inclined plane" => no("no revolve"),
        "2b cylinder tangent inside a face cut" | "2f void face on a body face cut" | "3a cup wall 1mm" | "3b cup wall 0.1mm" | "3c cup wall 0.01mm" | "3d sliver 0.01mm from a near-miss cut" | "4a cylinder minus a cylinder at 45deg" | "6i box cut by itself" => no("no solid difference (only a plane cut)"),
        "5a fillet plane-plane" | "5b fillet plane-cylinder" | "5c fillet NURBS-plane" | "5d fillet NURBS-NURBS" => no("no fillet on an existing edge"),
        "5e shell a cylinder open at the top" | "5f offset a cylindrical face" => no("no shell or face offset"),
        "6j extrude a NaN profile" | "6k extrude by zero" | "6l extrude a self-intersecting profile" => no("no extrude of a profile"),
        "2a cylinder tangent to a face fuse" => (|| union(&bx([0., 0., 0.], [1000., 600., 200.])?, &cyl([0., 300., 300.], [1000., 300., 300.], 100.)?))(),
        "2c equal cylinders crossing at 90deg fuse" => (|| union(&cyl([-500., 0., 0.], [500., 0., 0.], 100.)?, &cyl([0., -500., 0.], [0., 500., 0.], 100.)?))(),
        "2d coplanar faces partly overlapping fuse" => (|| union(&bx([0., 0., 0.], [1000., 600., 750.])?, &bx([500., 300., 0.], [1500., 900., 750.])?))(),
        "2e boxes stacked sharing a face fuse" => (|| union(&bx([0., 0., 0.], [1000., 600., 750.])?, &bx([0., 0., 750.], [1000., 600., 1000.])?))(),
        // a plane cut is CadCore's own operation for these two: keep y <= y0 + x tan(0.1 deg)
        "3e plane cut at 0.1deg" => (|| plane_cut(&bx([0., 0., 0.], [1000., 600., 750.])?, [0., 300., 0.], [s01, -1., 0.]))(),
        "3f wedge of 0.1deg" => (|| plane_cut(&bx([0., 0., 0.], [1000., 600., 750.])?, [0., 0., 0.], [s01, -1., 0.]))(),
        // keep z <= 250 + x tan(30 deg)
        "4e cylinder cut by an inclined plane" => (|| plane_cut(&cyl([0., 0., 0.], [0., 0., 500.], 300.)?, [0., 0., 250.], [t30, 0., -1.]))(),
        "6a box with a NaN corner" => (|| done(bx([0., f64::NAN, 0.], [1000., 600., 750.])?))(),
        "6b box of zero thickness" => (|| done(bx([0., 0., 0.], [1000., 600., 0.])?))(),
        "6c box with inverted corners" => (|| done(bx([1000., 600., 750.], [0., 0., 0.])?))(),
        "6d cylinder of radius 0" => (|| done(cyl([0., 0., 0.], [0., 0., 750.], 0.)?))(),
        "6e cylinder of radius NaN" => (|| done(cyl([0., 0., 0.], [0., 0., 750.], f64::NAN)?))(),
        "6f cylinder of negative radius" => (|| done(cyl([0., 0., 0.], [0., 0., 750.], -100.)?))(),
        "6g unit box at 1e9 from the origin" => (|| done(bx([1e9, 1e9, 1e9], [1e9 + 1., 1e9 + 1., 1e9 + 1.])?))(),
        "6h box fused with itself" => (|| union(&bx([0., 0., 0.], [1000., 600., 750.])?, &bx([0., 0., 0.], [1000., 600., 750.])?))(),
        _ => return None,
    })
}

fn esc(s: &str) -> String { s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ") }
fn num(x: Option<f64>) -> String { match x { Some(v) if v.is_finite() => format!("{v:.17e}"), _ => "null".into() } }

fn main() {
    let name = std::env::args().nth(1).expect("case name");
    std::panic::set_hook(Box::new(|_| {}));
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| run(&name)));
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let head = format!("{{\"case\":\"{}\",\"kernel\":\"cadcore\",\"ms\":{ms:.3}", esc(&name));
    match r {
        Err(p) => { let s = p.downcast_ref::<String>().cloned().or(p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default(); println!("{head},\"outcome\":\"panic\",\"err\":\"{}\"}}", esc(&s)); }
        Ok(None) => { eprintln!("unknown case"); std::process::exit(2); }
        Ok(Some(Err(x))) => println!("{head},\"outcome\":\"error\",\"err\":\"{}\"}}", esc(&x.chars().take(300).collect::<String>())),
        Ok(Some(Ok(Res::NoOp(w)))) => println!("{head},\"outcome\":\"no_op\",\"err\":\"{w}\"}}"),
        Ok(Some(Ok(Res::Done(b, sh)))) => {
            let faces = b.shells[sh].faces.len();
            let edges = b.stats().edges;
            if faces == 0 { println!("{head},\"outcome\":\"empty\",\"faces\":0}}"); return; }
            let valid = catch_unwind(AssertUnwindSafe(|| validate_shell(&b, sh, &UnionConfig::default(), &mut Diagnostics::default()).passed())).ok();
            let (v, mesh) = match catch_unwind(AssertUnwindSafe(|| mesh_brep(&b, &ImportOptions::default()))) {
                // CadCore's mesher for a B-rep goes through its own STEP writer and reader
                Ok(Ok((fm, _))) => (Some(mesh_volume(&fm).abs()), format!("ok {} faces meshed", fm.len())),
                Ok(Err(x)) => (None, format!("err {}", esc(&x.chars().take(120).collect::<String>()))),
                Err(_) => (None, "err panic".into()),
            };
            let step = match catch_unwind(AssertUnwindSafe(|| cadcore::step::brep_to_step(&b))) { Ok(Ok(s)) => format!("ok {} bytes", s.len()), Ok(Err(x)) => format!("err {}", esc(&format!("{x}").chars().take(80).collect::<String>())), Err(_) => "err panic".into() };
            println!("{head},\"outcome\":\"solid\",\"volume\":{},\"vsrc\":\"mesh\",\"ref\":null,\"faces\":{faces},\"edges\":{edges},\"valid\":{},\"mesh\":\"{mesh}\",\"step\":\"{step}\"}}",
                num(v), valid.map(|x| x.to_string()).unwrap_or("null".into()));
        }
    }
}
