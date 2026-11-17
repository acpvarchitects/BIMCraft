//! Dividing a wall's layer as Revit's Divide Parts does, and as a restorer needs: a damp zone outlined on the face, a
//! plaster patch to a depth, a wall out of plumb, a lacuna.

use super::*;

/// A box from `lo` to `hi`, its faces turned out.
fn cuboid(lo: P3, hi: P3) -> Mesh {
    let v = |x: usize, y: usize, z: usize| [[lo[0], hi[0]][x], [lo[1], hi[1]][y], [lo[2], hi[2]][z]];
    [
        [v(0, 0, 1), v(1, 0, 1), v(1, 1, 1), v(0, 1, 1)],
        [v(0, 0, 0), v(0, 1, 0), v(1, 1, 0), v(1, 0, 0)],
        [v(0, 0, 0), v(1, 0, 0), v(1, 0, 1), v(0, 0, 1)],
        [v(1, 1, 0), v(0, 1, 0), v(0, 1, 1), v(1, 1, 1)],
        [v(0, 1, 0), v(0, 0, 0), v(0, 0, 1), v(0, 1, 1)],
        [v(1, 0, 0), v(1, 1, 0), v(1, 1, 1), v(1, 0, 1)],
    ]
    .iter()
    .flat_map(|[a, b, c, d]| [[*a, *b, *c], [*a, *c, *d]])
    .collect()
}

/// A wall's layer 4 m long, 3 m high and 300 mm thick: along x, its outer face at y = 0.
fn wall() -> Mesh {
    cuboid([0.0, 0.0, 0.0], [4000.0, 300.0, 3000.0])
}
const WALL: f64 = 4000.0 * 300.0 * 3000.0;
/// The way the wall's outer face looks.
const OUT: P3 = [0.0, -1.0, 0.0];

/// The wall's outer face: x along the wall, y up, looking out of it.
fn face() -> Frame {
    Frame { origin: [0.0, 0.0, 0.0], x: [1.0, 0.0, 0.0], normal: OUT }
}

fn pts(p: &[(f64, f64)]) -> Vec<P2> {
    p.iter().map(|&(x, y)| P2(x, y)).collect()
}

/// An open line on the wall's face.
fn line(p: &[(f64, f64)]) -> Cut {
    Cut::Sketch { frame: face(), points: pts(p), closed: false, depth: None }
}

/// A closed line on the wall's face, through it or to a depth.
fn patch(p: &[(f64, f64)], depth: Option<f64>) -> Cut {
    Cut::Sketch { frame: face(), points: pts(p), closed: true, depth }
}

fn square(x: f64, y: f64, side: f64) -> Vec<(f64, f64)> {
    vec![(x, y), (x + side, y), (x + side, y + side), (x, y + side)]
}

fn recipe(cuts: Vec<Cut>) -> Recipe {
    Recipe { cuts: cuts.into_iter().map(Division::from).collect(), offsets: Vec::new() }
}

fn gapped(cut: Cut, gap: f64) -> Recipe {
    Recipe { cuts: vec![Division { cut, gap, of: None }], offsets: Vec::new() }
}

fn key(steps: &[(u32, Side, u32)]) -> PieceKey {
    PieceKey(steps.iter().map(|&(cut, side, lump)| Step { cut, side, lump }).collect())
}

fn keys(o: &Outcome) -> Vec<String> {
    o.pieces.iter().map(|p| p.key.to_string()).collect()
}

fn near(got: f64, want: f64) -> bool {
    (got - want).abs() <= 1e-6 * want.abs().max(1.0)
}

/// A triangle's height on its longest edge, mm: none for a triangle with no area.
fn height(t: &[P3; 3]) -> f64 {
    let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    let longest = [sub(t[1], t[0]), sub(t[2], t[1]), sub(t[0], t[2])].iter().map(|e| dot(*e, *e).sqrt()).fold(0.0, f64::max);
    if longest > 0.0 { dot(n, n).sqrt() / longest } else { 0.0 }
}

/// What every division owes: closed pieces that hold something, made of triangles that have an area, and volumes that
/// add up to the solid's.
fn sound(o: &Outcome, whole: f64) {
    for p in &o.pieces {
        let flat = p.mesh.iter().filter(|t| height(t) < 1e-4).count();
        assert!(flat == 0, "{} has {flat} triangles with no area, of {}", p.key, p.mesh.len());
        assert!(p.closed && is_closed(&p.mesh), "{} isn't closed", p.key);
        assert!(p.volume > 0.0 && near(volume(&p.mesh), p.volume), "{} holds {}", p.key, p.volume);
    }
    let sum: f64 = o.pieces.iter().map(|p| p.volume).sum();
    assert!(near(sum + o.removed, whole), "{sum} in pieces and {} removed, of {whole}", o.removed);
}

fn held(o: &Outcome, steps: &[(u32, Side, u32)]) -> f64 {
    o.piece(&key(steps)).unwrap_or_else(|| panic!("no piece {}: {:?}", key(steps), keys(o))).volume
}

fn conflict(r: Result<Outcome, DivideError>) -> (Stage, Conflict) {
    match r {
        Err(DivideError::Conflict { at, conflict }) => (at, conflict),
        other => panic!("not a conflict: {other:?}"),
    }
}

fn input(r: Result<Outcome, DivideError>) -> String {
    match r {
        Err(DivideError::Input(what)) => what,
        other => panic!("not refused as input: {other:?}"),
    }
}

#[test]
fn a_horizontal_line_across_a_wall_layer_divides_it_in_two() {
    // Drawn past both ends, and drawn from edge to edge: the same two pieces, what is above the line on its left.
    for ends in [(-100.0, 4100.0), (0.0, 4000.0)] {
        let o = divide(&wall(), &recipe(vec![line(&[(ends.0, 1000.0), (ends.1, 1000.0)])])).unwrap();
        assert_eq!(keys(&o), ["0.left.0", "0.right.0"]);
        sound(&o, WALL);
        assert!(near(held(&o, &[(0, Side::Left, 0)]), 4000.0 * 300.0 * 2000.0));
        assert!(near(held(&o, &[(0, Side::Right, 0)]), 4000.0 * 300.0 * 1000.0));
        assert!(o.removed.abs() < 1.0 && o.conflicts.is_empty());
    }
}

#[test]
fn a_closed_line_makes_a_patch_and_leaves_the_rest_with_a_hole() {
    let o = divide(&wall(), &recipe(vec![patch(&square(1000.0, 500.0, 1000.0), None)])).unwrap();
    assert_eq!(keys(&o), ["0.inside.0", "0.outside.0"]);
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Inside, 0)]), 1000.0 * 1000.0 * 300.0));
    assert!(near(held(&o, &[(0, Side::Outside, 0)]), WALL - 1000.0 * 1000.0 * 300.0));
    // The rest's face is the wall's less the patch, for the bill of quantities; the patch's is its own square.
    let (inside, outside) = (&o.pieces[0].mesh, &o.pieces[1].mesh);
    assert!(near(area_facing(outside, OUT, FACING), 4000.0 * 3000.0 - 1.0e6));
    assert!(near(area_facing(inside, OUT, FACING), 1.0e6));
    // Drawn clockwise, it is the same patch.
    let mut back = square(1000.0, 500.0, 1000.0);
    back.reverse();
    let turned = divide(&wall(), &recipe(vec![patch(&back, None)])).unwrap();
    assert!(near(held(&turned, &[(0, Side::Inside, 0)]), 3.0e8));
}

#[test]
fn a_patch_cut_to_a_depth_is_one_piece_and_what_is_behind_stays_with_the_rest() {
    // Detached plaster, 15 mm deep.
    let o = divide(&wall(), &recipe(vec![patch(&square(1000.0, 500.0, 1000.0), Some(15.0))])).unwrap();
    assert_eq!(keys(&o), ["0.inside.0", "0.outside.0"]);
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Inside, 0)]), 1.0e6 * 15.0));
    assert!(near(held(&o, &[(0, Side::Outside, 0)]), WALL - 1.0e6 * 15.0));
    let (inside, outside) = (&o.pieces[0].mesh, &o.pieces[1].mesh);
    assert!(near(area_facing(inside, OUT, FACING), 1.0e6));
    // The rest shows the wall's face round the patch and, behind it, the floor of the pocket: the whole elevation.
    assert!(near(area_facing(outside, OUT, FACING), 4000.0 * 3000.0));
    assert!(near(area_facing(outside, [0.0, 1.0, 0.0], FACING), 4000.0 * 3000.0));
}

