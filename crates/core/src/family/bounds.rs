//! Local envelopes of supported forms, without triangulation or boolean construction.
use super::*;
use crate::bounds::Bounds;

fn include(out: &mut Option<[P3; 2]>, p: P3) -> Option<()> {
    if !p.iter().all(|v| v.is_finite()) {
        return None;
    }
    *out = Some(match *out {
        None => [p, p],
        Some([lo, hi]) => [std::array::from_fn(|i| lo[i].min(p[i])), std::array::from_fn(|i| hi[i].max(p[i]))],
    });
    Some(())
}

impl Family {
    /// Conservative local bounds for visible swept profiles, revolves and imported meshes.
    /// No triangles or boolean results are constructed. None means unknown/empty;
    /// callers must keep the instance. A void only takes away from the solids it cuts, so it adds nothing to their box
    /// (GEO-2g); a revolve is boxed by the cylinder its profile turns within. Sweeps along a path return unknown, and so
    /// does an adaptive family, whose geometry its instances' points decide. These envelopes must not replace exact
    /// framing extrema.
    pub fn conservative_bounds(&self, values: &BTreeMap<String, Val>) -> Option<[P3; 2]> {
        // Where its dimensions put its planes, ends and sketches for the type (F7).
        match self.flexed(values).ok()? {
            Cow::Borrowed(f) => f.bounds_as_drawn(values),
            Cow::Owned(f) => f.bounds_as_drawn(values),
        }
    }

