//! The parts of building elements (`parts.rs`) in a plan, where its view shows them (`graphics/parts.rs`): each the cut
//! of its solid at the plan's cut plane, drawn as a layer of its host is, and what of it the plan sees below the cut
//! (above it, in a ceiling plan), outlined; a component's part as the component is drawn. Each primitive is its part's,
//! so a click picks the part and a selection highlights it.

use super::*;
use crate::parts::BuildingPart;

/// What a plan draws of the parts it shows: where its cut plane cuts them, and what it sees of them from it.
#[derive(Default)]
pub(super) struct Shown {
    /// Each piece the cut plane cuts: its part and its ring in plan.
    pub cut: Vec<(Id, Vec<P2>)>,
    /// Each piece seen from the cut plane, the farthest first, so the nearest is drawn over the rest: its part, its ring,
    /// and the height of its face toward the plane.
    pub seen: Vec<(Id, Vec<P2>, f64)>,
}

/// A closed outline in plan.
type Ring = Vec<P2>;

/// The pieces of part `p`'s solid a plan of range `r` cuts and sees, looking up from its cut (`up`, a ceiling plan) or down:
/// of each prism of the solid, its ring where the cut plane passes through it; else its ring whole, where its top lies
/// from the view depth up to the cut plane (looking up, its underside from the cut plane up to the range's top). None for
/// a component's part, which is no prisms, and for a part in conflict.
fn pieces(m: &Model, p: &BuildingPart, r: &PlanRange, up: bool) -> (Vec<Ring>, Vec<(Ring, f64)>) {
    let (mut cut, mut seen) = (Vec::new(), Vec::new());
    for prism in m.part_prisms(p).ok().flatten().unwrap_or_default() {
        let (lo, hi) = prism.ring.iter().fold((f64::MAX, f64::MIN), |(lo, hi), q| (lo.min(prism.bottom.at(*q)), hi.max(prism.top.at(*q))));
        if lo <= r.cut && r.cut < hi {
            // Under a sloped top or over a sloped underside, only where the plane is inside it.
            let ring = geom::clip(&geom::clip(&prism.ring, |q| r.cut - prism.bottom.at(q)), |q| prism.top.at(q) - r.cut);
            if ring.len() >= 3 && geom::area(&ring).abs() > 1e-6 {
                cut.push(ring);
            }
        } else if !up && r.depth - TOL < hi && hi <= r.cut {
            seen.push((prism.ring, hi));
        } else if up && r.cut < lo && lo <= r.top {
            seen.push((prism.ring, lo));
        }
    }
    (cut, seen)
}

/// What a plan of range `r` draws of the parts among `elements` that `look` shows.
pub(super) fn shown<'a>(m: &Model, elements: impl Iterator<Item = (Id, &'a Element)>, r: &PlanRange, up: bool, look: &Look) -> Shown {
    let mut out = Shown::default();
    for (id, e) in elements {
        let Element::Part(p) = e else { continue };
        if !look.shows(id) {
            continue;
        }
        let (cut, seen) = pieces(m, p, r, up);
        out.cut.extend(cut.into_iter().map(|ring| (id, ring)));
        out.seen.extend(seen.into_iter().map(|(ring, z)| (id, ring, z)));
    }
    // Looking down, the lowest first; looking up, the highest.
    out.seen.sort_by(|a, b| if up { b.2.total_cmp(&a.2) } else { a.2.total_cmp(&b.2) });
    out
}

/// The points a plan of range `r` shows of part `p`: what it is framed to.
pub(super) fn extent(m: &Model, p: &BuildingPart, r: &PlanRange) -> Vec<P2> {
    let (cut, seen) = pieces(m, p, r, false);
    cut.into_iter().chain(seen.into_iter().map(|(ring, _)| ring)).flatten().collect()
}