#[test]
fn a_tilted_plane_divides_a_wall_out_of_plumb() {
    // From 100 mm in at the foot to 250 mm in at the head: the front is a wedge 200 mm thick below and 50 above.
    let cut = Cut::Plane { point: [0.0, 100.0, 0.0], normal: [0.0, 3000.0, -150.0] };
    let o = divide(&wall(), &recipe(vec![cut])).unwrap();
    assert_eq!(keys(&o), ["0.front.0", "0.back.0"]);
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Front, 0)]), (200.0 + 50.0) / 2.0 * 3000.0 * 4000.0));
    assert!(near(held(&o, &[(0, Side::Back, 0)]), (100.0 + 250.0) / 2.0 * 3000.0 * 4000.0));
    // The plane's own faces: the wall's elevation, longer by the slope.
    let slope = (3000.0_f64.powi(2) + 150.0_f64.powi(2)).sqrt() * 4000.0;
    assert!(near(area_facing(&o.pieces[1].mesh, [0.0, 3000.0, -150.0], FACING), slope));
    // And by itself: the two closed halves.
    let (front, back) = cut_plane(&wall(), [0.0, 100.0, 0.0], [0.0, 3000.0, -150.0]).unwrap();
    assert!(is_closed(&front) && is_closed(&back) && near(volume(&front) + volume(&back), WALL));
}

#[test]
fn an_extruded_profile_takes_a_niche_or_a_lacuna() {
    // An L-shaped niche 200 mm into the face.
    let l = pts(&[(500.0, 500.0), (1500.0, 500.0), (1500.0, 1000.0), (1000.0, 1000.0), (1000.0, 1500.0), (500.0, 1500.0)]);
    let niche = Cut::Profile { frame: face(), points: l, from: Some(-200.0), to: None };
    let o = divide(&wall(), &recipe(vec![niche])).unwrap();
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Inside, 0)]), 750_000.0 * 200.0));
    // A lacuna inside the wall: a triangle about an upright axis, 400 mm tall. The rest keeps it as a hollow.
    let upright = Frame { origin: [2000.0, 150.0, 1500.0], x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] };
    let lacuna =
        Cut::Profile { frame: upright, points: pts(&[(-100.0, -50.0), (100.0, -50.0), (0.0, 50.0)]), from: Some(-200.0), to: Some(200.0) };
    let o = divide(&wall(), &recipe(vec![lacuna])).unwrap();
    assert_eq!(keys(&o), ["0.inside.0", "0.outside.0"]);
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Inside, 0)]), 10_000.0 * 400.0));
    assert!(near(held(&o, &[(0, Side::Outside, 0)]), WALL - 10_000.0 * 400.0));
    // A profile at any angle, through everything: a prism leaning 30° from the wall's normal takes its section.
    let lean = Frame { origin: [2000.0, 0.0, 1500.0], x: [1.0, 0.0, 0.0], normal: [0.0, -0.75_f64.sqrt(), 0.5] };
    let bore = Cut::Profile { frame: lean, points: pts(&square(-100.0, -100.0, 200.0)), from: None, to: None };
    let o = divide(&wall(), &recipe(vec![bore])).unwrap();
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Inside, 0)]), 200.0 * 200.0 * 300.0 / 0.75_f64.sqrt()));
}

#[test]
fn a_gap_takes_its_width_along_the_cut() {
    // A plane: a joint 10 mm wide takes 10 mm of the wall's section.
    let level = Cut::Plane { point: [0.0, 0.0, 1000.0], normal: [0.0, 0.0, 1.0] };
    let o = divide(&wall(), &gapped(level, 10.0)).unwrap();
    sound(&o, WALL);
    assert!(near(o.removed, 10.0 * 4000.0 * 300.0));
    assert!(near(held(&o, &[(0, Side::Front, 0)]), 4000.0 * 300.0 * 1995.0));
    // The piece's face, net of the joint.
    assert!(near(area_facing(&o.pieces[0].mesh, OUT, FACING), 4000.0 * 1995.0));
    // A bent line: the joint follows it, as wide all along.
    let bent = line(&[(-100.0, 1000.0), (2000.0, 1000.0), (4100.0, 2050.0)]);
    let o = divide(&wall(), &gapped(bent, 10.0)).unwrap();
    assert_eq!(keys(&o), ["0.left.0", "0.right.0"]);
    sound(&o, WALL);
    let along = 2000.0 + (2000.0_f64.powi(2) + 1000.0_f64.powi(2)).sqrt();
    assert!((o.removed / (10.0 * along * 300.0) - 1.0).abs() < 1e-4, "{} removed", o.removed);
    // A closed line: the joint runs round the patch, half inside it and half outside.
    let o = divide(&wall(), &gapped(patch(&square(1000.0, 500.0, 1000.0), None), 10.0)).unwrap();
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Inside, 0)]), 990.0 * 990.0 * 300.0));
    assert!(near(o.removed, (1010.0 * 1010.0 - 990.0 * 990.0) * 300.0));
    // To a depth, the joint runs under the patch too.
    let o = divide(&wall(), &gapped(patch(&square(1000.0, 500.0, 1000.0), Some(20.0)), 10.0)).unwrap();
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Inside, 0)]), 990.0 * 990.0 * 15.0));
    assert!(near(o.removed, 1010.0 * 1010.0 * 25.0 - 990.0 * 990.0 * 15.0));
    // A gap wider than the patch leaves nothing in it: the recipe's fault, whatever the wall.
    assert!(input(divide(&wall(), &gapped(patch(&square(1000.0, 500.0, 100.0), None), 150.0))).contains("gap"));
}

#[test]
fn a_bent_line_with_no_gap_leaves_pieces_that_meet_on_it() {
    let bent = line(&[(-100.0, 1000.0), (2000.0, 1000.0), (4100.0, 2050.0)]);
    let o = divide(&wall(), &recipe(vec![bent])).unwrap();
    assert_eq!(keys(&o), ["0.left.0", "0.right.0"]);
    sound(&o, WALL);
    // Below the line: 1 m high for 2 m, then rising to 2 m at the wall's end.
    assert!(near(held(&o, &[(0, Side::Right, 0)]), (2000.0 * 1000.0 + 2000.0 * 1500.0) * 300.0));
    assert!(o.removed.abs() < 1.0, "{} removed", o.removed);
    // Every corner of the cut lies on the line itself, not a hair off it.
    for p in o.pieces.iter().flat_map(|p| p.mesh.iter().flatten()) {
        let on_first = (p[2] - 1000.0).abs() < 1e-3 && p[0] < 2000.0 + 1e-3;
        let on_second = (p[2] - (1000.0 + (p[0] - 2000.0) / 2.0)).abs() < 1e-3 && p[0] > 2000.0 - 1e-3;
        if on_first || on_second {
            assert!((p[2] - 1000.0).abs() < 1e-7 || (p[2] - (1000.0 + (p[0] - 2000.0) / 2.0)).abs() < 1e-7, "{p:?} is off the line");
        }
    }
}

