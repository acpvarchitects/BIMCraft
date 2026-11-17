//! A recipe run on a solid: each cut checked and made ready ([`Shape`]), then made on every piece it is for, a piece's
//! lumps taking its place under its key with one more step.

use super::mesh::Soup;
use super::{
    Axes, Conflict, Cut, DivideError, Division, KERF, MAX_CUTS, MAX_GAP, MAX_PIECES, MIN_VOLUME, Outcome, Piece, PieceKey, REACH, Recipe,
    Stage, Step, THIN, WELD, Work, clip, flat, from_csg, place, shave, tidy,
};
use crate::csg;
use crate::geom::{self, P2};
use crate::model::MAX_COORD;
use crate::view3d::{P3, cross, dot, unit};

/// How far past a solid a cut through it runs, mm.
pub(crate) const MARGIN: f64 = 10.0;
/// How near the face a wall left, mm, a triangle or a corner is on it.
const ON_FACE: f64 = 1e-4;

/// A cut made ready: checked, its axes square, its line tidy.
pub(crate) enum Shape {
    /// The plane `n · p = w`.
    Plane { n: P3, w: f64, axes: Axes },
    /// An open line through the part.
    Line { axes: Axes, line: Vec<P2> },
    /// A counter-clockwise ring, from `lo` to `hi` along the normal (none: through everything that way); `depth`, when
    /// it is a patch on a face.
    Ring { axes: Axes, ring: Vec<P2>, lo: Option<f64>, hi: Option<f64>, depth: Option<f64> },
}

impl Shape {
    pub fn of(cut: &Cut) -> Result<Shape, String> {
        let length = |d: Option<f64>| d.is_none_or(|d| d.is_finite() && d.abs() <= MAX_COORD);
        match cut {
            Cut::Plane { point, normal } => {
                if !place(*point) || !place(*normal) {
                    return Err("its point or its normal has a number that isn't finite, or is beyond 1000 km".into());
                }
                if dot(*normal, *normal).sqrt() < 1e-9 {
                    return Err("its normal has no length: give the way the plane looks".into());
                }
                let n = unit(*normal);
                let x = clip::square_to(n);
                Ok(Shape::Plane { n, w: dot(n, *point), axes: Axes { o: *point, x, y: cross(n, x), n } })
            }
            Cut::Sketch { frame, points, closed, depth } => {
                let axes = frame.axes()?;
                let line = flat::clean(points, *closed)?;
                match (closed, depth) {
                    (false, Some(_)) => Err("an open line cut only to a depth parts nothing: close the line round a patch, \
                                             or leave the depth out to cut through"
                        .into()),
                    (false, None) => Ok(Shape::Line { axes, line }),
                    (true, Some(d)) if !(d.is_finite() && *d >= THIN && *d <= MAX_COORD) => {
                        Err("its depth must be a length above zero: leave it out to cut through".into())
                    }
                    (true, d) => Ok(Shape::Ring { axes, ring: line, lo: d.map(|d| -d), hi: None, depth: *d }),
                }
            }
            Cut::Profile { frame, points, from, to } => {
                let axes = frame.axes()?;
                let ring = flat::clean(points, true)?;
                if !length(*from) || !length(*to) {
                    return Err("its `from` and `to` must be finite lengths".into());
                }
                if from.zip(*to).is_some_and(|(a, b)| b - a < THIN) {
                    return Err("its `to` must be above its `from`: they are heights along the frame's normal".into());
                }
                Ok(Shape::Ring { axes, ring, lo: *from, hi: *to, depth: None })
            }
        }
    }

    pub fn axes(&self) -> &Axes {
        match self {
            Shape::Plane { axes, .. } | Shape::Line { axes, .. } | Shape::Ring { axes, .. } => axes,
        }
    }
}

/// A piece on the way: its key and its surface.
#[derive(Clone)]
pub(crate) struct Part {
    pub key: PieceKey,
    pub soup: Soup,
}

