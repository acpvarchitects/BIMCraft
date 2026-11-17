fn main() {
    let a = std::env::args().nth(1).unwrap_or_default();
    let why = if a.starts_with('8') { "can't build: no NURBS/free-form geometry; meshing only of lines/circles/planes (see forms table for mesh of what it can build)" }
        else if a == "9" { "can't build: fj-core has no STEP export (separate fj-export crates: STL/3MF/OBJ only)" }
        else { "can't build: fj-core 0.49 geometry is planes, lines and circles only (GlobalPath::{Line,Circle}); no NURBS curves or surfaces, no loft, no boolean, no fillet" };
    println!("| {a} | {why} | |");
}