#[test]
fn a_face_moves_into_its_piece_and_never_out() {
    let level = Cut::Plane { point: [0.0, 0.0, 1000.0], normal: [0.0, 0.0, 1.0] };
    let upper = key(&[(0, Side::Front, 0)]);
    let with = |cuts: Vec<Cut>, piece: &PieceKey, face: FaceRef, distance: f64| {
        let mut r = recipe(cuts);
        r.offsets.push(Offset { piece: piece.clone(), face, distance });
        r
    };
    // The face a cut made: the upper piece's underside, 50 mm in.
    let o = divide(&wall(), &with(vec![level.clone()], &upper, FaceRef::Cut(0), 50.0)).unwrap();
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Front, 0)]), 4000.0 * 300.0 * 1950.0));
    assert!(near(held(&o, &[(0, Side::Back, 0)]), 4000.0 * 300.0 * 1000.0));
    assert!(near(o.removed, 4000.0 * 300.0 * 50.0));
    // A face by its plane: the wall's end, eroded 100 mm on the upper piece only.
    let end = FaceRef::Plane { point: [4000.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0] };
    let o = divide(&wall(), &with(vec![level.clone()], &upper, end, 100.0)).unwrap();
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Front, 0)]), 3900.0 * 300.0 * 2000.0));
    // A face of a piece that reaches past its plane: the rest round a corner patch is an L, and the face inside the L
    // moves in without the foot of the L being cut off.
    let corner = patch(&[(3000.0, 2000.0), (4100.0, 2000.0), (4100.0, 3100.0), (3000.0, 3100.0)], None);
    let rest = key(&[(0, Side::Outside, 0)]);
    let inner = FaceRef::Plane { point: [3000.0, 0.0, 2500.0], normal: [1.0, 0.0, 0.0] };
    let o = divide(&wall(), &with(vec![corner.clone()], &rest, inner, 100.0)).unwrap();
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Outside, 0)]), WALL - 1000.0 * 1000.0 * 300.0 - 100.0 * 1000.0 * 300.0));
    // The same face by the cut that made it moves both of its sides in: the L's two inner faces.
    let o = divide(&wall(), &with(vec![corner.clone()], &rest, FaceRef::Cut(0), 100.0)).unwrap();
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Outside, 0)]), WALL - 1100.0 * 1100.0 * 300.0));
    // A patch's own outline, drawn in all round.
    let whole = patch(&square(1000.0, 500.0, 1000.0), None);
    let o = divide(&wall(), &with(vec![whole.clone()], &key(&[(0, Side::Inside, 0)]), FaceRef::Cut(0), 10.0)).unwrap();
    sound(&o, WALL);
    assert!(near(held(&o, &[(0, Side::Inside, 0)]), 980.0 * 980.0 * 300.0));
    // A bent line's face, on its left piece.
    let bent = line(&[(-100.0, 1000.0), (2000.0, 1000.0), (4100.0, 2050.0)]);
    let o = divide(&wall(), &with(vec![bent], &key(&[(0, Side::Left, 0)]), FaceRef::Cut(0), 20.0)).unwrap();
    sound(&o, WALL);
    let along = 2000.0 + (2000.0_f64.powi(2) + 1000.0_f64.powi(2)).sqrt();
    assert!((o.removed / (20.0 * along * 300.0) - 1.0).abs() < 1e-3, "{} removed", o.removed);

    // Outwards there is nothing to add: refused, saying so.
    assert!(input(divide(&wall(), &with(vec![level.clone()], &upper, FaceRef::Cut(0), -5.0))).contains("never out"));
    // A cut the piece wasn't made by, and a cut the recipe hasn't got.
    assert!(input(divide(&wall(), &with(vec![level.clone()], &upper, FaceRef::Cut(3), 5.0))).contains("cut 3"));
    assert!(input(divide(&wall(), &with(vec![level.clone(), whole], &upper, FaceRef::Cut(1), 5.0))).contains("wasn't made by cut 1"));
    // What the host no longer has: the piece, the face, or enough of the piece.
    let gone = key(&[(0, Side::Front, 1)]);
    assert_eq!(conflict(divide(&wall(), &with(vec![level.clone()], &gone, end, 5.0))), (Stage::Offset(0), Conflict::NoSuchPiece));
    let nowhere = FaceRef::Plane { point: [4500.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0] };
    assert_eq!(conflict(divide(&wall(), &with(vec![level.clone()], &upper, nowhere, 5.0))), (Stage::Offset(0), Conflict::NoSuchFace));
    assert_eq!(
        conflict(divide(&wall(), &with(vec![level.clone()], &upper, FaceRef::Cut(0), 2500.0))),
        (Stage::Offset(0), Conflict::NothingLeft)
    );
    // A cut's face that a later cut took off the piece: above a second plane, nothing is left of the first one's.
    let higher = Cut::Plane { point: [0.0, 0.0, 2000.0], normal: [0.0, 0.0, 1.0] };
    let top = key(&[(0, Side::Front, 0), (1, Side::Front, 0)]);
    assert_eq!(
        conflict(divide(&wall(), &with(vec![level.clone(), higher.clone()], &top, FaceRef::Cut(0), 5.0))),
        (Stage::Offset(0), Conflict::NoSuchFace)
    );
    let o = divide(&wall(), &with(vec![level.clone(), higher], &top, FaceRef::Cut(1), 5.0)).unwrap();
    assert!(near(held(&o, &[(0, Side::Front, 0), (1, Side::Front, 0)]), 4000.0 * 300.0 * 995.0));
    // Regenerating leaves that offset out, says so, and keeps the pieces.
    let o = regenerate(&wall(), &with(vec![level], &upper, nowhere, 5.0)).unwrap();
    assert_eq!(o.conflicts, [(Stage::Offset(0), Conflict::NoSuchFace)]);
    sound(&o, WALL);
    assert!(o.removed.abs() < 1.0);
}

#[test]
fn cuts_in_sequence_give_pieces_named_by_the_cuts_that_made_them() {
    let across = line(&[(-100.0, 1000.0), (4100.0, 1000.0)]);
    let up = line(&[(1500.0, -100.0), (1500.0, 3100.0)]);
    let o = divide(&wall(), &recipe(vec![across.clone(), up.clone()])).unwrap();
    assert_eq!(keys(&o), ["0.left.0/1.left.0", "0.left.0/1.right.0", "0.right.0/1.left.0", "0.right.0/1.right.0"]);
    sound(&o, WALL);
    // Going up, the line's left is the wall's start.
    assert!(near(held(&o, &[(0, Side::Left, 0), (1, Side::Left, 0)]), 1500.0 * 2000.0 * 300.0));
    assert!(near(held(&o, &[(0, Side::Right, 0), (1, Side::Right, 0)]), 2500.0 * 1000.0 * 300.0));
    // A cut of one piece only: what is above the first line, as Revit divides the part selected.
    let mut one = recipe(vec![across.clone(), up.clone()]);
    one.cuts[1].of = Some(key(&[(0, Side::Left, 0)]));
    let o = divide(&wall(), &one).unwrap();
    assert_eq!(keys(&o), ["0.left.0/1.left.0", "0.left.0/1.right.0", "0.right.0"]);
    sound(&o, WALL);
    // A third cut of a piece of a piece; a cut that passes a piece by leaves it and its name alone.
    let damp = patch(&square(200.0, 200.0, 500.0), Some(20.0));
    let o = divide(&wall(), &recipe(vec![across.clone(), up.clone(), damp.clone()])).unwrap();
    assert_eq!(
        keys(&o),
        [
            "0.left.0/1.left.0",
            "0.left.0/1.right.0",
            "0.right.0/1.left.0/2.inside.0",
            "0.right.0/1.left.0/2.outside.0",
            "0.right.0/1.right.0"
        ]
    );
    sound(&o, WALL);
    // A cut of a piece that isn't there.
    let mut lost = recipe(vec![across, up]);
    lost.cuts[1].of = Some(key(&[(0, Side::Front, 0)]));
    assert_eq!(conflict(divide(&wall(), &lost)), (Stage::Cut(1), Conflict::NoSuchPiece));
}

#[test]
fn the_same_recipe_on_a_host_moved_or_stretched_gives_the_same_keys() {
    // Lines drawn well past the wall, a patch near its start: as kept in the wall's own coordinates.
    let cuts = vec![
        line(&[(-1000.0, 1000.0), (9000.0, 1000.0)]),
        line(&[(1500.0, -1000.0), (1500.0, 9000.0)]),
        patch(&square(200.0, 200.0, 500.0), Some(20.0)),
    ];
    let before = divide(&wall(), &recipe(cuts.clone())).unwrap();
    // The wall grown to 5.5 m by 3.4 m and 350 mm thick.
    let grown = cuboid([0.0, 0.0, 0.0], [5500.0, 350.0, 3400.0]);
    let after = divide(&grown, &recipe(cuts.clone())).unwrap();
    assert_eq!(keys(&before), keys(&after));
    sound(&after, 5500.0 * 350.0 * 3400.0);
    assert!(near(held(&after, &[(0, Side::Left, 0), (1, Side::Right, 0)]), 4000.0 * 2400.0 * 350.0));
    // The wall moved and turned with its recipe: the frame turns with it, and the pieces are the same.
    let (s, c) = 0.6_f64.sin_cos();
    let moved = |p: P3| [c * p[0] - s * p[1] + 7000.0, s * p[0] + c * p[1] - 2000.0, p[2] + 400.0];
    let turned = |v: P3| [c * v[0] - s * v[1], s * v[0] + c * v[1], v[2]];
    let host: Mesh = wall().iter().map(|t| t.map(moved)).collect();
    let there = Frame { origin: moved([0.0; 3]), x: turned([1.0, 0.0, 0.0]), normal: turned(OUT) };
    let carried: Vec<Cut> = cuts
        .iter()
        .map(|cut| match cut {
            Cut::Sketch { points, closed, depth, .. } => {
                Cut::Sketch { frame: there, points: points.clone(), closed: *closed, depth: *depth }
            }
            other => other.clone(),
        })
        .collect();
    let o = divide(&host, &recipe(carried)).unwrap();
    assert_eq!(keys(&before), keys(&o));
    sound(&o, WALL);
    for (a, b) in before.pieces.iter().zip(&o.pieces) {
        assert!(near(a.volume, b.volume), "{}: {} and {}", a.key, a.volume, b.volume);
    }
}