/// A solid as it is cut: checked finite, closed and holding something.
pub(crate) fn solid_of(solid: &[[P3; 3]], work: &mut Work) -> Result<Soup, DivideError> {
    if solid.is_empty() {
        return Err(DivideError::Input("the solid has no triangles".into()));
    }
    if solid.len() > csg::MAX_TRIANGLES {
        return Err(DivideError::Input(format!(
            "the solid has {} triangles; it can't have more than {} to divide",
            solid.len(),
            csg::MAX_TRIANGLES
        )));
    }
    if solid.iter().flatten().any(|p| !place(*p)) {
        return Err(DivideError::Input("the solid has a corner that isn't finite, or is beyond 1000 km".into()));
    }
    work.spend(solid.len())?;
    let mut soup = Soup::weld(solid);
    soup.mend(work)?;
    if !soup.is_closed() {
        return Err(clip::not_closed());
    }
    if soup.volume() < MIN_VOLUME {
        return Err(DivideError::Input(
            "the solid holds nothing, or is inside out: its triangles must turn outward (counter-clockwise seen from outside)".into(),
        ));
    }
    Ok(soup)
}

/// What a partition gives, as a surface mended.
pub(crate) fn mended(tris: Vec<[P3; 3]>, work: &mut Work) -> Result<Soup, DivideError> {
    // A partition of a solid of many small faces that aren't in one plane (a finely faceted curve) cuts each face of
    // the cutter by all of them: past this it is no piece to keep.
    if tris.len() > 10 * csg::MAX_TRIANGLES {
        return Err(DivideError::TooMuch(format!(
            "this cut leaves a piece in {} triangles: the solid has too many small faces for a line or a profile to cut it; \
             give it fewer faces, or cut it by planes",
            tris.len()
        )));
    }
    work.spend(tris.len())?;
    let mut soup = Soup::weld(&tris);
    soup.mend(work)?;
    Ok(soup)
}

pub(crate) fn run(solid: &[[P3; 3]], recipe: &Recipe, lenient: bool) -> Result<Outcome, DivideError> {
    let bad = |at: Stage, what: String| DivideError::Input(format!("{at}: {what}"));
    if recipe.cuts.len() > MAX_CUTS || recipe.offsets.len() > MAX_CUTS {
        return Err(DivideError::Input(format!(
            "the recipe has {} cuts and {} offsets; it takes {MAX_CUTS} of each at most",
            recipe.cuts.len(),
            recipe.offsets.len()
        )));
    }
    let mut shapes = Vec::with_capacity(recipe.cuts.len());
    for (k, d) in recipe.cuts.iter().enumerate() {
        let at = Stage::Cut(k);
        shapes.push(Shape::of(&d.cut).map_err(|e| bad(at, e))?);
        if !(d.gap.is_finite() && (0.0..=MAX_GAP).contains(&d.gap)) {
            return Err(bad(at, format!("its gap must be a width from 0 to {MAX_GAP} mm")));
        }
        if d.of.as_ref().is_some_and(|key| key.0.len() > MAX_CUTS) {
            return Err(bad(at, "the piece it names has more steps than a recipe has cuts".into()));
        }
    }
    for (k, o) in recipe.offsets.iter().enumerate() {
        shave::check(o, recipe).map_err(|e| bad(Stage::Offset(k), e))?;
    }
    let mut work = Work::new();
    let whole = solid_of(solid, &mut work)?;
    let held = whole.volume();
    let mut parts = vec![Part { key: PieceKey::default(), soup: whole }];
    let mut conflicts = Vec::new();
    let mut settle = |at: Stage, conflict: Conflict| -> Result<(), DivideError> {
        if !lenient {
            return Err(DivideError::Conflict { at, conflict });
        }
        conflicts.push((at, conflict));
        Ok(())
    };
    for (k, (d, shape)) in recipe.cuts.iter().zip(&shapes).enumerate() {
        match apply(&parts, k, shape, d, &mut work)? {
            Ok(next) => parts = next,
            Err(conflict) => settle(Stage::Cut(k), conflict)?,
        }
        if parts.len() > MAX_PIECES {
            return Err(DivideError::TooMuch(format!("the recipe makes more than {MAX_PIECES} pieces: give it fewer cuts")));
        }
    }
    for (k, o) in recipe.offsets.iter().enumerate() {
        if let Err(conflict) = shave::apply(&mut parts, o, recipe, &shapes, &mut work)? {
            settle(Stage::Offset(k), conflict)?;
        }
    }
    let pieces: Vec<Piece> =
        parts.into_iter().map(|p| Piece { volume: p.soup.volume(), closed: p.soup.is_closed(), mesh: p.soup.mesh(), key: p.key }).collect();
    let removed = held - pieces.iter().map(|p| p.volume).sum::<f64>();
    Ok(Outcome { pieces, removed, conflicts })
}