/// The parts a plan sees from its cut plane, in projection: each piece filled where the view fills its part's surfaces
/// (a floor's finish coloured by what it is), then outlined, the pieces of one part as one outline. Past the range's
/// bottom, faint, as a wall below is.
pub(super) fn seen(d: &mut Drawing, shown: &Shown, r: &PlanRange, look: &Look) {
    let outlines: Vec<(Id, Vec<P2>)> = shown.seen.iter().map(|(id, ring, _)| (*id, ring.clone())).collect();
    let edges = cut_edges(&outlines, 1.0 / d.view.scale, &|_| true);
    for ((id, ring, z), (_, runs)) in shown.seen.iter().zip(edges) {
        d.owner = Some(*id);
        if look.filled(*id) {
            d.fill(std::slice::from_ref(ring), PAPER);
        }
        let ink = if *z > r.bottom - TOL { INK } else { FAINT };
        for run in runs {
            match run.split_last() {
                Some((last, open)) if open.len() > 2 && open[0].dist(*last) < 1e-9 => d.line(open, BY_CATEGORY, ink, true),
                _ => d.line(&run, BY_CATEGORY, ink, false),
            }
        }
    }
}

/// A component's parts, where the plan sees the component: drawn as it is, each form's outline, the part's own.
pub(super) fn components<'a>(
    d: &mut Drawing,
    m: &'a Model,
    elements: impl Iterator<Item = (Id, &'a Element)>,
    r: &PlanRange,
    look: &Look,
    geometry: &mut crate::component_geometry::Components<'a>,
) {
    for (id, e) in elements {
        let Element::Part(p) = e else { continue };
        let Some(Element::Component(c)) = m.elements.get(p.host) else { continue };
        if !look.shows(id) {
            continue;
        }
        let (bottom, top) = geometry.heights(c);
        if bottom < r.cut && top > r.depth {
            d.owner = Some(id);
            component(d, m, &geometry.pieces(c));
        }
    }
}

/// The parts the plan cuts, over whatever else it cuts: each piece in poché, and where the part is a layer of its host's
/// type and the view isn't coarse, as that layer is drawn where it's cut (its material's cut pattern where it is wide
/// enough to hold it, else lighter where it isn't its host's structure), in the part's own material where it has one; then
/// each part's outline, the pieces of one part as one, so the line between two parts is drawn.
pub(super) fn cut(d: &mut Drawing, m: &Model, shown: &Shown) {
    for (id, ring) in &shown.cut {
        d.owner = Some(*id);
        d.fill(std::slice::from_ref(ring), POCHE);
        layer(d, m, *id, ring, None);
    }
    for (id, runs) in cut_edges(&shown.cut, 1.0 / d.view.scale, &|_| true) {
        d.owner = Some(id);
        for run in runs {
            ink_cut(d, &run);
        }
    }
}

/// Part `id`'s `ring`, where a plan or a section cuts it, as the layer it is: nothing for a part that is all of its
/// host, or in a coarse view, which leave it poché. `along`, the way its courses run in a section; none in a plan, where
/// they run along its wall.
pub(super) fn layer(d: &mut Drawing, m: &Model, id: Id, ring: &[P2], along: Option<P2>) {
    let Some(Element::Part(p)) = m.elements.get(id) else { return };
    let Some((_, of)) = m.part_layer(p).ok().flatten().filter(|_| d.detail != Detail::Coarse) else { return };
    let mut layer = of.clone();
    if let Some(own) = &p.material {
        layer.material = own.clone();
    }
    // How wide it is drawn: its thickness, across a wall in plan; else the least it measures either way.
    let (u, room) = match (along, m.elements.get(p.host)) {
        (None, Some(Element::Wall(w))) => ((w.end - w.start).unit(), layer.thickness * d.view.scale),
        (along, _) => {
            let (lo, hi) = bounds(ring).unwrap_or_default();
            (along.unwrap_or(P2(1.0, 0.0)), (hi.0 - lo.0).min(hi.1 - lo.1) * d.view.scale)
        }
    };
    cut_layer(d, m, &layer, ring, u, room);
}

