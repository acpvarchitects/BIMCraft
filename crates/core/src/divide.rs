//! A part's solid divided into pieces, as Revit's Divide Parts and Archicad's Morphs do it, for restoration work: a zone
//! of rising damp or detached plaster outlined on a wall's face, a stratigraphic unit of masonry, a wall out of plumb or
//! tapering, a lacuna. Pure geometry: no model types, no commands. The write-up is `docs/areas/parts-geometry.md`.
//!
//! **The solid** is a closed surface of triangles in model coordinates (mm; x east, y north, z up), each triangle's
//! corners counter-clockwise seen from outside, as [`crate::csg::subtract`] takes and gives them. The pieces are the same.
//!
//! **The recipe** ([`Recipe`]) is all a project keeps: the cuts, in order, and the faces moved in afterwards. The pieces
//! are worked out from it again whenever the host changes, so the same recipe on the same solid always gives the same
//! pieces in the same order, and a piece is named by how it was made ([`PieceKey`]): for each cut that divided it, the
//! cut's place in the recipe, the side of it the piece lies on, and which lump of that side it is. A cut that passes a
//! piece by leaves it, and its key, alone.
//!
//! **The cuts** ([`Cut`]), each with an optional gap (a joint: what lies within half of it of the cut is taken away):
//!
//! - a plane at any angle: the triangles are clipped one by one and each side is closed with the section's outline
//!   filled in ([`clip`], [`cap`]). Nothing is rounded and every new corner is made once for the edge it lies on, so the
//!   pieces of a closed solid are closed by construction;
//! - a closed line drawn on a face, through the part or to a depth, and a closed profile extruded over a range: the
//!   inside is the solid and the profile's prism shared, the outside the solid less it, by the binary space partitions
//!   of [`crate::csg`]. A partition leaves corners of one triangle on another's edge, so each piece is mended after it
//!   (`Soup::mend`): corners within [`WELD`] become one and an edge is cut at every corner on it;
//! - an open line drawn on a face: it cuts where it is drawn and nowhere else (a line across one jamb of a door leaves
//!   the other whole), so no plane or half space will do. The solid loses a wall as thin as [`KERF`] along the line,
//!   what is left falls into lumps, each on the left or the right of the line by the face the wall left on it, and the
//!   corners on that face are moved back onto the line: with no gap the pieces meet again exactly. A lump the wall left
//!   a face on both sides of was not cut through: the line doesn't cross it ([`Conflict::DoesNotCross`]). A straight
//!   line that spans the whole piece is the plane through it, and is cut as one.
//!
//! **What is promised.** The pieces of a plane cut are closed. The pieces of the other cuts are closed after mending in
//! every case tested, and each [`Piece`] says whether it is (`closed`), since a partition gives no proof. The volumes add
//! up: the pieces and what [`Outcome::removed`] counts (gaps, offsets, and lumps too thin to keep) make the solid, within
//! a millionth of it. A lump of less than [`MIN_VOLUME`] or thinner than [`THIN`] is noise and is left out.
//!
//! **Hostile input** ends in an error: numbers are checked finite and within `MAX_COORD`, counts are capped
//! ([`MAX_CUTS`], [`MAX_LINE_POINTS`], [`MAX_PIECES`], `csg::MAX_TRIANGLES`), and every loop spends from a budget.
//! [`DivideError::Input`] is a recipe or a solid that can't be used; [`DivideError::Conflict`] is a cut that no longer
//! meets the solid it was made for, which a caller shows on the part after its host changed ([`regenerate`] lists these
//! and goes on without those cuts).

mod cap;
mod clip;
mod flat;
mod mesh;
mod prism;
mod run;
mod shave;
#[cfg(test)]
mod tests;

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::geom::P2;
use crate::model::MAX_COORD;
use crate::view3d::{P3, add, cross, dot, mul, sub, unit};

pub use mesh::{area, area_facing, is_closed, volume};
pub use prism::{Prism, divide_prism};

/// A closed surface of triangles, each counter-clockwise seen from outside.
pub type Mesh = Vec<[P3; 3]>;