/// What a cut makes of one piece: the lumps on its first side and on its second, or why it leaves the piece whole.
pub(crate) enum Cutting {
    Sides(Vec<Soup>, Vec<Soup>),
    Not(Conflict),
}

/// Cut `k` made on every piece it is for: the pieces after it, or why it divides none.
fn apply(parts: &[Part], k: usize, shape: &Shape, d: &Division, work: &mut Work) -> Result<Result<Vec<Part>, Conflict>, DivideError> {
    let sides = d.cut.sides();
    let rank = |c: Conflict| match c {
        Conflict::TooDeep => 2,
        Conflict::HoldsAll => 1,
        _ => 0,
    };
    let mut out = Vec::with_capacity(parts.len() + 1);
    let (mut met, mut any, mut why): (bool, bool, Option<Conflict>) = (false, false, None);
    for part in parts {
        if d.of.as_ref().is_some_and(|of| !part.key.is_of(of)) {
            out.push(part.clone());
            continue;
        }
        met = true;
        match cut(&part.soup, shape, d.gap, work)? {
            Cutting::Sides(a, b) => {
                any = true;
                for (side, lumps) in [(sides[0], a), (sides[1], b)] {
                    for (n, soup) in lumps.into_iter().enumerate() {
                        let mut key = part.key.clone();
                        key.0.push(Step { cut: k as u32, side, lump: n as u32 });
                        out.push(Part { key, soup: tidy::tidy(&soup, work)? });
                    }
                }
            }
            Cutting::Not(Conflict::DoesNotCross) => return Ok(Err(Conflict::DoesNotCross)),
            Cutting::Not(c) => {
                if why.is_none_or(|w| rank(c) > rank(w)) {
                    why = Some(c);
                }
                out.push(part.clone());
            }
        }
    }
    Ok(if !met {
        Err(Conflict::NoSuchPiece)
    } else if !any {
        Err(why.unwrap_or(Conflict::Misses))
    } else {
        Ok(out)
    })
}

/// Whether a lump is worth keeping: not less than [`MIN_VOLUME`], nor thinner than [`THIN`].
pub(crate) fn worth(lump: &Soup) -> bool {
    let (v, a) = (lump.volume(), lump.area());
    v >= MIN_VOLUME && 2.0 * v >= THIN * a
}

/// The lumps worth keeping, in the order their keys count them: by their centres in the cut's frame, x, then y, then
/// along the normal, each to the millimetre first.
pub(crate) fn sorted(lumps: Vec<Soup>, axes: &Axes) -> Vec<Soup> {
    let mut kept: Vec<([f64; 3], Soup)> = lumps
        .into_iter()
        .filter(worth)
        .map(|l| {
            let (q, along) = axes.local(l.centre());
            ([q.0, q.1, along], l)
        })
        .collect();
    kept.sort_by(|(a, _), (b, _)| {
        let whole = (0..3).map(|k| a[k].round().total_cmp(&b[k].round())).find(|o| o.is_ne());
        whole.unwrap_or_else(|| (0..3).map(|k| a[k].total_cmp(&b[k])).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal))
    });
    kept.into_iter().map(|(_, l)| l).collect()
}

/// Two sides as their lumps; a side with none is a conflict.
fn sides(a: Vec<Soup>, b: Vec<Soup>, axes: &Axes, none: [Conflict; 2]) -> Cutting {
    let (a, b) = (sorted(a, axes), sorted(b, axes));
    if a.is_empty() {
        Cutting::Not(none[0])
    } else if b.is_empty() {
        Cutting::Not(none[1])
    } else {
        Cutting::Sides(a, b)
    }
}

/// `soup` either side of the plane `n · p = w`, less what lies within `half` of it.
pub(crate) fn halves(soup: &Soup, n: P3, w: f64, half: f64, work: &mut Work) -> Result<(Soup, Soup), DivideError> {
    if half <= 0.0 {
        return clip::split(soup, n, w, work);
    }
    Ok((clip::split(soup, n, w + half, work)?.0, clip::split(soup, n, w - half, work)?.1))
}