/// The parts plan `plan` draws, of those `look` shows: those its cut plane cuts or sees (inside its crop, where it is
/// cropped), and the parts of the components it sees. What its theme's legend counts.
pub(crate) fn in_plan(m: &Model, plan: Id, look: &Look) -> std::collections::BTreeSet<Id> {
    let key = m.guid(plan);
    let r = m.shown_range(key);
    let crop = m.shown_crop(key);
    let inside = |ring: &[P2]| match (crop, bounds(ring)) {
        (Some([lo, hi]), Some((a, b))) => a.0 <= hi.0 && lo.0 <= b.0 && a.1 <= hi.1 && lo.1 <= b.1,
        (None, _) => true,
        (Some(_), None) => false,
    };
    let mut geometry = crate::component_geometry::Components::new(m);
    let mut out = std::collections::BTreeSet::new();
    for (id, e) in &m.elements {
        let Element::Part(p) = e else { continue };
        if !look.shows(*id) {
            continue;
        }
        let seen = match m.elements.get(p.host) {
            Some(Element::Component(c)) => {
                let (bottom, top) = geometry.heights(c);
                bottom < r.cut && top > r.depth && crop.is_none_or(|_| inside(&geometry.outlines(c)))
            }
            _ => {
                let (cut, seen) = pieces(m, p, &r, false);
                cut.iter().chain(seen.iter().map(|(ring, _)| ring)).any(|ring| inside(ring))
            }
        };
        if seen {
            out.insert(*id);
        }
    }
    out
}

/// The key of the view's theme (`graphics::parts::legend`), where the view asks for it and has parts to show: in the
/// drawing's top left corner, over the paper, a row for each value with its colour, how many parts hold it and their net
/// area. The view's own, so a sheet keeps it with the view; laid out as annotation is, no smaller than on paper and no
/// more than twice that, so a canvas zoomed far in or out still reads it.
pub(super) fn key(d: &mut Drawing, m: &Model, view: Id) {
    let Some(legend) = crate::graphics::parts::legend(m, view).filter(|l| l.drawn && !l.rows.is_empty()) else { return };
    let k = d.k.clamp(1.0, 2.0);
    let (pad, text, pitch, swatch) = (8.0 * k, 10.0 * k, 18.0 * k, (22.0 * k, 12.0 * k));
    let unset = m.language.word(bimcraft_i18n::msg!("not set")).to_string();
    let rows: Vec<(Rgb, String, String)> = legend
        .rows
        .iter()
        .map(|r| {
            let (area, unit) = m.units.area(r.area);
            let [red, green, blue] = r.color;
            (Rgb(red, green, blue), r.value.clone().unwrap_or_else(|| unset.clone()), format!("{}  {area:.2} {unit}", r.count))
        })
        .collect();
    let widest = |texts: &mut dyn Iterator<Item = &str>| texts.map(|t| font::width(t, text)).fold(0.0, f32::max);
    let (names, amounts) = (widest(&mut rows.iter().map(|r| r.1.as_str())), widest(&mut rows.iter().map(|r| r.2.as_str())));
    let width = (swatch.0 + pad + names + 2.0 * pad + amounts).max(font::width(&legend.by, text)) + 2.0 * pad;
    let height = 2.0 * pad + pitch * (rows.len() + 1) as f32;
    let (x, y) = (12.0 * k, 12.0 * k);
    let frame = vec![(x, y), (x + width, y), (x + width, y + height), (x, y + height)];
    d.owner = Some(view);
    d.part(Part::Projection);
    d.push(Prim::Fill { rings: vec![frame.clone()], color: PAPER });
    d.push(Prim::Line { pts: frame, width: k, color: INK, dash: None, closed: true });
    // Lettering is laid out about its middle: each text from its left end.
    let left = |d: &mut Drawing, t: &str, x: f32, y: f32| d.text(t, (x + font::width(t, text) / 2.0, y), text, INK);
    left(d, &legend.by, x + pad, y + pad + pitch / 2.0);
    for (n, (color, name, amount)) in rows.iter().enumerate() {
        let mid = y + pad + pitch * (n as f32 + 1.5);
        let (sx, sy) = (x + pad, mid - swatch.1 / 2.0);
        let ring = vec![(sx, sy), (sx + swatch.0, sy), (sx + swatch.0, sy + swatch.1), (sx, sy + swatch.1)];
        d.push(Prim::Fill { rings: vec![ring.clone()], color: *color });
        d.push(Prim::Line { pts: ring, width: k, color: INK, dash: None, closed: true });
        left(d, name, sx + swatch.0 + pad, mid);
        left(d, amount, x + width - pad - font::width(amount, text), mid);
    }
    d.owner = None;
}
