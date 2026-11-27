//! Depth-ordered project owners beneath a pointer, including those covered by another owner.
use super::*;

/// One project owner under the pointer. Its projected edges let the view light it through the covering geometry.
#[derive(Clone, Debug)]
pub struct PickCandidate {
    pub id: Id,
    pub edges: Vec<[P2; 2]>,
    depth: f64,
}

struct Ray<'a> {
    camera: &'a Camera,
    section: Option<&'a Clip>,
    look: &'a Look,
    at: P2,
    project: Projector,
    hits: BTreeMap<Id, PickCandidate>,
}

impl<'a> Ray<'a> {
    fn new(camera: &'a Camera, section: Option<&'a Clip>, look: &'a Look, (x, y): (f64, f64)) -> Self {
        Self { camera, section, look, at: P2(x.floor() + 0.5, y.floor() + 0.5), project: camera.projector(), hits: BTreeMap::new() }
    }

    fn may_hit(&self, bounds: &[P3; 2]) -> bool {
        let P2(x, y) = self.at;
        Tiles::screen(&self.project, bounds).is_none_or(|[x0, x1, y0, y1, ..]| x >= x0 - 3. && x <= x1 + 3. && y >= y0 - 3. && y <= y1 + 3.)
    }

    fn contains(&self, x: f64, y: f64, depth: f64) -> bool {
        depth.is_finite()
            && match self.section {
                Some(c) => {
                    depth <= c.near
                        && c.far.is_none_or(|far| depth >= c.near - far)
                        && (c.x.0..=c.x.1).contains(&x)
                        && (c.y.0..=c.y.1).contains(&y)
                }
                None => kept(self.look, self.camera.at_depth((x, y), depth)),
            }
    }

    fn face_depth(&self, points: &[P3; 3]) -> Option<f64> {
        let (parts, count) = self.project.in_front(points);
        let at = (self.at.0, self.at.1);
        parts[..count]
            .iter()
            .filter_map(|weights| {
                let tri = weighed(points, weights).map(|p| self.project.at(p));
                geom::depth_at(tri, at).filter(|z| self.contains(at.0, at.1, *z))
            })
            .max_by(f64::total_cmp)
    }

    /// Clip the line in the same camera, section range and crop that produced the picture.
    fn line(&self, ends: [P3; 2]) -> Option<([P2; 2], f64)> {
        let [a, b] = match self.section {
            Some(_) => ends,
            None => self.look.section_box().map_or(Some(ends), |bx| within(ends[0], ends[1], bx))?,
        };
        let [a, b] = self.camera.in_front(a, b)?;
        let segment = (self.project.at(a), self.project.at(b));
        let (a, b) = self.section.map_or(Some(segment), |c| kept_in_depth(segment, c))?;
        let (p, q) = (P2(a.0, a.1), P2(b.0, b.1));
        let (lo, hi) = self
            .section
            .map_or((P2(0., 0.), P2(self.camera.width.into(), self.camera.height.into())), |c| (P2(c.x.0, c.y.0), P2(c.x.1, c.y.1)));
        let (start, end) = clip(p, q, lo, hi)?;
        let t = geom::param_on_segment(self.at, p, q);
        Some(([start, end], a.2 + (b.2 - a.2) * t))
    }

    fn add(&mut self, id: Id, depth: f64, lines: Vec<[P2; 2]>) {
        let h = self.hits.entry(id).or_insert_with(|| PickCandidate { id, depth, edges: Vec::new() });
        h.depth = h.depth.max(depth);
        h.edges.extend(lines);
    }