/// How far a surface reaches along a frame's normal: its lowest and highest corners.
pub(crate) fn extent(soup: &Soup, axes: &Axes) -> (f64, f64) {
    soup.pts.iter().map(|p| axes.local(*p).1).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), w| (lo.min(w), hi.max(w)))
}

fn cut(soup: &Soup, shape: &Shape, gap: f64, work: &mut Work) -> Result<Cutting, DivideError> {
    let half = gap / 2.0;
    match shape {
        Shape::Plane { n, w, axes } => {
            let (front, back) = halves(soup, *n, *w, half, work)?;
            Ok(sides(front.lumps(work)?, back.lumps(work)?, axes, [Conflict::Misses; 2]))
        }
        Shape::Ring { axes, ring, lo, hi, depth } => {
            if half > 0.0 && flat::shrunk(ring, half).is_none() {
                return Err(DivideError::Input("its gap is too wide for its outline: nothing would be left inside it".into()));
            }
            let (inside, outside) = ring_sides(soup, axes, ring, (*lo, *hi), half, half, work)?;
            let deepest = inside.pts.iter().map(|p| axes.local(*p).1).fold(f64::INFINITY, f64::min);
            if depth.is_some_and(|d| !inside.tris.is_empty() && deepest > half - d + 1e-3) {
                return Ok(Cutting::Not(Conflict::TooDeep));
            }
            Ok(sides(inside.lumps(work)?, outside.lumps(work)?, axes, [Conflict::Misses, Conflict::HoldsAll]))
        }
        Shape::Line { axes, line } => line_sides(soup, axes, line, gap, work),
    }
}

/// `soup` inside a ring's prism drawn in by `shrink`, and outside it drawn out by `grow`: each as far as its range
/// goes, less and more the same.
pub(crate) fn ring_sides(
    soup: &Soup,
    axes: &Axes,
    ring: &[P2],
    (lo, hi): (Option<f64>, Option<f64>),
    shrink: f64,
    grow: f64,
    work: &mut Work,
) -> Result<(Soup, Soup), DivideError> {
    let wide = || DivideError::Input("its gap or offset is too wide for its outline: the outline would close up or cross itself".into());
    let inner = flat::shrunk(ring, shrink);
    let outer = if grow > 0.0 { Some(flat::shift(ring, true, -grow)).filter(|r| flat::sound(ring, r, true)) } else { Some(ring.to_vec()) };
    let outer = outer.ok_or_else(wide)?;
    if lo.zip(hi).is_some_and(|(l, u)| u - l - 2.0 * shrink < THIN) {
        return Err(wide());
    }
    let (below, above) = extent(soup, axes);
    if lo.is_some_and(|l| l + shrink >= above - WELD) || hi.is_some_and(|u| u - shrink <= below + WELD) {
        return Ok((Soup::default(), soup.clone()));
    }
    let mesh = soup.for_cutting();
    let range = |by: f64| (lo.map_or(below - MARGIN, |l| l + by), hi.map_or(above + MARGIN, |u| u - by));
    let inside = match inner {
        Some(inner) => {
            let (from, to) = range(shrink);
            let cutter = flat::prism(axes, &inner, from, to, work)?;
            mended(csg::intersect(&mesh, &cutter, &mut work.meter).map_err(from_csg)?, work)?
        }
        None => Soup::default(),
    };
    let (from, to) = range(-grow);
    let cutter = flat::prism(axes, &outer, from, to, work)?;
    let outside = mended(csg::subtract(&mesh, &cutter, &mut work.meter).map_err(from_csg)?, work)?;
    Ok((inside, outside))
}

/// The plane an open line of two points is, when it spans the whole of `soup`: its normal (the line's left) and where
/// it lies.
pub(crate) fn spanning(soup: &Soup, axes: &Axes, line: &[P2]) -> Option<(P3, f64)> {
    let &[a, b] = line else { return None };
    let (d, len) = ((b - a).unit(), a.dist(b));
    let spans = soup.pts.iter().all(|p| {
        let t = (axes.local(*p).0 - a).dot(d);
        t >= -REACH && t <= len + REACH
    });
    spans.then(|| {
        let n = axes.across(d.perp());
        (n, dot(n, axes.point(a, 0.0)))
    })
}