#[test]
fn a_cut_that_no_longer_meets_its_host_is_a_conflict_of_its_kind() {
    let across = line(&[(-100.0, 1000.0), (4100.0, 1000.0)]);
    // The wall grew past the line's end: the line doesn't cross it any more.
    let longer = cuboid([0.0, 0.0, 0.0], [5000.0, 300.0, 3000.0]);
    assert_eq!(conflict(divide(&longer, &recipe(vec![across.clone()]))), (Stage::Cut(0), Conflict::DoesNotCross));
    // A line that stops short inside the wall, straight or bent.
    assert_eq!(conflict(divide(&wall(), &recipe(vec![line(&[(-100.0, 1000.0), (2000.0, 1000.0)])]))).1, Conflict::DoesNotCross);
    assert_eq!(
        conflict(divide(&wall(), &recipe(vec![line(&[(-100.0, 1000.0), (2000.0, 1000.0), (3000.0, 1500.0)])]))).1,
        Conflict::DoesNotCross
    );
    // The wall got lower than the line, and than the plane.
    let lower = cuboid([0.0, 0.0, 0.0], [4000.0, 300.0, 900.0]);
    assert_eq!(conflict(divide(&lower, &recipe(vec![across.clone()]))).1, Conflict::Misses);
    let level = Cut::Plane { point: [0.0, 0.0, 1000.0], normal: [0.0, 0.0, 1.0] };
    assert_eq!(conflict(divide(&lower, &recipe(vec![level]))).1, Conflict::Misses);
    // A plaster patch 15 mm deep on a layer that is now 10 mm thick; a patch the wall no longer reaches; one that holds it all.
    let thin = cuboid([0.0, 0.0, 0.0], [4000.0, 10.0, 3000.0]);
    let plaster = patch(&square(1000.0, 500.0, 1000.0), Some(15.0));
    assert_eq!(conflict(divide(&thin, &recipe(vec![plaster.clone()]))).1, Conflict::TooDeep);
    assert_eq!(conflict(divide(&cuboid([0.0; 3], [900.0, 300.0, 400.0]), &recipe(vec![plaster.clone()]))).1, Conflict::Misses);
    assert_eq!(conflict(divide(&wall(), &recipe(vec![patch(&square(-500.0, -500.0, 6000.0), None)]))).1, Conflict::HoldsAll);
    // Regenerating goes on without the cut that fails, says which, and the other cuts' pieces keep their names.
    let up = line(&[(1500.0, -100.0), (1500.0, 3100.0)]);
    let o = regenerate(&longer, &recipe(vec![across, up, plaster])).unwrap();
    assert_eq!(o.conflicts, [(Stage::Cut(0), Conflict::DoesNotCross)]);
    assert_eq!(keys(&o), ["1.left.0/2.inside.0", "1.left.0/2.outside.0", "1.right.0/2.inside.0", "1.right.0/2.outside.0"]);
    sound(&o, 5000.0 * 300.0 * 3000.0);
    // What a conflict says is what to do about it.
    let said = DivideError::Conflict { at: Stage::Cut(0), conflict: Conflict::DoesNotCross }.to_string();
    assert!(said.starts_with("cut 0: the line doesn't cross the part"), "{said}");
}

/// A wall's layer with a door: 4 m by 3 m, the door from 1 m to 2 m along it and 2.1 m high.
fn doorway() -> Prism {
    let outline = pts(&[
        (0.0, 0.0),
        (1000.0, 0.0),
        (1000.0, 2100.0),
        (2000.0, 2100.0),
        (2000.0, 0.0),
        (4000.0, 0.0),
        (4000.0, 3000.0),
        (0.0, 3000.0),
    ]);
    Prism { frame: face(), outline, holes: Vec::new(), thickness: 300.0 }
}
const DOORWAY: f64 = (4000.0 * 3000.0 - 1000.0 * 2100.0) * 300.0;

#[test]
fn a_line_cuts_where_it_is_drawn_and_nowhere_else() {
    let solid = doorway().mesh().unwrap();
    assert!(is_closed(&solid) && near(volume(&solid), DOORWAY));
    // Across one jamb of a door, ending in the opening: the foot of that jamb comes off, the other jamb is whole.
    let jamb = line(&[(-50.0, 1000.0), (1050.0, 1000.0)]);
    let o = divide(&solid, &recipe(vec![jamb.clone()])).unwrap();
    assert_eq!(keys(&o), ["0.left.0", "0.right.0"]);
    sound(&o, DOORWAY);
    assert!(near(held(&o, &[(0, Side::Right, 0)]), 1000.0 * 1000.0 * 300.0));
    // Across the door and both jambs: two feet on the right, counted along the wall.
    let both = line(&[(-50.0, 1000.0), (4050.0, 1000.0)]);
    let o = divide(&solid, &recipe(vec![both.clone()])).unwrap();
    assert_eq!(keys(&o), ["0.left.0", "0.right.0", "0.right.1"]);
    sound(&o, DOORWAY);
    assert!(near(held(&o, &[(0, Side::Right, 0)]), 1000.0 * 1000.0 * 300.0));
    assert!(near(held(&o, &[(0, Side::Right, 1)]), 2000.0 * 1000.0 * 300.0));
    // A line ending in a window, which the wall closes round, parts nothing.
    let mut window = doorway();
    window.outline = pts(&square(0.0, 0.0, 4000.0));
    window.holes = vec![pts(&square(1500.0, 1500.0, 1000.0))];
    let into = line(&[(-50.0, 2000.0), (1600.0, 2000.0)]);
    assert_eq!(conflict(divide(&window.mesh().unwrap(), &recipe(vec![into.clone()]))).1, Conflict::DoesNotCross);
    assert_eq!(conflict(divide_prism(&window, &recipe(vec![into]))).1, Conflict::DoesNotCross);
    // Drawn through the window from edge to edge, it does.
    let through = line(&[(-50.0, 2000.0), (4050.0, 2000.0)]);
    let o = divide(&window.mesh().unwrap(), &recipe(vec![through])).unwrap();
    assert_eq!(keys(&o), ["0.left.0", "0.right.0"]);
    sound(&o, (4000.0 * 4000.0 - 1.0e6) * 300.0);
}

#[test]
fn a_prism_divided_in_its_plane_and_as_a_solid_gives_the_same_pieces() {
    let mut layer = doorway();
    layer.holes = vec![pts(&square(2600.0, 1000.0, 800.0))];
    let whole = DOORWAY - 800.0 * 800.0 * 300.0;
    let solid = layer.mesh().unwrap();
    assert!(is_closed(&solid) && near(volume(&solid), whole));
    let cuts = vec![
        // Across both jambs and the window; up the wall beside the door, bending; a patch over the door's head.
        line(&[(0.0, 1400.0), (4000.0, 1400.0)]),
        line(&[(600.0, -50.0), (600.0, 2400.0), (900.0, 3050.0)]),
        patch(&[(1200.0, 2300.0), (1800.0, 2300.0), (1800.0, 2800.0), (1500.0, 2950.0), (1200.0, 2800.0)], None),
    ];
    let mut r = recipe(cuts);
    r.cuts[1].of = Some(key(&[(0, Side::Left, 0)]));
    let flat = divide_prism(&layer, &r).unwrap();
    let full = divide(&solid, &r).unwrap();
    sound(&flat, whole);
    sound(&full, whole);
    assert_eq!(keys(&flat), keys(&full));
    // Below the first line the window only notches what is right of the door: two feet, not three.
    assert_eq!(
        keys(&flat),
        ["0.left.0/1.left.0", "0.left.0/1.right.0/2.inside.0", "0.left.0/1.right.0/2.outside.0", "0.right.0", "0.right.1"]
    );
    for (a, b) in flat.pieces.iter().zip(&full.pieces) {
        assert!(near(a.volume, b.volume), "{}: {} in the plane, {} as a solid", a.key, a.volume, b.volume);
        assert!(near(area_facing(&a.mesh, OUT, FACING), area_facing(&b.mesh, OUT, FACING)), "{}'s face", a.key);
    }
    // A line drawn on the back of the layer, seen from behind: its left is the other way round, in both.
    let behind = Frame { origin: [0.0, 300.0, 0.0], x: [1.0, 0.0, 0.0], normal: [0.0, 1.0, 0.0] };
    let back = recipe(vec![Cut::Sketch { frame: behind, points: pts(&[(-50.0, -2500.0), (4050.0, -2500.0)]), closed: false, depth: None }]);
    let (flat, full) = (divide_prism(&layer, &back).unwrap(), divide(&solid, &back).unwrap());
    assert_eq!(keys(&flat), keys(&full));
    assert!(
        near(held(&flat, &[(0, Side::Right, 0)]), 4000.0 * 500.0 * 300.0)
            && near(held(&full, &[(0, Side::Right, 0)]), 4000.0 * 500.0 * 300.0)
    );
    // What the plane can't do goes the solid's way: a gap, a depth.
    let o = divide_prism(&layer, &gapped(line(&[(0.0, 2600.0), (4000.0, 2600.0)]), 10.0)).unwrap();
    assert!(near(o.removed, 10.0 * 4000.0 * 300.0));
    // A prism that is none.
    let mut odd = doorway();
    odd.thickness = f64::NAN;
    assert!(matches!(divide_prism(&odd, &r), Err(DivideError::Input(_))));
    odd = doorway();
    odd.holes = vec![pts(&square(3500.0, 2000.0, 800.0))];
    assert!(matches!(odd.mesh(), Err(DivideError::Input(_))));
}

#[test]
fn lumps_on_one_side_are_counted_in_order_across_the_cut() {
    // A comb: a rail along the bottom and three teeth, cut by a plane across the teeth.
    let mut comb = Prism { frame: face(), outline: Vec::new(), holes: Vec::new(), thickness: 200.0 };
    comb.outline = pts(&[
        (0.0, 0.0),
        (5000.0, 0.0),
        (5000.0, 2000.0),
        (4000.0, 2000.0),
        (4000.0, 500.0),
        (3000.0, 500.0),
        (3000.0, 2500.0),
        (2000.0, 2500.0),
        (2000.0, 500.0),
        (1000.0, 500.0),
        (1000.0, 1500.0),
        (0.0, 1500.0),
    ]);
    let solid = comb.mesh().unwrap();
    let across = Cut::Plane { point: [0.0, 0.0, 1000.0], normal: [0.0, 0.0, 1.0] };
    let o = divide(&solid, &recipe(vec![across])).unwrap();
    assert_eq!(keys(&o), ["0.front.0", "0.front.1", "0.front.2", "0.back.0"]);
    sound(&o, volume(&solid));
    // A level plane's x runs east: the teeth are counted along the wall, whatever their size.
    let tops: Vec<f64> = o.pieces.iter().take(3).map(|p| p.volume / (1000.0 * 200.0)).collect();
    assert!(near(tops[0], 500.0) && near(tops[1], 1500.0) && near(tops[2], 1000.0), "{tops:?}");
    // The same teeth cut by a line drawn across them on the face: counted the same way, on the line's left.
    let o = divide(&solid, &recipe(vec![line(&[(-100.0, 1000.0), (5100.0, 1000.0)])])).unwrap();
    assert_eq!(keys(&o), ["0.left.0", "0.left.1", "0.left.2", "0.right.0"]);
    assert_eq!(keys(&divide_prism(&comb, &recipe(vec![line(&[(-100.0, 1000.0), (5100.0, 1000.0)])])).unwrap()), keys(&o));
}