/// The most cuts a recipe holds, and the most faces it moves in.
pub const MAX_CUTS: usize = 64;
/// The most points a line or a profile has.
pub const MAX_LINE_POINTS: usize = 2_000;
/// The most pieces a solid is divided into.
pub const MAX_PIECES: usize = 256;
/// The widest gap, and the furthest a face is moved in, mm.
pub const MAX_GAP: f64 = 10_000.0;
/// How near corners are one, and how near a plane a corner lies on it, mm.
pub const WELD: f64 = 1e-5;
/// The wall an open line takes out to part what lies either side of it, mm thick; with no gap, the faces it leaves are
/// moved back onto the line.
pub const KERF: f64 = 2e-3;
/// How far past its ends an open line cuts, mm: a line drawn up to an outline's edge crosses it.
pub const REACH: f64 = 1.0;
/// A lump of less than this, mm³, is noise.
pub const MIN_VOLUME: f64 = 1.0;
/// A lump thinner than this, mm (twice its volume over its surface), is noise.
pub const THIN: f64 = 0.01;
/// How far from a direction, degrees, a face may turn and still face it ([`area_facing`]'s usual tolerance).
pub const FACING: f64 = 1.0;
/// Points of a line closer than this, mm, are one.
pub(crate) const MIN_SEGMENT: f64 = 0.01;

/// A plane with its own axes: a point of it, the way its x runs, and its normal. A line or a profile is drawn in its
/// coordinates: x along `x`, y along `normal × x`, seen from where the normal points. On a part's face the normal points
/// out of the part. It holds its x itself, not the level one a [`crate::workplane::Frame`] works out, so a recipe kept in
/// its host's coordinates turns with the host.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub origin: P3,
    pub x: P3,
    pub normal: P3,
}

impl From<crate::workplane::Frame> for Frame {
    /// A work plane with its own axes: x level across it.
    fn from(p: crate::workplane::Frame) -> Frame {
        Frame { origin: p.origin, x: p.axes().0, normal: p.normal }
    }
}

impl Frame {
    /// Its axes made square and of length one, or what's wrong with it.
    pub(crate) fn axes(&self) -> Result<Axes, String> {
        if !place(self.origin) || !place(self.x) || !place(self.normal) {
            return Err("its frame has a number that isn't finite, or is beyond 1000 km".into());
        }
        if dot(self.normal, self.normal).sqrt() < 1e-9 {
            return Err("its frame's normal has no length: give the way the face looks".into());
        }
        let n = unit(self.normal);
        let across = sub(self.x, mul(n, dot(self.x, n)));
        if dot(across, across).sqrt() < 1e-9 {
            return Err("its frame's x has no length, or runs along its normal: give a direction in the face".into());
        }
        let x = unit(across);
        Ok(Axes { o: self.origin, x, y: cross(n, x), n })
    }

    /// The model point at `p` in its plane, `along` mm off it the way its normal points; none for a frame that is no plane.
    pub fn point(&self, p: P2, along: f64) -> Option<P3> {
        self.axes().ok().map(|a| a.point(p, along))
    }

    /// A model point in its coordinates: where it is in its plane, and how far off it along its normal.
    pub fn local(&self, p: P3) -> Option<(P2, f64)> {
        self.axes().ok().map(|a| a.local(p))
    }
}

/// A frame's axes, square and of length one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Axes {
    pub o: P3,
    pub x: P3,
    pub y: P3,
    pub n: P3,
}

impl Axes {
    pub fn point(&self, p: P2, along: f64) -> P3 {
        add(add(self.o, mul(self.x, p.0)), add(mul(self.y, p.1), mul(self.n, along)))
    }

    pub fn local(&self, p: P3) -> (P2, f64) {
        let d = sub(p, self.o);
        (P2(dot(d, self.x), dot(d, self.y)), dot(d, self.n))
    }

    /// A direction in its plane, as one in space.
    pub fn across(&self, d: P2) -> P3 {
        add(mul(self.x, d.0), mul(self.y, d.1))
    }
}

/// One cut of a solid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum Cut {
    /// A plane through `point`, square to `normal`, at any angle: what lies the way the normal points is
    /// [`Side::Front`], the rest [`Side::Back`]. A tilted one models a wall out of plumb or tapering.
    Plane { point: P3, normal: P3 },
    /// A line drawn on a face, in `frame`'s coordinates, as Revit's Divide Parts sketch. The cut runs square to the face.
    /// `closed`, it is an outline: what is in it is [`Side::Inside`], the rest [`Side::Outside`]; through the whole part,
    /// or, with a `depth`, only that far in from the face (the frame's normal points out of the part), what is behind
    /// staying with the rest. Open, it must cross the piece from outline to outline (it may cross openings on the way),
    /// and cuts through the whole part: what is on its left, going from its first point to its last and seen from where
    /// the normal points, is [`Side::Left`], the rest [`Side::Right`]. A straight line needs two points.
    Sketch {
        frame: Frame,
        points: Vec<P2>,
        #[serde(default)]
        closed: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        depth: Option<f64>,
    },
    /// A closed profile in `frame`'s coordinates, extruded along the frame's normal from `from` to `to`, mm off its
    /// plane (the way the normal points is positive); without one, through everything on that side. What is in it is
    /// [`Side::Inside`], the rest [`Side::Outside`]: a lacuna, a niche, any volume with straight sides.
    Profile {
        frame: Frame,
        points: Vec<P2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<f64>,
    },
}