/// `soup` either side of an open line. A straight line that spans it is a plane. Any other takes a thin wall out along
/// itself, and what is left is on the left or the right by the face the wall left on it. With a gap the wall is the
/// gap. With none it is taken twice, once wholly on the right of the line and once wholly on its left, and each side is
/// kept from the time the wall lay across the line from it: its face is then on the line itself, and the two meet.
fn line_sides(soup: &Soup, axes: &Axes, line: &[P2], gap: f64, work: &mut Work) -> Result<Cutting, DivideError> {
    if let Some((n, w)) = spanning(soup, axes, line) {
        let (front, back) = halves(soup, n, w, gap / 2.0, work)?;
        return Ok(sides(front.lumps(work)?, back.lumps(work)?, axes, [Conflict::Misses; 2]));
    }
    let reach = flat::reaching(line);
    let (below, above) = extent(soup, axes);
    let mesh = soup.for_cutting();
    // How far the wall reaches on the left of the line, and on its right.
    let walls: &[(f64, f64)] = if gap > 0.0 { &[(gap / 2.0, gap / 2.0)] } else { &[(0.0, KERF), (KERF, 0.0)] };
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for (n, &(left, right)) in walls.iter().enumerate() {
        let band = flat::strip(&reach, left, right).ok_or_else(|| {
            DivideError::Input("the line turns too sharply for its gap: give a narrower gap, or a line with gentler corners".into())
        })?;
        let wall = flat::prism(axes, &band, below - MARGIN, above + MARGIN, work)?;
        let rest = mended(csg::subtract(&mesh, &wall, &mut work.meter).map_err(from_csg)?, work)?;
        let (on_left, on_right) = (flat::shift(&reach, false, left), flat::shift(&reach, false, -right));
        for lump in rest.lumps(work)? {
            let is_left = match (faces(&lump, axes, &on_left, -1.0, work)?, faces(&lump, axes, &on_right, 1.0, work)?) {
                (true, true) => return Ok(Cutting::Not(Conflict::DoesNotCross)),
                (true, false) => true,
                (false, true) => false,
                (false, false) => {
                    // A lump the line never touched: on the side of the line its centre is.
                    let q = axes.local(lump.centre()).0;
                    let nearest = line
                        .windows(2)
                        .min_by(|s, t| geom::dist_to_segment(q, s[0], s[1]).total_cmp(&geom::dist_to_segment(q, t[0], t[1])));
                    nearest.is_none_or(|s| (s[1] - s[0]).cross(q - s[0]) >= 0.0)
                }
            };
            if walls.len() == 1 || (n == 0) == is_left {
                (if is_left { &mut a } else { &mut b }).push(lump);
            }
        }
    }
    // A line through a pinch of the piece (where two parts of it touch at a corner) leaves what hangs there on both
    // sides: more than there was. It parts nothing cleanly.
    let both: f64 = a.iter().chain(&b).map(Soup::volume).sum();
    if both > soup.volume() * (1.0 + 1e-9) + MIN_VOLUME {
        return Ok(Cutting::Not(Conflict::DoesNotCross));
    }
    Ok(sides(a, b, axes, [Conflict::Misses; 2]))
}

/// Whether `lump` has a face the wall left: an upright triangle on `curve` (the wall's side: the line moved off) that
/// looks back at the line, `toward` saying which way that is against the curve's left.
fn faces(lump: &Soup, axes: &Axes, curve: &[P2], toward: f64, work: &mut Work) -> Result<bool, DivideError> {
    for t in &lump.tris {
        let [a, b, c] = lump.corners(t);
        let n = unit(cross(crate::view3d::sub(b, a), crate::view3d::sub(c, a)));
        if dot(n, n) < 0.5 || dot(n, axes.n).abs() > 1e-6 {
            continue;
        }
        work.spend(curve.len())?;
        let looks = P2(dot(n, axes.x), dot(n, axes.y));
        let mid = (axes.local(a).0 + axes.local(b).0 + axes.local(c).0) * (1.0 / 3.0);
        let on = curve
            .windows(2)
            .any(|s| geom::dist_to_segment(mid, s[0], s[1]) <= ON_FACE && looks.dot((s[1] - s[0]).unit().perp()) * toward > 0.99);
        if on {
            return Ok(true);
        }
    }
    Ok(false)
}
