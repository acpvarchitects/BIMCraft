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
    boxed: Option<Boxed>,
    at: P2,
    project: Projector,
    hits: BTreeMap<Id, PickCandidate>,
}

impl<'a> Ray<'a> {
    fn new(camera: &'a Camera, section: Option<&'a Clip>, look: &'a Look, (x, y): (f64, f64)) -> Self {
        let boxed = look.section_box().filter(|_| section.is_none()).map(|b| Boxed::new(camera, b));
        Self { camera, section, look, boxed, at: P2(x.floor() + 0.5, y.floor() + 0.5), project: camera.projector(), hits: BTreeMap::new() }
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
                None => self.boxed.as_ref().is_none_or(|b| b.keeps(x, y, depth)),
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
    fn projected_line(&self, ends: [P3; 2]) -> Option<(Px, Px)> {
        let [a, b] = match self.section {
            Some(_) => ends,
            None => self.boxed.as_ref().map_or(Some(ends), |b| within(ends[0], ends[1], (b.lo, b.hi)))?,
        };
        let [a, b] = self.camera.in_front(a, b)?;
        let segment = (self.project.at(a), self.project.at(b));
        self.section.map_or(Some(segment), |c| kept_in_depth(segment, c))
    }

    fn line(&self, ends: [P3; 2]) -> Option<([P2; 2], f64)> {
        let (a, b) = self.projected_line(ends)?;
        let (p, q) = (P2(a.0, a.1), P2(b.0, b.1));
        let (lo, hi) = self
            .section
            .map_or((P2(0., 0.), P2(self.camera.width.into(), self.camera.height.into())), |c| (P2(c.x.0, c.y.0), P2(c.x.1, c.y.1)));
        let (start, end) = clip(p, q, lo, hi)?;
        let t = geom::param_on_segment(self.at, p, q);
        Some(([start, end], a.2 + (b.2 - a.2) * t))
    }

    fn wire_hit(&self, [a, b]: [P2; 2]) -> bool {
        let at = (self.at.0.floor() as i64, self.at.1.floor() as i64);
        geom::dist_to_segment(self.at, a, b) <= 4. && wire_pixels(a, b).any(|(x, y)| (x - at.0).abs() <= 2 && (y - at.1).abs() <= 2)
    }

    fn line_depth(&self, ends: [P3; 2], wide: u8) -> Option<f64> {
        if self.section.is_none() && self.look.style() == VisualStyle::Wireframe {
            let (line, z) = self.line(ends)?;
            return self.wire_hit(line).then_some(z);
        }
        let (a, b) = self.projected_line(ends)?;
        if geom::dist_to_segment(self.at, P2(a.0, a.1), P2(b.0, b.1)) > 2. {
            return None;
        }
        let size = (self.camera.width as usize, self.camera.height as usize);
        let pixel = self.at.1.floor() as usize * size.0 + self.at.0.floor() as usize;
        let mut depth: Option<f64> = None;
        outline_samples(a, b, size, wide, |k, z| {
            if k == pixel && self.contains(self.at.0, self.at.1, z) {
                depth = Some(depth.map_or(z, |d| d.max(z)));
            }
        });
        depth
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
            depth = lines.iter().filter(|(line, _)| self.wire_hit(*line)).map(|(_, z)| *z).max_by(f64::total_cmp);
        }
        if let Some(z) = depth {
            self.add(id, z, lines.into_iter().map(|(line, _)| line).collect());
        }
    }