#[test]
fn hostile_input_is_refused_and_never_panics_or_hangs() {
    let across = || recipe(vec![line(&[(-100.0, 1000.0), (4100.0, 1000.0)])]);
    // The solid: no triangles, numbers that aren't, a surface left open, one inside out, too many triangles.
    assert!(input(divide(&[], &across())).contains("no triangles"));
    for bad in [f64::NAN, f64::INFINITY, -f64::INFINITY, 1e300] {
        let mut odd = wall();
        odd[3][1][2] = bad;
        assert!(input(divide(&odd, &across())).contains("isn't finite"));
    }
    let mut open = wall();
    open.pop();
    assert!(input(divide(&open, &across())).contains("isn't a closed surface"));
    let inside_out: Mesh = wall().iter().map(|[a, b, c]| [*a, *c, *b]).collect();
    assert!(input(divide(&inside_out, &across())).contains("inside out"));
    let many = vec![wall()[0]; crate::csg::MAX_TRIANGLES + 1];
    assert!(input(divide(&many, &across())).contains("20000"));
    // Triangles with no area among a solid's own are passed over.
    let mut odd = wall();
    odd.push([[0.0; 3]; 3]);
    odd.push([[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 0.0, 0.0]]);
    sound(&divide(&odd, &across()).unwrap(), WALL);
    // A solid of nothing but them is none.
    assert!(matches!(divide(&[[[1.0; 3]; 3]; 12], &across()), Err(DivideError::Input(_))));

    // Lines: a million points, numbers that aren't, one point, a line that crosses itself, one that folds back.
    let long: Vec<(f64, f64)> = (0..1_000_000).map(|k| (k as f64, (k % 7) as f64)).collect();
    assert!(input(divide(&wall(), &recipe(vec![line(&long)]))).contains("2000"));
    assert!(input(divide(&wall(), &recipe(vec![patch(&long, None)]))).contains("2000"));
    for bad in [f64::NAN, f64::INFINITY, 1e30] {
        assert!(input(divide(&wall(), &recipe(vec![line(&[(-100.0, bad), (4100.0, 1000.0)])]))).contains("cut 0"));
        assert!(matches!(divide(&wall(), &gapped(line(&[(-100.0, 1000.0), (4100.0, 1000.0)]), bad)), Err(DivideError::Input(_))));
        assert!(matches!(divide(&wall(), &recipe(vec![patch(&square(0.0, 0.0, 500.0), Some(bad))])), Err(DivideError::Input(_))));
        let plane = Cut::Plane { point: [0.0, bad, 0.0], normal: [0.0, 0.0, 1.0] };
        assert!(matches!(divide(&wall(), &recipe(vec![plane])), Err(DivideError::Input(_))));
        let mut frame = face();
        frame.origin[0] = bad;
        let cut = Cut::Profile { frame, points: pts(&square(0.0, 0.0, 500.0)), from: None, to: Some(bad) };
        assert!(matches!(divide(&wall(), &recipe(vec![cut])), Err(DivideError::Input(_))));
    }
    assert!(input(divide(&wall(), &recipe(vec![line(&[(5.0, 5.0)])]))).contains("two points"));
    assert!(input(divide(&wall(), &recipe(vec![line(&[(5.0, 5.0), (5.0, 5.0), (5.0, 5.001)])]))).contains("two points"));
    let knot = line(&[(-100.0, 1000.0), (3000.0, 1000.0), (3000.0, 2000.0), (2000.0, 2000.0), (2000.0, -100.0)]);
    assert!(input(divide(&wall(), &recipe(vec![knot]))).contains("crosses"));
    assert!(input(divide(&wall(), &recipe(vec![line(&[(-100.0, 1000.0), (3000.0, 1000.0), (-100.0, 1000.5)])]))).contains("folds back"));
    // Outlines with no area, a depth of nothing or less, an open line to a depth, a gap below zero.
    assert!(input(divide(&wall(), &recipe(vec![patch(&[(0.0, 0.0), (500.0, 0.0), (1000.0, 0.0)], None)]))).contains("no area"));
    assert!(input(divide(&wall(), &recipe(vec![patch(&[(0.0, 0.0), (500.0, 0.0)], None)]))).contains("no area"));
    let flat = Cut::Profile { frame: face(), points: pts(&[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)]), from: None, to: None };
    assert!(input(divide(&wall(), &recipe(vec![flat]))).contains("no area"));
    for depth in [0.0, -15.0] {
        assert!(input(divide(&wall(), &recipe(vec![patch(&square(0.0, 0.0, 500.0), Some(depth))]))).contains("depth"));
    }
    let scratch = Cut::Sketch { frame: face(), points: pts(&[(-100.0, 1000.0), (4100.0, 1000.0)]), closed: false, depth: Some(15.0) };
    assert!(input(divide(&wall(), &recipe(vec![scratch]))).contains("parts nothing"));
    assert!(input(divide(&wall(), &gapped(line(&[(-100.0, 1000.0), (4100.0, 1000.0)]), -1.0))).contains("gap"));
    // Frames and planes that are none; a profile whose range is upside down.
    let no_normal = Cut::Plane { point: [0.0; 3], normal: [0.0; 3] };
    assert!(input(divide(&wall(), &recipe(vec![no_normal]))).contains("normal"));
    let along = Frame { origin: [0.0; 3], x: [0.0, -2.0, 0.0], normal: OUT };
    let cut = Cut::Sketch { frame: along, points: pts(&[(-100.0, 1000.0), (4100.0, 1000.0)]), closed: false, depth: None };
    assert!(input(divide(&wall(), &recipe(vec![cut]))).contains("frame"));
    let upside = Cut::Profile { frame: face(), points: pts(&square(0.0, 0.0, 500.0)), from: Some(10.0), to: Some(-10.0) };
    assert!(input(divide(&wall(), &recipe(vec![upside]))).contains("`to`"));
    // Too many cuts, and too many pieces.
    let cuts: Vec<Cut> =
        (0..MAX_CUTS + 1).map(|k| Cut::Plane { point: [k as f64 * 10.0 + 5.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0] }).collect();
    assert!(input(divide(&wall(), &recipe(cuts))).contains("64"));
    let slices: Vec<Cut> =
        (0..9).map(|k| Cut::Plane { point: [0.0; 3], normal: [(k as f64 * 0.3).cos(), 0.0, (k as f64 * 0.3).sin()] }).collect();
    let ball = cuboid([-1000.0; 3], [1000.0; 3]);
    let crowd: Vec<Cut> = slices
        .into_iter()
        .chain((0..40).map(|k| Cut::Plane { point: [0.0, k as f64 * 40.0 - 800.0, 0.0], normal: [0.0, 1.0, 0.0] }))
        .collect();
    assert!(matches!(divide(&ball, &recipe(crowd)), Err(DivideError::TooMuch(_))));
}

#[test]
fn the_same_input_gives_the_same_pieces_triangle_for_triangle() {
    let mut layer = doorway();
    layer.holes = vec![pts(&square(2600.0, 1000.0, 800.0))];
    let solid = layer.mesh().unwrap();
    let mut r = recipe(vec![
        line(&[(0.0, 1400.0), (4000.0, 1400.0)]),
        line(&[(600.0, -50.0), (600.0, 2400.0), (900.0, 3050.0)]),
        patch(&square(3000.0, 2000.0, 600.0), Some(25.0)),
        Cut::Plane { point: [0.0, 120.0, 0.0], normal: [0.0, 3000.0, -100.0] },
    ]);
    r.cuts[2].gap = 4.0;
    r.offsets.push(Offset {
        piece: key(&[(0, Side::Right, 0), (1, Side::Left, 0), (3, Side::Back, 0)]),
        face: FaceRef::Cut(0),
        distance: 30.0,
    });
    let (a, b) = (divide(&solid, &r).unwrap(), divide(&solid, &r).unwrap());
    assert_eq!(a, b);
    assert_eq!(a, regenerate(&solid, &r).unwrap());
    assert_eq!(divide_prism(&layer, &r).unwrap(), a);
    sound(&a, volume(&solid));
    assert!(a.pieces.len() >= 8, "{:?}", keys(&a));
}