    fn owner(&mut self, id: Id, triangles: &[Tri]) {
        if crate::link::is_linked(id) || !self.look.shows(id) {
            return;
        }
        let wire = self.section.is_none() && self.look.style() == VisualStyle::Wireframe;
        let shown = |t: &&Tri| t.seen_in(self.look) && self.look.forms_show(t.vis, false);
        let mut depth =
            if wire { None } else { triangles.iter().filter(shown).filter_map(|t| self.face_depth(&t.p)).max_by(f64::total_cmp) };
        if depth.is_none() && !wire {
            return;
        }
        // Restrict edge extraction to this actual occurrence so coincident outlines keep both identities.
        let mut part = Scene::bare(triangles.iter().filter(shown).copied().collect());
        part.planes = planes(&part.tris);
        let lines: Vec<_> = part.edges().iter().filter_map(|(_, seg)| self.line(*seg)).collect();
        if wire {
            depth =
                lines.iter().filter(|([a, b], _)| geom::dist_to_segment(self.at, *a, *b) <= 2.8).map(|(_, z)| *z).max_by(f64::total_cmp);
        }
        if let Some(z) = depth {
            self.add(id, z, lines.into_iter().map(|(line, _)| line).collect());
        }
    }

    fn strokes(&mut self, strokes: &[Stroke]) {
        let mut lines: BTreeMap<Id, Vec<([P2; 2], f64)>> = BTreeMap::new();
        for s in strokes {
            if !crate::link::is_linked(s.owner)
                && self.look.shows(s.owner)
                && self.look.forms_show(s.vis, false)
                && self.look.line_edge(s.owner, s.sub, EDGE).is_some()
                && let Some(line) = self.line(s.ends)
            {
                lines.entry(s.owner).or_default().push(line);
            }
        }
        for (id, lines) in lines {
            let z =
                lines.iter().filter(|([a, b], _)| geom::dist_to_segment(self.at, *a, *b) <= 2.5).map(|(_, z)| *z).max_by(f64::total_cmp);
            if let Some(z) = z {
                self.add(id, z, lines.into_iter().map(|(line, _)| line).collect());
            }
        }
    }

    fn scans(&mut self, scans: &[Scan]) {
        let kept: Vec<_> = scans.iter().filter(|s| self.look.shows(s.owner)).collect();
        let step = every(kept.iter().map(|s| s.points.len()).sum());
        for scan in kept {
            if crate::link::is_linked(scan.owner) {
                continue;
            }
            let target = (self.at.0.floor() as i64, self.at.1.floor() as i64);
            let z = scan
                .points
                .iter()
                .step_by(step)
                .filter_map(|p| {
                    let (x, y, z) = self.project.at(scan.place(*p));
                    let (px, py) = dot_at((x, y, z), (self.camera.width as usize, self.camera.height as usize))?;
                    ((px..px + DOT as i64).contains(&target.0) && (py..py + DOT as i64).contains(&target.1) && self.contains(x, y, z))
                        .then_some(z)
                })
                .max_by(f64::total_cmp);
            if let Some(z) = z {
                let mut outline = Vec::new();
                for i in 0..8 {
                    for bit in [1, 2, 4] {
                        if i & bit == 0
                            && let Some((line, _)) = self.line([scan.corners[i], scan.corners[i | bit]])
                        {
                            outline.push(line);
                        }
                    }
                }
                self.add(scan.owner, z, outline);
            }
        }
    }

    fn finish(self) -> Vec<PickCandidate> {
        let mut hits: Vec<_> = self.hits.into_values().collect();
        hits.sort_by(|a, b| b.depth.total_cmp(&a.depth).then(a.id.cmp(&b.id)));
        hits
    }
}