impl Cut {
    /// The two sides it divides a piece into, in the order their pieces come.
    pub fn sides(&self) -> [Side; 2] {
        match self {
            Cut::Plane { .. } => [Side::Front, Side::Back],
            Cut::Sketch { closed: false, .. } => [Side::Left, Side::Right],
            Cut::Sketch { .. } | Cut::Profile { .. } => [Side::Inside, Side::Outside],
        }
    }
}

/// A cut as a recipe holds it: with its gap, and the piece it divides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Division {
    pub cut: Cut,
    /// A joint, mm wide: what lies within half of it of the cut is taken away, so the pieces don't touch.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gap: f64,
    /// The piece it divides, as the cuts before it left it, and whatever they made of that piece since; without one,
    /// every piece it meets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<PieceKey>,
}

impl From<Cut> for Division {
    fn from(cut: Cut) -> Division {
        Division { cut, gap: 0.0, of: None }
    }
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// Which side of a cut a piece lies on: front and back of a plane, left and right of an open line, inside and outside
/// a closed one or a profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Front,
    Back,
    Left,
    Right,
    Inside,
    Outside,
}

/// One cut's share in a piece's name: the cut's place in the recipe (from 0), the side of it the piece is on, and which
/// lump of that side: where a cut leaves several lumps apart on one side, they are counted from 0 in order of their
/// centres of volume in the cut's frame, by x, then y, then along its normal, each compared to the millimetre (a
/// plane's frame is a work plane's: x level across it). Lumps that touch only along an edge or at a corner are one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub cut: u32,
    pub side: Side,
    pub lump: u32,
}

/// A piece's name: the cuts that divided it, in order; none for the solid whole. It stays the same as long as the same
/// cuts divide the same pieces, however the host moves or stretches.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PieceKey(pub Vec<Step>);

impl PieceKey {
    /// Whether this piece is `other`, or was made of it.
    pub fn is_of(&self, other: &PieceKey) -> bool {
        self.0.starts_with(&other.0)
    }
}

impl fmt::Display for PieceKey {
    /// As `0.front.0/2.inside.1`; `whole` for the solid undivided.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("whole");
        }
        for (k, s) in self.0.iter().enumerate() {
            let side = match s.side {
                Side::Front => "front",
                Side::Back => "back",
                Side::Left => "left",
                Side::Right => "right",
                Side::Inside => "inside",
                Side::Outside => "outside",
            };
            write!(f, "{}{}.{side}.{}", if k > 0 { "/" } else { "" }, s.cut, s.lump)?;
        }
        Ok(())
    }
}

/// A face of a piece, named so as to be found again after the host changes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum FaceRef {
    /// The face the cut at this place in the recipe made on the piece.
    Cut(u32),
    /// The piece's flat face in the plane through `point` that looks the way `normal` points (out of the piece), within
    /// [`FACING`] degrees and a thousandth of a millimetre.
    Plane { point: P3, normal: P3 },
}

/// A face of a finished piece moved into it by `distance` mm: erosion, missing material, a joint on one side only. A
/// face moves in, never out: what would be added isn't in the solid to take.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offset {
    pub piece: PieceKey,
    pub face: FaceRef,
    pub distance: f64,
}

/// All that is kept of a division: its cuts in order, then the faces moved in on the finished pieces.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    #[serde(default)]
    pub cuts: Vec<Division>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offsets: Vec<Offset>,
}

/// One piece of a divided solid.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub key: PieceKey,
    pub mesh: Mesh,
    /// What it holds, mm³.
    pub volume: f64,
    /// Whether every edge of its surface is shared by two triangles, one each way ([`is_closed`]).
    pub closed: bool,
}

/// A solid divided: its pieces in the order of their keys (a piece's parts take its place, first side first), what the
/// gaps, the offsets and the noise took of it, mm³, and, from [`regenerate`], the cuts and offsets left out.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub pieces: Vec<Piece>,
    pub removed: f64,
    pub conflicts: Vec<(Stage, Conflict)>,
}

impl Outcome {
    /// The piece of this name.
    pub fn piece(&self, key: &PieceKey) -> Option<&Piece> {
        self.pieces.iter().find(|p| &p.key == key)
    }
}

/// A place in a recipe: a cut or an offset, counted from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Cut(usize),
    Offset(usize),
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stage::Cut(k) => write!(f, "cut {k}"),
            Stage::Offset(k) => write!(f, "offset {k}"),
        }
    }
}