#[test]
fn a_recipe_is_kept_as_it_is_and_read_strictly() {
    let mut r = recipe(vec![
        line(&[(0.0, 1400.0), (4000.0, 1400.0)]),
        patch(&square(3000.0, 2000.0, 600.0), Some(25.0)),
        Cut::Plane { point: [0.0, 120.0, 0.0], normal: [0.0, 3000.0, -100.0] },
        Cut::Profile { frame: face(), points: pts(&square(0.0, 0.0, 300.0)), from: Some(-50.0), to: None },
    ]);
    r.cuts[1].gap = 4.0;
    r.cuts[2].of = Some(key(&[(0, Side::Left, 0)]));
    r.offsets.push(Offset { piece: key(&[(0, Side::Right, 0)]), face: FaceRef::Cut(0), distance: 30.0 });
    r.offsets.push(Offset { piece: key(&[(0, Side::Right, 0)]), face: FaceRef::Plane { point: [0.0; 3], normal: OUT }, distance: 3.0 });
    let text = serde_json::to_string(&r).unwrap();
    assert_eq!(serde_json::from_str::<Recipe>(&text).unwrap(), r);
    assert!(text.contains(r#"{"cut":0,"side":"left","lump":0}"#) && text.contains(r#""sketch""#) && text.contains(r#""plane""#), "{text}");
    // A key by itself, as a part would hold it, and a prism.
    let k = key(&[(0, Side::Left, 0), (2, Side::Back, 1)]);
    assert_eq!(serde_json::from_str::<PieceKey>(&serde_json::to_string(&k).unwrap()).unwrap(), k);
    assert_eq!(k.to_string(), "0.left.0/2.back.1");
    assert_eq!(PieceKey::default().to_string(), "whole");
    let prism = doorway();
    assert_eq!(serde_json::from_str::<Prism>(&serde_json::to_string(&prism).unwrap()).unwrap(), prism);
    // A misspelt key is refused, as the model's types refuse one.
    for odd in [
        r#"{"cuts":[],"offset":[]}"#,
        r#"{"cuts":[{"cut":{"plane":{"point":[0,0,0],"normal":[0,0,1],"tilt":3}}}]}"#,
        r#"{"cuts":[{"cut":{"plane":{"point":[0,0,0],"normal":[0,0,1]}},"gaps":2}]}"#,
        r#"{"cuts":[{"cut":{"plane":{"point":[0,0,0],"normal":[0,0,1]}},"of":[{"cut":0,"side":"up","lump":0}]}]}"#,
    ] {
        assert!(serde_json::from_str::<Recipe>(odd).is_err(), "{odd}");
    }
    // A work plane as a frame: its x level across it.
    let plane = crate::workplane::Frame { origin: [0.0; 3], normal: OUT };
    let frame = Frame::from(plane);
    let p = frame.point(P2(1200.0, 800.0), -40.0).unwrap();
    assert!((p[0].abs() - 1200.0).abs() < 1e-9 && (p[1] - 40.0).abs() < 1e-9 && (p[2] - 800.0).abs() < 1e-9, "{p:?}");
    let (q, along) = frame.local(p).unwrap();
    assert!((q.0 - 1200.0).abs() < 1e-9 && (q.1 - 800.0).abs() < 1e-9 && (along + 40.0).abs() < 1e-9);
}

#[test]
fn a_surface_is_measured_and_checked_for_the_bill_of_quantities() {
    let w = wall();
    assert!(near(volume(&w), WALL) && near(area(&w), 2.0 * (4000.0 * 3000.0 + 4000.0 * 300.0 + 3000.0 * 300.0)));
    assert!(near(area_facing(&w, OUT, FACING), 4000.0 * 3000.0) && near(area_facing(&w, [0.0, 0.0, 5.0], FACING), 4000.0 * 300.0));
    // Within the tolerance a face still faces; past it, it doesn't. No direction, no face.
    let tilt = |deg: f64| [0.0, -deg.to_radians().cos(), deg.to_radians().sin()];
    assert!(near(area_facing(&w, tilt(0.9), FACING), 4000.0 * 3000.0) && area_facing(&w, tilt(1.1), FACING) == 0.0);
    assert!(near(area_facing(&w, tilt(30.0), 31.0), 4000.0 * 3000.0));
    assert!(area_facing(&w, [0.0; 3], FACING) == 0.0 && area_facing(&w, OUT, f64::NAN) == 0.0);
    // Closed: every edge twice. Less a triangle, or with one turned over, it isn't.
    assert!(is_closed(&w) && !is_closed(&w[1..]) && !is_closed(&[]));
    let mut flipped = w.clone();
    flipped[0].swap(1, 2);
    assert!(!is_closed(&flipped));
    // Corners that aren't numbers make no closed surface, and nothing worse.
    for bad in [f64::NAN, f64::INFINITY, -f64::INFINITY, 1e300] {
        let mut odd = w.clone();
        odd[5][2] = [bad; 3];
        assert!(!is_closed(&odd));
        let _ = (volume(&odd), area(&odd), area_facing(&odd, OUT, FACING), area_facing(&w, [bad; 3], bad));
        assert!(matches!(cut_plane(&odd, [0.0; 3], [0.0, 0.0, 1.0]), Err(DivideError::Input(_))));
    }
    // Inside out, it holds less than nothing.
    let inside_out: Mesh = w.iter().map(|[a, b, c]| [*a, *c, *b]).collect();
    assert!(near(volume(&inside_out), -WALL));
}

#[test]
fn a_piece_has_no_more_triangles_than_its_faces_need() {
    // A patch through the wall: a box, two triangles a face; the rest, a box with a square hole through it: eight to
    // each face with the hole in it, two to each of the four sides and of the hole's four.
    let o = divide(&wall(), &recipe(vec![patch(&square(1000.0, 500.0, 1000.0), None)])).unwrap();
    sound(&o, WALL);
    assert_eq!((o.pieces[0].mesh.len(), o.pieces[1].mesh.len()), (12, 32));
    // A bent line: each piece a prism over five corners.
    let o = divide(&wall(), &recipe(vec![line(&[(-100.0, 1000.0), (2000.0, 1000.0), (4100.0, 2050.0)])])).unwrap();
    sound(&o, WALL);
    assert_eq!((o.pieces[0].mesh.len(), o.pieces[1].mesh.len()), (16, 16));
    // Cut after cut, the pieces stay as plain: boxes are boxes.
    let cuts = vec![
        line(&[(-100.0, 1000.0), (4100.0, 1000.0)]),
        line(&[(1500.0, -100.0), (1500.0, 3100.0)]),
        line(&[(2500.0, -100.0), (2500.0, 3100.0)]),
    ];
    let o = divide(&wall(), &recipe(cuts)).unwrap();
    sound(&o, WALL);
    assert!(
        o.pieces.len() == 6 && o.pieces.iter().all(|p| p.mesh.len() == 12),
        "{:?}",
        o.pieces.iter().map(|p| p.mesh.len()).collect::<Vec<_>>()
    );
}

/// A curved wall as it comes: a quarter ring in plan, 3 m to 3.3 m from its centre, in 24 flat facets, 3 m high.
fn curved() -> (Mesh, f64) {
    let at = |r: f64, k: usize| {
        let a = k as f64 / 24.0 * std::f64::consts::FRAC_PI_2;
        P2(r * a.cos(), r * a.sin())
    };
    let outline: Vec<P2> = (0..=24).map(|k| at(3300.0, k)).chain((0..=24).rev().map(|k| at(3000.0, k))).collect();
    let top = Frame { origin: [0.0, 0.0, 3000.0], x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] };
    let held = crate::geom::area(&outline) * 3000.0;
    (Prism { frame: top, outline, holes: Vec::new(), thickness: 3000.0 }.mesh().unwrap(), held)
}

#[test]
fn a_faceted_curved_wall_is_cut_as_any_solid() {
    let (solid, held) = curved();
    assert!(is_closed(&solid) && near(volume(&solid), held));
    // A frame on its outer face, half way round: x along the wall, y up, looking out.
    let h = 0.5_f64.sqrt();
    let on_face = Frame { origin: [3300.0 * h, 3300.0 * h, 0.0], x: [-h, h, 0.0], normal: [h, h, 0.0] };
    let sketch = |p: &[(f64, f64)], closed: bool, depth: Option<f64>| Cut::Sketch { frame: on_face, points: pts(p), closed, depth };
    let cuts: Vec<(&str, Cut, usize)> = vec![
        ("level", Cut::Plane { point: [0.0, 0.0, 1000.0], normal: [0.0, 0.0, 1.0] }, 2),
        ("leaning", Cut::Plane { point: [2000.0, 2000.0, 1500.0], normal: [0.3, -0.5, 1.0] }, 2),
        ("a patch through", sketch(&square(-400.0, 1000.0, 800.0), true, None), 2),
        ("a patch 40 mm deep", sketch(&square(-400.0, 1000.0, 800.0), true, Some(40.0)), 2),
        ("a bent line", sketch(&[(-200.0, -100.0), (100.0, 1500.0), (-300.0, 3100.0)], false, None), 2),
        ("a straight line", sketch(&[(50.0, -100.0), (50.0, 3100.0)], false, None), 2),
        (
            "a niche",
            Cut::Profile { frame: on_face, points: pts(&[(-300.0, 500.0), (300.0, 500.0), (0.0, 1100.0)]), from: Some(-150.0), to: None },
            2,
        ),
    ];
    for (what, cut, pieces) in &cuts {
        for gap in [0.0, 6.0] {
            let o = divide(&solid, &gapped(cut.clone(), gap)).unwrap_or_else(|e| panic!("{what}, gap {gap}: {e}"));
            assert_eq!(o.pieces.len(), *pieces, "{what}, gap {gap}: {:?}", keys(&o));
            sound(&o, held);
            assert!(if gap == 0.0 { o.removed.abs() < 1.0 } else { o.removed > 1.0 }, "{what}, gap {gap}: {} removed", o.removed);
        }
    }
    // All of them, one after another, and a face moved in on one of the pieces.
    let mut all = recipe(cuts.iter().map(|c| c.1.clone()).collect());
    let o = divide(&solid, &all).unwrap();
    sound(&o, held);
    assert!(o.pieces.len() >= 12, "{:?}", keys(&o));
    let last = o.pieces[0].key.0.last().unwrap().cut;
    all.offsets.push(Offset { piece: o.pieces[0].key.clone(), face: FaceRef::Cut(last), distance: 25.0 });
    let less = divide(&solid, &all).unwrap();
    sound(&less, held);
    assert!(less.pieces[0].volume < o.pieces[0].volume && keys(&less) == keys(&o));
    // The outer face of a piece, for its quantities: the facets turn 3.75° each, so the tolerance is wider than a flat face's.
    let whole_face = area_facing(&solid, [h, h, 0.0], 46.0);
    assert!((whole_face / (2.0 * 3300.0 * (std::f64::consts::FRAC_PI_2 / 48.0).sin() * 24.0 * 3000.0) - 1.0).abs() < 1e-9, "{whole_face}");
}

/// Numbers from a seed, the same every run.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// From `lo` to `hi`.
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f64 / 1.0e6
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// A number as a hostile recipe gives one: now and then no number, or a huge one.
    fn wild(&mut self, lo: f64, hi: f64) -> f64 {
        match self.below(40) {
            0 => f64::NAN,
            1 => f64::INFINITY,
            2 => -1.0e300,
            3 => 0.0,
            _ => self.range(lo, hi),
        }
    }
}