impl Scene {
    /// On Tab, resolve each distinct owner intersecting this pixel through the displayed 3D camera, nearest first.
    pub fn pick_all(&self, cam: &Camera, at: (f64, f64), look: &Look) -> Vec<PickCandidate> {
        if !(at.0 >= 0. && at.1 >= 0. && at.0 < f64::from(cam.width) && at.1 < f64::from(cam.height)) {
            return Vec::new();
        }
        let mut ray = Ray::new(cam, None, look, at);
        for span in &self.spans {
            if ray.may_hit(&span.bounds) {
                ray.owner(span.id, &self.tris[span.at.clone()]);
            }
        }
        ray.strokes(&self.strokes);
        ray.scans(&self.scans);
        for mark in level_marks(&self.levels, cam, &|id| look.shows(id).then_some((EDGE, false))) {
            let lines: Vec<_> = mark.lines.iter().filter_map(|(s, _)| ray.line(*s)).collect();
            let [a, b, c, d] = mark.head;
            let z = ray.face_depth(&[a, b, c]).or_else(|| ray.face_depth(&[a, c, d])).or_else(|| {
                lines.iter().filter(|([a, b], _)| geom::dist_to_segment(ray.at, *a, *b) <= 2.5).map(|(_, z)| *z).max_by(f64::total_cmp)
            });
            if let Some(z) = z {
                ray.add(mark.id, z, lines.into_iter().map(|(line, _)| line).collect());
            }
        }
        ray.finish()
    }

    pub(crate) fn section_pick_all(
        m: &Model,
        (solids, ranges, placed, boxes): Work<'_>,
        (cam, clip, fine): (&Camera, &Clip, Option<SectionFine>),
        look: &Look,
        at: (f64, f64),
    ) -> Vec<PickCandidate> {
        let mut ray = Ray::new(cam, Some(clip), look, at);
        let mut components = crate::component_geometry::Components::for_section(m, fine);
        for ((id, _), bounds) in placed.iter().zip(boxes) {
            if !look.shows(*id) || matches!(m.elements.get(*id), Some(Element::Mass(_))) {
                continue;
            }
            let bounds = if fine.is_some() && matches!(m.elements.get(*id), Some(Element::Component(_))) {
                bounds.map(crate::family::fineness::finer_reach)
            } else {
                *bounds
            };
            if bounds.as_ref().is_some_and(|b| !ray.may_hit(b)) {
                continue;
            }
            let mut tris = Vec::new();
            section_object(m, *id, ranges.get(id).cloned().unwrap_or(0..0), solids, look, &mut components, &mut tris);
            ray.owner(*id, &tris);
        }
        ray.strokes(&strokes_of(m));
        ray.scans(&scans_of(m));
        ray.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        engine::Session,
        render::{SectionWork, View, section_pick_all},
    };
    use serde_json::json;

    #[test]
    fn a_section_ray_orders_the_walls_in_range_and_skips_hidden_and_clipped_owners() {
        let mut s = Session::new();
        let wall = |s: &mut Session, y| s.execute("wall", &json!({"points":[[0,y],[5000,y]]})).unwrap()["ids"][0].as_u64().unwrap();
        // Create the distant wall first, to distinguish depth order from element order.
        let far = wall(&mut s, 1400);
        let near = wall(&mut s, 200);
        let beyond = wall(&mut s, 4200);
        let before = wall(&mut s, -1500);
        let section = s.execute("section", &json!({"start":[0,-400],"end":[5000,-400],"depth":3200})).unwrap()["id"].as_u64().unwrap();
        let view = View { width: 400, height: 320, cx: 2500., cy: 1400., scale: 0.1, angle: 0. };
        let (x, y) = view.px(P2(2500., 1400.));
        let query = |m: &Model, work: &SectionWork| {
            section_pick_all(m, work, section, view, view, (x.into(), y.into())).into_iter().map(|c| c.id).collect::<Vec<_>>()
        };
        let work = SectionWork::new_deferred(&s.model);
        assert_eq!(query(&s.model, &work), [near, far], "walls {before} and {beyond} lie outside the cut range");
        let m = s.model_mut();
        let guid = m.guid(near);
        let Some(Element::Section(sec)) = m.elements.get_mut(section) else { panic!("section") };
        sec.graphics.elements.entry(guid).or_default().hidden = true;
        assert_eq!(query(m, &work), [far]);
    }
}