    /// [`Family::conservative_bounds`] of what is drawn, as it is drawn.
    fn bounds_as_drawn<'a>(&'a self, values: &BTreeMap<String, Val>) -> Option<[P3; 2]> {
        // Nested instances (T6a) are boxed by what they build, as an adaptive family is.
        if self.adaptive.is_some() || !self.nested_instances.is_empty() {
            return None;
        }
        let mut read = HashMap::new();
        let mut num = |e: &'a Expr| number(e, values, &mut read).ok();
        let mut out = None;
        for form in &self.forms {
            if let Some(visible) = &form.visible
                && num(visible)? == 0.0
            {
                continue;
            }
            if form.void {
                continue;
            }
            if !form.mesh.is_empty() {
                if form.mesh.len() > MAX_BUILT {
                    return None;
                }
                for p in form.mesh.iter().flatten() {
                    include(&mut out, *p)?;
                }
                continue;
            }
            // A sweep's solid follows its path: no box of it without building it.
            if form.kind().swept() || (form.revolves() && (!form.top.is_empty() || form.sweep.is_some())) {
                return None;
            }
            // A blend's box is its base's and its top's together, between its ends.
            let profile: Vec<P2> = match form.circle_ref().filter(|_| form.top.is_empty()) {
                Some((centre, radius)) => {
                    let (x, y, r) = (num(&centre[0])?, num(&centre[1])?, num(radius)?);
                    if r <= 0.0 {
                        return None;
                    }
                    // Enclose every circular profile angle, including every facet.
                    vec![P2(x - r, y - r), P2(x + r, y - r), P2(x + r, y + r), P2(x - r, y + r)]
                }
                None => {
                    // A sketch of arcs or of several loops: every point its lines reach, an arc by its chords' box.
                    let mut pts = Vec::new();
                    for l in form.boundary().chain(&form.top) {
                        let drawn = l.curve(&mut |e| num(e).ok_or_else(String::new)).ok()?;
                        let square =
                            |c: P2, r: f64| [P2(c.0 - r, c.1 - r), P2(c.0 + r, c.1 - r), P2(c.0 + r, c.1 + r), P2(c.0 - r, c.1 + r)];
                        match &drawn.curve {
                            crate::curves::Curve::Circle { centre, radius } => pts.extend(square(*centre, *radius)),
                            crate::curves::Curve::Poly { points, arcs, .. } => {
                                pts.extend(points.iter().copied());
                                if let ([a, b], Some(turn)) = (points.as_slice(), arcs.first()) {
                                    let ax = crate::walls::Axis::new(*a, *b, *turn);
                                    pts.extend(ax.centre().zip(ax.radius()).map(|(c, r)| square(c, r)).into_iter().flatten());
                                }
                            }
                            _ => return None,
                        }
                    }
                    if pts.is_empty() || pts.len() > 4 * crate::family::sketch::MAX_SKETCH {
                        return None;
                    }
                    pts
                }
            };
            let frame = self.frame(&form.host).ok()?;
            // A revolve turns its profile about its axis: within the cylinder round the axis, from its profile's nearest
            // to its farthest along it, as wide as its profile's farthest from it (the box of an arc's circle is farther
            // than any of the arc). Whatever its angles, a part of that.
            if let Some([a, b]) = form.axis_ends() {
                let (a, b) = (P2(num(&a[0])?, num(&a[1])?), P2(num(&b[0])?, num(&b[1])?));
                if a.dist(b).is_nan() || a.dist(b) < 1e-6 {
                    return None;
                }
                let (along, left) = ((b - a).unit(), (b - a).unit().perp());
                let (mut t0, mut t1, mut reach) = (f64::INFINITY, f64::NEG_INFINITY, 0.0_f64);
                for p in &profile {
                    let t = (*p - a).dot(along);
                    (t0, t1, reach) = (t0.min(t), t1.max(t), reach.max((*p - a).dot(left).abs()));
                }
                // And a hair more for the rounding of each corner it places.
                let reach = reach + 1e-6;
                let (o, u) = (frame.point(a, 0.0), frame.dir(along));
                for t in [t0, t1] {
                    for k in 0..8 {
                        let r = |i: usize| if (k >> i) & 1 == 0 { -reach } else { reach };
                        include(&mut out, std::array::from_fn(|i| o[i] + u[i] * t + r(i)))?;
                    }
                }
                continue;
            }
            let (from, to) = (num(&form.from)?, num(&form.to)?);
            if to <= from {
                return None;
            }
            for p in profile {
                include(&mut out, frame.point(p, from))?;
                include(&mut out, frame.point(p, to))?;
            }
        }
        match crate::bounds::padded(out?.into_iter()) {
            Bounds::Box(bounds) => Some(bounds),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encloses(f: &Family, values: &BTreeMap<String, Val>) {
        let [lo, hi] = f.conservative_bounds(values).unwrap();
        let pieces = f.pieces(values).unwrap();
        assert!(!pieces.is_empty());
        for p in pieces.iter().flat_map(|p| p.tris.iter().flatten()) {
            assert!((0..3).all(|i| lo[i] <= p[i] && p[i] <= hi[i]), "{p:?} outside {lo:?}..{hi:?}");
        }
    }

    #[test]
    fn swept_and_imported_family_envelopes_enclose_full_geometry() {
        let n = Expr::Number;
        let ring = vec![[n(-100.0), n(-50.0)], [n(700.0), n(-30.0)], [n(800.0), n(400.0)], [n(-80.0), n(300.0)]];
        for plane in [Plane::Xy, Plane::Xz, Plane::Yz] {
            for circle in [None, Some(Circle { centre: [n(-20.0), n(17.0)], radius: n(80.0) })] {
                let mut form = Form { host: plane.host(), from: n(-25.0), to: n(900.0), ..Default::default() };
                match &circle {
                    Some(c) => form.set_circle(c),
                    None => form.set_corners(&ring),
                }
                let f = Family { forms: vec![form], ..Default::default() };
                encloses(&f, &BTreeMap::new());
            }
        }
        let source: Family = serde_json::from_value(serde_json::json!({"forms":[{
            "profile":[[0,0],[500,0],[500,300],[0,300]],"from":-20,"to":100}]}))
        .unwrap();
        let mesh = source.pieces(&BTreeMap::new()).unwrap().into_iter().flat_map(|p| p.tris).collect();
        encloses(&Family { forms: vec![Form { mesh: Arc::new(mesh), ..Default::default() }], ..Default::default() }, &BTreeMap::new());
    }

    #[test]
    fn default_family_envelopes_cover_their_supported_types() {
        let mut supported = 0;
        for family in library(crate::language::Language::English).values() {
            for name in family.types.keys() {
                let values = family.values(name, &BTreeMap::new()).unwrap();
                if family.conservative_bounds(&values).is_some() {
                    encloses(family, &values);
                    supported += 1;
                }
            }
        }
        assert!(supported >= 20, "the library must exercise supported geometry, not only unknown bounds");
    }

    #[test]
    fn voids_add_nothing_revolves_are_boxed_and_nonfinite_forms_keep_unknown_bounds() {
        let f: Family = serde_json::from_value(serde_json::json!({"forms":[{
            "profile":[[0,0],[400,0],[400,300],[0,300]],"to":900}]}))
        .unwrap();
        let mut unsupported = f.clone();
        unsupported.forms[0].void = true;
        assert!(unsupported.conservative_bounds(&BTreeMap::new()).is_none());
        unsupported.forms[0].visible = Some(Expr::Number(0.0));
        unsupported.forms.extend(f.forms.clone());
        encloses(&unsupported, &BTreeMap::new());
        unsupported = f.clone();
        // A revolve is boxed by the cylinder it turns within (GEO-2g), whole or a part of a turn.
        unsupported.forms[0].set_axis(Some(&[[Expr::Number(0.0), Expr::Number(0.0)], [Expr::Number(0.0), Expr::Number(1.0)]]));
        for (from, to) in [(0.0, 360.0), (30.0, 100.0), (-90.0, 45.0)] {
            (unsupported.forms[0].from, unsupported.forms[0].to) = (Expr::Number(from), Expr::Number(to));
            encloses(&unsupported, &BTreeMap::new());
        }
        // Nothing but a void: no solid to box.
        unsupported = f.clone();
        unsupported.forms[0].void = true;
        assert!(unsupported.conservative_bounds(&BTreeMap::new()).is_none());
        unsupported = f;
        unsupported.forms[0].to = Expr::Number(f64::NAN);
        assert!(unsupported.conservative_bounds(&BTreeMap::new()).is_none());
    }
}