/// Why a cut or an offset that was good no longer meets the solid: what a part shows after its host changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conflict {
    /// It passes by every piece it was to divide, or only grazes them: all of each stays on one side.
    Misses,
    /// A closed line or a profile holds all of every piece it meets: nothing is left outside it.
    HoldsAll,
    /// An open line stops short of a piece's outline, or ends in an opening that piece closes round: it parts nothing.
    DoesNotCross,
    /// A cut to a depth is as deep as the part is thick, or deeper: nothing lies behind it.
    TooDeep,
    /// The piece it names is not among the pieces any more.
    NoSuchPiece,
    /// The face an offset names is not on its piece any more.
    NoSuchFace,
    /// An offset takes all its piece has.
    NothingLeft,
}

impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Conflict::Misses => "it no longer meets the part: all of the part lies on one side of it; move it onto the part, or remove it",
            Conflict::HoldsAll => "it holds the whole part, so nothing is left outside it: make it smaller, or remove it",
            Conflict::DoesNotCross => {
                "the line doesn't cross the part from edge to edge: draw it past the part's outline at both ends \
                 (through an opening it ends in), or close it"
            }
            Conflict::TooDeep => {
                "it is as deep as the part is thick, so nothing lies behind it: give a smaller depth, or none to cut through"
            }
            Conflict::NoSuchPiece => "the piece it names is no longer there: the cuts before it don't make that piece any more",
            Conflict::NoSuchFace => "the piece has no flat face in that plane looking that way any more: name the face as it is now",
            Conflict::NothingLeft => "it takes all the piece has: give a smaller distance",
        })
    }
}

/// Why a solid wasn't divided.
#[derive(Clone, Debug, PartialEq)]
pub enum DivideError {
    /// The solid or the recipe can't be used as given: what to fix.
    Input(String),
    /// A cut or an offset of a good recipe doesn't meet this solid.
    Conflict { at: Stage, conflict: Conflict },
    /// More work than a division may take.
    TooMuch(String),
}

impl fmt::Display for DivideError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DivideError::Input(what) | DivideError::TooMuch(what) => f.write_str(what),
            DivideError::Conflict { at, conflict } => write!(f, "{at}: {conflict}"),
        }
    }
}

impl std::error::Error for DivideError {}

/// What a division may cost: steps of its own loops, and the partitions' meter.
pub(crate) struct Work {
    steps: u64,
    pub meter: crate::csg::Meter,
}

impl Work {
    pub fn new() -> Work {
        Work { steps: 400_000_000, meter: crate::csg::Meter::times(8) }
    }

    pub fn spend(&mut self, steps: usize) -> Result<(), DivideError> {
        self.steps = self.steps.checked_sub(steps as u64 + 1).ok_or_else(too_much)?;
        Ok(())
    }
}

pub(crate) fn too_much() -> DivideError {
    DivideError::TooMuch("the division is too much work: give the solid fewer triangles, or the recipe fewer cuts and points".into())
}

/// A partition's refusal as a division's.
pub(crate) fn from_csg(e: String) -> DivideError {
    DivideError::TooMuch(format!("a piece can't be cut: {e}"))
}

/// Whether `p` is a place: finite, and within `MAX_COORD` of the origin.
pub(crate) fn place(p: P3) -> bool {
    p.iter().all(|v| v.is_finite() && v.abs() <= MAX_COORD)
}

/// `solid` divided by `recipe`: the pieces, each with its key. An error says what is wrong with the solid or the recipe
/// ([`DivideError::Input`]), or which cut or offset no longer meets the solid and why ([`DivideError::Conflict`]).
pub fn divide(solid: &[[P3; 3]], recipe: &Recipe) -> Result<Outcome, DivideError> {
    run::run(solid, recipe, false)
}

/// [`divide`] for a recipe made for a host that has since changed: a cut or an offset that no longer meets the solid is
/// left out and listed in the outcome's `conflicts`, and the rest is done. The pieces the other cuts make keep their
/// keys, a key holding each cut's own place in the recipe.
pub fn regenerate(solid: &[[P3; 3]], recipe: &Recipe) -> Result<Outcome, DivideError> {
    run::run(solid, recipe, true)
}

/// `solid` cut by the plane through `point` square to `normal`: what lies the way the normal points, and the rest, each
/// closed; either is empty where the plane passes the solid by.
pub fn cut_plane(solid: &[[P3; 3]], point: P3, normal: P3) -> Result<(Mesh, Mesh), DivideError> {
    let shape = run::Shape::of(&Cut::Plane { point, normal }).map_err(DivideError::Input)?;
    let mut work = Work::new();
    let soup = run::solid_of(solid, &mut work)?;
    let run::Shape::Plane { n, w, .. } = shape else { return Ok((Vec::new(), Vec::new())) };
    let (front, back) = clip::split(&soup, n, w, &mut work)?;
    Ok((front.mesh(), back.mesh()))
}