#[test]
fn a_ball_cut_by_planes_at_any_angle_stays_closed() {
    // A ball of 16 by 8 facets, 1 m across.
    let at = |i: usize, j: usize| {
        let (a, b) = (i as f64 / 16.0 * std::f64::consts::TAU, j as f64 / 8.0 * std::f64::consts::PI);
        [500.0 * b.sin() * a.cos(), 500.0 * b.sin() * a.sin(), 500.0 * b.cos()]
    };
    let mut ball: Mesh = Vec::new();
    for i in 0..16 {
        for j in 0..8 {
            let (a, b, c, d) = (at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j));
            if j > 0 {
                ball.push([a, b, d]);
            }
            if j < 7 {
                ball.push([b, c, d]);
            }
        }
    }
    let held = volume(&ball);
    assert!(is_closed(&ball) && held > 4.0e8);
    let mut dice = Dice(0x2545_f491_4f6c_dd1d);
    for round in 0..40 {
        let cuts: Vec<Cut> = (0..6)
            .map(|_| Cut::Plane {
                point: [dice.range(-300.0, 300.0), dice.range(-300.0, 300.0), dice.range(-300.0, 300.0)],
                normal: [dice.range(-1.0, 1.0), dice.range(-1.0, 1.0), dice.range(0.1, 1.0)],
            })
            .collect();
        // Through corners and along edges of the ball too: planes through its centre and its poles.
        let cuts = if round % 4 == 0 {
            vec![Cut::Plane { point: [0.0; 3], normal: [1.0, 0.0, 0.0] }, Cut::Plane { point: [0.0; 3], normal: [0.0, 0.0, 1.0] }]
        } else {
            cuts
        };
        let mut r = recipe(cuts);
        r.cuts[0].gap = if round % 3 == 0 { 8.0 } else { 0.0 };
        let o = regenerate(&ball, &r).unwrap_or_else(|e| panic!("round {round}: {e}"));
        for p in &o.pieces {
            assert!(p.closed, "round {round}: {} isn't closed", p.key);
        }
        let sum: f64 = o.pieces.iter().map(|p| p.volume).sum();
        assert!(near(sum + o.removed, held) && o.removed >= -1.0, "round {round}: {sum} and {}", o.removed);
    }
}

#[test]
fn random_recipes_never_panic_and_what_they_make_adds_up() {
    let mut layer = doorway();
    layer.holes = vec![pts(&square(2600.0, 1000.0, 800.0))];
    let solids = [(wall(), WALL), (layer.mesh().unwrap(), DOORWAY - 800.0 * 800.0 * 300.0), curved()];
    let mut dice = Dice(0x9e37_79b9_7f4a_7c15);
    let (mut made, mut refused, mut open) = (0, 0, Vec::new());
    for round in 0..240 {
        let (solid, held) = &solids[round % 3];
        let hostile = round % 4 == 3;
        let num = |dice: &mut Dice, lo: f64, hi: f64| if hostile { dice.wild(lo, hi) } else { dice.range(lo, hi) };
        let frame = |dice: &mut Dice| {
            if dice.below(4) > 0 {
                face()
            } else {
                Frame {
                    origin: [2000.0, 150.0, 1500.0],
                    x: [dice.range(-1.0, 1.0), 0.3, 0.2],
                    normal: [dice.range(-1.0, 1.0), dice.range(-1.0, 1.0), 0.4],
                }
            }
        };
        let mut r = Recipe::default();
        for _ in 0..1 + dice.below(4) {
            let points: Vec<P2> =
                (0..2 + dice.below(5)).map(|_| P2(num(&mut dice, -500.0, 4500.0), num(&mut dice, -500.0, 3500.0))).collect();
            let cut = match dice.below(5) {
                0 => Cut::Plane {
                    point: [num(&mut dice, 0.0, 4000.0), num(&mut dice, 0.0, 300.0), num(&mut dice, 0.0, 3000.0)],
                    normal: [num(&mut dice, -1.0, 1.0), num(&mut dice, -1.0, 1.0), num(&mut dice, -1.0, 1.0)],
                },
                1 => Cut::Sketch { frame: frame(&mut dice), points, closed: false, depth: None },
                2 => Cut::Sketch {
                    frame: frame(&mut dice),
                    points,
                    closed: true,
                    depth: (dice.below(2) == 0).then(|| num(&mut dice, 1.0, 400.0)),
                },
                3 => Cut::Sketch {
                    frame: face(),
                    points: pts(&[(-600.0, num(&mut dice, 100.0, 2900.0)), (4600.0, num(&mut dice, 100.0, 2900.0))]),
                    closed: false,
                    depth: None,
                },
                _ => Cut::Profile {
                    frame: frame(&mut dice),
                    points,
                    from: (dice.below(2) == 0).then(|| num(&mut dice, -300.0, 0.0)),
                    to: (dice.below(3) == 0).then(|| num(&mut dice, 0.0, 300.0)),
                },
            };
            let gap = if dice.below(3) == 0 { num(&mut dice, 0.0, 30.0) } else { 0.0 };
            r.cuts.push(Division { cut, gap, of: None });
        }
        // Now and then: an offset on whatever piece comes first, by the cut that made it or by a plane.
        if let Ok(o) = regenerate(solid, &r)
            && let Some(step) = o.pieces.first().and_then(|p| p.key.0.last()).filter(|_| dice.below(2) == 0)
        {
            let face = if dice.below(2) == 0 { FaceRef::Cut(step.cut) } else { FaceRef::Plane { point: [0.0; 3], normal: OUT } };
            r.offsets.push(Offset { piece: o.pieces[0].key.clone(), face, distance: num(&mut dice, 0.0, 60.0) });
        }
        for o in [regenerate(solid, &r), divide(solid, &r)] {
            match o {
                Ok(o) => {
                    made += 1;
                    let sum: f64 = o.pieces.iter().map(|p| p.volume).sum();
                    assert!(
                        near(sum + o.removed, *held) && o.removed > -1.0,
                        "round {round}: {sum} in pieces, {} removed, of {held}\n{r:?}",
                        o.removed
                    );
                    assert!(o.pieces.iter().all(|p| p.volume >= MIN_VOLUME), "round {round}");
                    open.extend(o.pieces.iter().filter(|p| !p.closed).map(|p| format!("round {round}: {}", p.key)));
                }
                Err(e) => {
                    refused += 1;
                    assert!(!e.to_string().is_empty());
                }
            }
        }
    }
    assert!(made > 100 && refused > 20, "{made} made, {refused} refused");
    assert!(open.is_empty(), "pieces not closed: {open:?}");
}

