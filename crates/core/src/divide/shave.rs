//! A face of a finished piece moved into it. The face is named by the cut that made it, or by its plane; what lies
//! within the distance behind it is taken off the piece. A face made by a cut is that cut made again further in, on the
//! piece's own side; a face named by its plane is clipped off by the plane moved in where the whole piece lies behind
//! it, and otherwise loses the slab behind the face itself.

use super::mesh::Soup;
use super::run::{MARGIN, Part, Shape, extent, mended, ring_sides, spanning, worth};
use super::{Conflict, DivideError, FACING, FaceRef, KERF, MAX_CUTS, MAX_GAP, Offset, Recipe, Side, Work, clip, flat, from_csg, place};
use crate::csg;
use crate::view3d::{cross, dot, sub, unit};

/// How near a plane, mm, a face's corners lie in it.
const NEAR: f64 = 1e-3;
/// How far in front of a face, mm, the slab taken from behind it starts, so no side of it lies in the face.
const PROUD: f64 = 0.01;

/// What is wrong with an offset as the recipe gives it, whatever the solid.
pub(crate) fn check(o: &Offset, recipe: &Recipe) -> Result<(), String> {
    if !o.distance.is_finite() {
        return Err("its distance isn't a number".into());
    }
    if o.distance < 0.0 {
        return Err("its distance is negative: a face moves into its piece, never out of it, where there is no solid to add".into());
    }
    if o.distance > MAX_GAP {
        return Err(format!("its distance is more than {MAX_GAP} mm"));
    }
    if o.piece.0.len() > MAX_CUTS {
        return Err("the piece it names has more steps than a recipe has cuts".into());
    }
    match o.face {
        FaceRef::Cut(c) if c as usize >= recipe.cuts.len() => {
            Err(format!("it names cut {c}, and the recipe has {} cuts", recipe.cuts.len()))
        }
        FaceRef::Cut(c) if !o.piece.0.iter().any(|s| s.cut == c) => {
            Err(format!("the piece it names wasn't made by cut {c}: name a cut of the piece's key, or the face by its plane"))
        }
        FaceRef::Cut(_) => Ok(()),
        FaceRef::Plane { point, normal } if !place(point) || !place(normal) => {
            Err("its plane has a number that isn't finite, or is beyond 1000 km".into())
        }
        FaceRef::Plane { normal, .. } if dot(normal, normal).sqrt() < 1e-9 => {
            Err("its plane's normal has no length: give the way the face looks".into())
        }
        FaceRef::Plane { .. } => Ok(()),
    }
}

/// The offset made on the piece it names; or why it can't be, the pieces as they were.
pub(crate) fn apply(
    parts: &mut [Part],
    o: &Offset,
    recipe: &Recipe,
    shapes: &[Shape],
    work: &mut Work,
) -> Result<Result<(), Conflict>, DivideError> {
    let Some(part) = parts.iter_mut().find(|p| p.key == o.piece) else { return Ok(Err(Conflict::NoSuchPiece)) };
    let (d, soup) = (o.distance, &part.soup);
    if d == 0.0 {
        return Ok(Ok(()));
    }
    let sharp = || DivideError::Input("the line turns too sharply to move its face that far in: give a smaller distance".into());
    let less: Soup = match o.face {
        FaceRef::Cut(c) => {
            let made = (shapes.get(c as usize), recipe.cuts.get(c as usize), part.key.0.iter().find(|s| s.cut == c));
            let (Some(shape), Some(division), Some(step)) = made else { return Ok(Err(Conflict::NoSuchPiece)) };
            let by = division.gap / 2.0 + d;
            let first = matches!(step.side, Side::Front | Side::Left | Side::Inside);
            let plane = match shape {
                Shape::Plane { n, w, .. } => Some((*n, *w)),
                Shape::Line { axes, line } => spanning(soup, axes, line),
                Shape::Ring { .. } => None,
            };
            match (plane, shape) {
                (Some((n, w)), _) if first => clip::split(soup, n, w + by, work)?.0,
                (Some((n, w)), _) => clip::split(soup, n, w - by, work)?.1,
                (None, Shape::Ring { axes, ring, lo, hi, .. }) if first => ring_sides(soup, axes, ring, (*lo, *hi), by, 0.0, work)?.0,
                (None, Shape::Ring { axes, ring, lo, hi, .. }) => ring_sides(soup, axes, ring, (*lo, *hi), 0.0, by, work)?.1,
                (None, Shape::Line { axes, line }) => {
                    // The strip from just across the line to `by` into the piece's side of it.
                    let reach = flat::reaching(line);
                    let band =
                        if first { flat::strip(&reach, by, KERF / 2.0) } else { flat::strip(&reach, KERF / 2.0, by) }.ok_or_else(sharp)?;
                    let (below, above) = extent(soup, axes);
                    let wall = flat::prism(axes, &band, below - MARGIN, above + MARGIN, work)?;
                    mended(csg::subtract(&soup.mesh(), &wall, &mut work.meter).map_err(from_csg)?, work)?
                }
                (None, Shape::Plane { .. }) => return Ok(Err(Conflict::NoSuchFace)),
            }
        }
        FaceRef::Plane { point, normal } => {
            let n = unit(normal);
            let w = dot(n, point);
            let level = FACING.to_radians().cos();
            let face: Vec<[u32; 3]> = soup
                .tris
                .iter()
                .filter(|t| {
                    let [a, b, c] = soup.corners(t);
                    [a, b, c].iter().all(|p| (dot(n, *p) - w).abs() <= NEAR) && dot(unit(cross(sub(b, a), sub(c, a))), n) >= level
                })
                .copied()
                .collect();
            if face.is_empty() {
                return Ok(Err(Conflict::NoSuchFace));
            }
            if soup.pts.iter().all(|p| dot(n, *p) - w <= NEAR) {
                clip::split(soup, n, w - d, work)?.1
            } else {
                let slab = flat::slab(&soup.pts, &face, n, PROUD, -d);
                mended(csg::subtract(&soup.mesh(), &slab, &mut work.meter).map_err(from_csg)?, work)?
            }
        }
    };
    let kept: Vec<Soup> = less.lumps(work)?.into_iter().filter(worth).collect();
    if kept.is_empty() {
        return Ok(Err(Conflict::NothingLeft));
    }
    part.soup = Soup::join(&kept);
    Ok(Ok(()))
}