    fn strokes(&mut self, strokes: &[Stroke]) {
        let mut lines: BTreeMap<Id, Vec<[P2; 2]>> = BTreeMap::new();
        let mut depths: BTreeMap<Id, f64> = BTreeMap::new();
        for s in strokes {
            if !crate::link::is_linked(s.owner)
                && self.look.shows(s.owner)
                && self.look.forms_show(s.vis, false)
                && self.look.line_edge(s.owner, s.sub, EDGE).is_some()
                && let Some((line, _)) = self.line(s.ends)
            {
                lines.entry(s.owner).or_default().push(line);
                if let Some(z) = self.line_depth(s.ends, 1) {
                    depths.entry(s.owner).and_modify(|d| *d = d.max(z)).or_insert(z);
                }
            }
        }
        for (id, z) in depths {
            self.add(id, z, lines.remove(&id).unwrap_or_default());
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
    pub fn pick_all(&self, cam: &Camera, at: (f64, f64), look: &Look, caps: Option<&Caps>, tints: &[(Id, Rgb)]) -> Vec<PickCandidate> {
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
        if look.style() == VisualStyle::Wireframe {
            return ray.finish();
        }
        ray.scans(&self.scans);
        // Caps are already cut to the box. Level heads sit just outside its clipped datum corners.
        let bounded = ray.boxed.take().map(|b| levels_in(&self.levels, (b.lo, b.hi)));
        if let Some(caps) = caps.filter(|_| look.style() != VisualStyle::Wireframe) {
            let mut hit_caps = BTreeSet::new();
            for tri in &caps.tris {
                if look.shows(tri.owner)
                    && !crate::link::is_linked(tri.owner)
                    && let Some(z) = ray.face_depth(&tri.p)
                {
                    ray.add(tri.owner, z, Vec::new());
                    hit_caps.insert(tri.owner);
                }
            }
            for &(line, _, owner) in &caps.lines {
                if hit_caps.contains(&owner)
                    && let Some((edge, _)) = ray.line(line)
                    && let Some(candidate) = ray.hits.get_mut(&owner)
                {
                    candidate.edges.push(edge);
                }
            }
        }
        for mark in level_marks(bounded.as_deref().unwrap_or(&self.levels), cam, &|id| {
            look.shows(id).then(|| (EDGE, tints.iter().any(|(owner, _)| *owner == id)))
        }) {
            let lines: Vec<_> = mark.lines.iter().filter_map(|(s, _)| ray.line(*s)).collect();
            let [a, b, c, d] = mark.head;
            let z = ray
                .face_depth(&[a, b, c])
                .or_else(|| ray.face_depth(&[a, c, d]))
                .or_else(|| mark.lines.iter().filter_map(|(ends, wide)| ray.line_depth(*ends, *wide)).max_by(f64::total_cmp));
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
        if !(at.0 >= 0. && at.1 >= 0. && at.0 < f64::from(cam.width) && at.1 < f64::from(cam.height)) {
            return Vec::new();
        }
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
    fn a_perspective_section_box_admits_only_the_owners_at_the_projected_world_point() {
        let mut s = Session::new();
        let mut wall = |x| s.execute("wall", &json!({"points":[[x,-1600],[x,1600]]})).unwrap()["ids"][0].as_u64().unwrap();
        let near = wall(900);
        let far = wall(300);
        let clipped = wall(1500);
        let scene = Scene::new(&s.model);
        let cam = Camera::at([2000., 0., 1450.], [0., 0., 1450.], 50., 400, 320);
        let look = Look::default().with_section_box(Some(([-100., -500., 1000.], [1150., 500., 2000.])));
        let (x, y, _) = cam.project([900., 110., 1480.]);
        let ids: Vec<_> = scene.pick_all(&cam, (x, y), &look, None, &[]).into_iter().map(|c| c.id).collect();
        assert_eq!(ids, [near, far], "wall {clipped} is outside the displayed box");
    }

    #[test]
    fn the_ray_uses_the_level_head_at_the_clipped_corner_and_filled_cap_under_the_pixel() {
        let mut scene = Scene::bare(Vec::new());
        scene.levels = vec![(71, "Ground".into(), [[-5000., -5000., 0.], [5000., -5000., 0.], [5000., 5000., 0.], [-5000., 5000., 0.]])];
        let mut cam = Camera::at([5000., -6000., 4500.], [0., 0., 0.], 50., 800, 600);
        cam.lens = None;
        cam.scale = 0.5;
        let bx = ([-300., -200., -100.], [300., 200., 300.]);
        let look = Look::default().with_section_box(Some(bx));
        let marks = level_marks(&levels_in(&scene.levels, bx), &cam, &|_| Some((EDGE, false)));
        let head = marks[0].head.map(|p| cam.project(p));
        let (x, y) = ((head[0].0 + head[2].0) / 2., (head[0].1 + head[2].1) / 2.);
        assert!(scene.pick_all(&cam, (x, y), &look, None, &[]).iter().any(|c| c.id == 71));
        let tri = [[-100., -150., 300.], [200., -150., 300.], [100., 150., 300.]];
        let caps = Caps {
            tris: vec![CapTri { p: tri, owner: 89, color: EDGE, normal: [0., 0., 1.], cap: 1 }],
            lines: vec![([tri[0], tri[1]], EDGE, 89)],
        };
        let (x, y, _) = cam.project([80., 0., 300.]);
        let hit = scene.pick_all(&cam, (x, y), &look, Some(&caps), &[]).into_iter().find(|c| c.id == 89).unwrap();
        assert_eq!(hit.edges.len(), 1);
        assert!(scene.pick_all(&cam, (x, y), &look, None, &[]).iter().all(|c| c.id != 89));
    }

    #[test]
    fn each_overlapping_model_line_uses_the_same_shaded_or_wire_pixels_as_its_picture() {
        let mut s = Session::new();
        let mut cam = Camera::at([1000., 0., 0.], [0., 0., 0.], 50., 40, 40);
        cam.lens = None;
        cam.scale = 1.;
        let mut scene = Scene::bare(Vec::new());
        scene.strokes = [40, 41].map(|owner| Stroke { owner, ends: [[0., -14., -6.], [0., 12., 7.]], vis: 0, sub: None }).to_vec();
        for style in ["shaded", "wireframe"] {
            s.execute("visual_style", &json!({"style":style})).unwrap();
            let look = Look::of_3d(&s.model);
            let pic = scene.render_in(&cam, &[], &look);
            let mut pixels = 0;
            for y in 8..33 {
                for x in 2..38 {
                    let at = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                    let candidates = scene.pick_all(&cam, at, &look, None, &[]);
                    if pic.pick(at.0, at.1).is_some() {
                        pixels += 1;
                        assert_eq!(candidates.iter().map(|c| c.id).collect::<Vec<_>>(), [40, 41], "{style} at {at:?}");
                    } else {
                        assert!(candidates.is_empty(), "{style} at {at:?}");
                    }
                }
            }
            assert!(pixels > 15);
        }
    }

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