#[test]
fn random_lines_divide_a_prism_the_same_in_its_plane_and_as_a_solid() {
    let mut layer = doorway();
    layer.holes = vec![pts(&square(2600.0, 1000.0, 800.0))];
    let solid = layer.mesh().unwrap();
    let whole = DOORWAY - 800.0 * 800.0 * 300.0;
    let mut dice = Dice(0x1234_5678_9abc_def1);
    let (mut same, mut refused) = (0, 0);
    for round in 0..150 {
        let mut r = Recipe::default();
        for _ in 0..1 + dice.below(3) {
            // Whole centimetres, so lines run through corners and along edges now and then.
            let mut at = |lo: f64, hi: f64| (dice.range(lo, hi) / 100.0).round() * 100.0;
            let cut = match round % 5 {
                0 => line(&[(-200.0, at(100.0, 2900.0)), (4200.0, at(100.0, 2900.0))]),
                1 => line(&[(at(100.0, 3900.0), -200.0), (at(100.0, 3900.0), 3200.0)]),
                2 => line(&[(-200.0, at(100.0, 2900.0)), (at(500.0, 3500.0), at(100.0, 2900.0)), (4200.0, at(100.0, 2900.0))]),
                3 => patch(
                    &[
                        (at(-200.0, 2000.0), at(-200.0, 1500.0)),
                        (at(2100.0, 4200.0), at(-200.0, 1500.0)),
                        (at(1000.0, 3000.0), at(1600.0, 3200.0)),
                    ],
                    None,
                ),
                _ => line(&[(at(-300.0, 4300.0), at(-300.0, 3300.0)), (at(-300.0, 4300.0), at(-300.0, 3300.0))]),
            };
            r.cuts.push(Division::from(cut));
        }
        match (divide_prism(&layer, &r), divide(&solid, &r)) {
            (Ok(flat), Ok(full)) => {
                same += 1;
                assert_eq!(keys(&flat), keys(&full), "round {round}: {r:?}");
                for (what, o) in [("in the plane", &flat), ("as a solid", &full)] {
                    for p in &o.pieces {
                        let none: Vec<&[P3; 3]> = p.mesh.iter().filter(|t| height(t) < 1e-4).collect();
                        assert!(none.is_empty(), "round {round}, {what}: {} has triangles with no area: {none:?}\n{r:?}", p.key);
                        assert!(p.closed, "round {round}, {what}: {} isn't closed\n{r:?}", p.key);
                    }
                }
                sound(&flat, whole);
                sound(&full, whole);
                for (a, b) in flat.pieces.iter().zip(&full.pieces) {
                    assert!(
                        near(a.volume, b.volume),
                        "round {round}, {}: {} in the plane, {} as a solid\n{r:?}",
                        a.key,
                        a.volume,
                        b.volume
                    );
                }
            }
            (Err(a), Err(b)) => {
                refused += 1;
                assert_eq!(a, b, "round {round}: {r:?}");
            }
            (a, b) => panic!("round {round}: in the plane {:?}, as a solid {:?}\n{r:?}", a.map(|o| keys(&o)), b.map(|o| keys(&o))),
        }
    }
    assert!(same > 60 && refused > 5, "{same} the same, {refused} refused");
}

#[test]
fn a_solid_of_thousands_of_triangles_is_divided_within_the_budget() {
    // A curved wall in 900 facets: 7200 triangles.
    const FACETS: usize = 900;
    let at = |r: f64, k: usize| {
        let a = k as f64 / FACETS as f64 * std::f64::consts::FRAC_PI_2;
        P2(r * a.cos(), r * a.sin())
    };
    let outline: Vec<P2> = (0..=FACETS).map(|k| at(3300.0, k)).chain((0..=FACETS).rev().map(|k| at(3000.0, k))).collect();
    let top = Frame { origin: [0.0, 0.0, 3000.0], x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] };
    let held = crate::geom::area(&outline) * 3000.0;
    let solid = Prism { frame: top, outline, holes: Vec::new(), thickness: 3000.0 }.mesh().unwrap();
    assert!(solid.len() == 8 * FACETS + 4 && is_closed(&solid));
    // Planes, and a straight line that spans it, clip it triangle by triangle: thousands are no matter.
    let h = 0.5_f64.sqrt();
    let on_face = Frame { origin: [3300.0 * h, 3300.0 * h, 0.0], x: [-h, h, 0.0], normal: [h, h, 0.0] };
    let r = recipe(vec![
        Cut::Plane { point: [0.0, 0.0, 1000.0], normal: [0.0, 0.0, 1.0] },
        Cut::Plane { point: [2000.0, 2000.0, 1500.0], normal: [0.3, -0.5, 1.0] },
        Cut::Sketch { frame: on_face, points: pts(&[(-9000.0, 2200.0), (9000.0, 2300.0)]), closed: false, depth: None },
    ]);
    let t = std::time::Instant::now();
    let o = divide(&solid, &r).unwrap();
    eprintln!("{} triangles, {} pieces by planes: {:?}", solid.len(), o.pieces.len(), t.elapsed());
    assert!(o.pieces.len() >= 5, "{:?}", keys(&o));
    for p in &o.pieces {
        assert!(p.closed && p.mesh.len() <= 8 * FACETS + 40, "{} isn't closed, or has {} triangles", p.key, p.mesh.len());
    }
    let sum: f64 = o.pieces.iter().map(|p| p.volume).sum();
    assert!(near(sum + o.removed, held));
    // A patch and a bent line go through a partition, after the planes too: the sections the planes left, each one
    // face in thousands of triangles, are no more to it than the wall's top.
    let patch = Cut::Sketch { frame: on_face, points: pts(&square(-400.0, 1200.0, 800.0)), closed: true, depth: Some(40.0) };
    let bent =
        Cut::Sketch { frame: on_face, points: pts(&[(-900.0, -100.0), (-700.0, 1500.0), (-1000.0, 3100.0)]), closed: false, depth: None };
    let mut all = r.clone();
    all.cuts.extend([patch, bent].map(Division::from));
    let t = std::time::Instant::now();
    let o = divide(&solid, &all).unwrap();
    eprintln!("{} triangles, {} pieces by planes, a patch and a bent line: {:?}", solid.len(), o.pieces.len(), t.elapsed());
    assert!(o.pieces.len() >= 9, "{:?}", keys(&o));
    for p in &o.pieces {
        assert!(p.closed && p.mesh.len() <= 8 * FACETS + 600, "{} isn't closed, or has {} triangles", p.key, p.mesh.len());
    }
    let sum: f64 = o.pieces.iter().map(|p| p.volume).sum();
    assert!(near(sum + o.removed, held));
}

#[test]
fn lumps_that_touch_at_a_corner_are_one_and_so_is_a_solid_with_a_face_inside_it() {
    // Two patches corner to corner are two pieces; the rest round them is one.
    let o =
        divide(&wall(), &recipe(vec![patch(&square(1000.0, 500.0, 500.0), None), patch(&square(1500.0, 1000.0, 500.0), None)])).unwrap();
    assert_eq!(keys(&o), ["0.inside.0", "0.outside.0/1.inside.0", "0.outside.0/1.outside.0"]);
    sound(&o, WALL);
    // A line whose corner only touches the top of the wall leaves two parts above it that meet along an edge: one
    // lump, one piece, in the plane and as a solid.
    let layer = Prism { frame: face(), outline: pts(&square(0.0, 0.0, 3000.0)), holes: Vec::new(), thickness: 300.0 };
    let peak = recipe(vec![line(&[(-100.0, 1000.0), (1500.0, 3000.0), (3100.0, 1000.0)])]);
    let (flat, full) = (divide_prism(&layer, &peak).unwrap(), divide(&layer.mesh().unwrap(), &peak).unwrap());
    assert_eq!(keys(&full), ["0.left.0", "0.right.0"]);
    assert_eq!(keys(&flat), keys(&full));
    for o in [&flat, &full] {
        sound(o, 3000.0 * 3000.0 * 300.0);
        assert!(near(held(o, &[(0, Side::Left, 0)]), 2.0 * (1500.0 * 1875.0 / 2.0) * 300.0));
    }
    // Two boxes side by side, each closed, sharing a face: a solid with a face inside it, as a part's prisms may come.
    // A plane across both leaves one lump each side, not two; so does a line.
    let twin: Mesh =
        cuboid([0.0, 0.0, 0.0], [2000.0, 300.0, 3000.0]).into_iter().chain(cuboid([2000.0, 0.0, 0.0], [4000.0, 300.0, 3000.0])).collect();
    let o = divide(&twin, &recipe(vec![Cut::Plane { point: [0.0, 0.0, 1000.0], normal: [0.0, 0.0, 1.0] }])).unwrap();
    assert_eq!(keys(&o), ["0.front.0", "0.back.0"]);
    assert!(o.pieces.iter().all(|p| p.closed) && near(o.pieces[0].volume, 4000.0 * 300.0 * 2000.0));
    let o = divide(&twin, &recipe(vec![line(&[(-100.0, 1000.0), (1500.0, 2000.0), (4100.0, 1000.0)])])).unwrap();
    assert_eq!(keys(&o), ["0.left.0", "0.right.0"]);
    assert!(o.pieces.iter().all(|p| p.closed) && near(o.pieces.iter().map(|p| p.volume).sum::<f64>(), WALL));
}
