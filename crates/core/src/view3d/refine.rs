//! GEO-2c-2b: a scene refined for its camera. A scene is made with every component at the default fineness, as it always was;
//! [`Scene::refined`] then makes again only the components whose band the camera changes, each at about half a pixel where it
//! is seen ([`crate::family::Fineness::for_view`]): a parallel view's scale for every one, a perspective view's at the nearest
//! point of the component's default build's box, placed, widened by as far as a finer build may reach past it
//! ([`crate::family::fineness::finer_reach`]). A component whose widened box is off the picture (beside it, or behind the eye)
//! stays as it is; one changes band only half a step past its band's edge ([`crate::family::Fineness::banded`]); finer builds are spent
//! nearest first within a budget of triangles, past which the farther ones are drawn at the default. Copies in one band share a
//! shape (`ShapeKey::Family` holds the fineness), so the card still draws them as copies. Nothing else reads it: a scene made
//! for an export, a sheet or IFC is as before.

use std::sync::Arc;

use super::{Element, FineOf, Finer, Id, Model, NEAR, P3, Scene, ShapeKey, Tri, dot, sub, update};
use crate::component_geometry::Components;
use crate::family::Fineness;
use crate::graphics::Look;

/// The most triangles a scene's refined components may have, every copy counted: as many of the scene's triangles as there are
/// bytes in the store's budget for finer builds ([`crate::component_geometry::BUDGET`], 256 MB), some two million.
const FINER_TRIS: usize = crate::component_geometry::BUDGET / std::mem::size_of::<Tri>();

/// How many components refining builds at once, on the plans' workers, before it counts what they cost.
const AT_ONCE: usize = 64;

/// The memory guard (cp's review of #99): running out of memory is a crash nothing catches (`memory.rs`), so a refinement
/// asks first whether it fits in what the machine has free, less `memory::room`'s margin, and else refines nothing. It may
/// take, beside the scene it is made from, bytes a triangle: the scene copied (a `Tri` of 128, its plane, face and smooth
/// normals' handle, some 160: the shapes and the normals themselves are shared), a card's mesh made from it (three vertices of
/// 52 and a picking triangle of 112, some 270) and the mesh's picking hierarchy, rounded up to [`COPIED`] for each triangle of
/// the scene; and for each triangle a finer build adds, that again, its build's own (72, kept in the store) and its smooth
/// normals (88): [`ADDED`]. The scene's must fit; what is left past it bounds the finer triangles, within [`FINER_TRIS`]. A
/// machine that can't say what is free (the tests, a library) refines as ever. Counted for the mesh whether or not a card
/// draws it: the processor's picture needs none, so on it the guard is conservative.
const COPIED: u64 = 512;
const ADDED: u64 = 672;

#[cfg(test)]
thread_local! {
    static BUDGET: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// The triangles refining may spend: [`FINER_TRIS`], or a test's.
fn budget() -> usize {
    #[cfg(test)]
    if let Some(b) = BUDGET.with(std::cell::Cell::get) {
        return b;
    }
    FINER_TRIS
}

/// `f`, with the 3D view's refining on this thread spending no more than `tris` triangles on finer builds.
#[cfg(test)]
pub(crate) fn with_finer_budget<T>(tris: usize, f: impl FnOnce() -> T) -> T {
    struct Back(Option<usize>);
    impl Drop for Back {
        fn drop(&mut self) {
            BUDGET.with(|b| b.set(self.0));
        }
    }
    let _back = Back(BUDGET.with(|b| b.replace(Some(tris))));
    f()
}

/// A camera as refining reads it: its axes and target, half its picture, px, and its scale; in perspective, its focal length,
/// px, and how far the eye stands from the target.
struct Sight {
    back: P3,
    right: P3,
    up: P3,
    target: P3,
    half: (f64, f64),
    scale: f64,
    eye: Option<(f64, f64)>,
}

impl Sight {
    /// None where the camera isn't a number, or its picture is no size.
    fn of(cam: &super::Camera) -> Option<Sight> {
        let numbers = cam.scale.is_finite()
            && cam.scale > 0.0
            && cam.azimuth.is_finite()
            && cam.elevation.is_finite()
            && cam.target.iter().all(|v| v.is_finite());
        if !numbers || cam.width == 0 || cam.height == 0 {
            return None;
        }
        let eye = match cam.lens {
            None => None,
            Some(lens) => {
                let f = f64::from(cam.width) * lens / super::FILM;
                let d = f / cam.scale;
                if !(lens.is_finite() && lens > 0.0 && f.is_finite() && d.is_finite() && d > 0.0) {
                    return None;
                }
                Some((f, d))
            }
        };
        let (back, right, up) = cam.axes();
        let half = (f64::from(cam.width) / 2.0, f64::from(cam.height) / 2.0);
        Some(Sight { back, right, up, target: cam.target, half, scale: cam.scale, eye })
    }

    /// Of what lies within `corners` (a box's, placed): none where it is wholly off the picture, beside it or (in perspective)
    /// behind the eye; else the px a mm it is seen at (in perspective, its nearest point's, no nearer than [`NEAR`]) and how
    /// near it is, the less the nearer (in perspective, how far in front of the eye).
    fn sees(&self, corners: &[P3; 8]) -> Option<(f64, f64)> {
        let (w, h) = self.half;
        // Wholly left, right, below, above, behind.
        let mut past = [true; 5];
        let mut nearest = f64::INFINITY;
        for p in corners {
            let d = sub(*p, self.target);
            let (x, y, z) = (dot(d, self.right), dot(d, self.up), dot(d, self.back));
            if !(x.is_finite() && y.is_finite() && z.is_finite()) {
                return None;
            }
            match self.eye {
                None => {
                    let (x, y) = (x * self.scale, y * self.scale);
                    past = [past[0] && x < -w, past[1] && x > w, past[2] && y < -h, past[3] && y > h, false];
                    nearest = nearest.min(-z);
                }
                // Each side of the picture a plane through the eye: wholly past one, it is wholly off the picture.
                Some((f, e)) => {
                    let ahead = e - z;
                    past = [
                        past[0] && x * f + w * ahead < 0.0,
                        past[1] && w * ahead - x * f < 0.0,
                        past[2] && y * f + h * ahead < 0.0,
                        past[3] && h * ahead - y * f < 0.0,
                        past[4] && ahead < NEAR,
                    ];
                    nearest = nearest.min(ahead);
                }
            }
        }
        if past.iter().any(|p| *p) {
            return None;
        }
        let scale = match self.eye {
            None => self.scale,
            Some((f, _)) => f / nearest.max(NEAR),
        };
        scale.is_finite().then_some((scale, nearest))
    }
}

/// Whether the box round `corners` meets section box `(lo, hi)`.
fn in_box(corners: &[P3; 8], (lo, hi): (P3, P3)) -> bool {
    (0..3).all(|k| corners.iter().any(|p| p[k] >= lo[k]) && corners.iter().any(|p| p[k] <= hi[k]))
}

/// A component refining asks of: what it read of it, and the fineness the view asks of it now.
struct Asked<'a> {
    id: Id,
    c: &'a crate::model::Component,
    of: &'a FineOf,
    want: Fineness,
    /// How near it is, the less the nearer; infinity off the picture.
    near: f64,
    /// Its triangles in the scene now.
    tris: usize,
}

impl Scene {
    /// GEO-2c-2b: this scene of `m` with its components chorded as finely as `cam` shows them through `look`, made again where
    /// any changes band; none where none does. Only those components are made again, as an edit makes what it touched; the
    /// rest of the scene is as it was.
    pub fn refined(&self, m: &Model, cam: &super::Camera, look: &Look) -> Option<Scene> {
        self.refined_cancellable(m, cam, look, &|| false)
    }

    /// [`Scene::refined`], none when `cancelled`.
    pub fn refined_cancellable(&self, m: &Model, cam: &super::Camera, look: &Look, cancelled: &(dyn Fn() -> bool + Sync)) -> Option<Scene> {
        self.refined_with(m, cam, look, crate::memory::free(), cancelled)
    }

    /// [`Scene::refined_cancellable`] on a machine with `free` bytes of memory free (none: it can't say).
    pub fn refined_with(
        &self,
        m: &Model,
        cam: &super::Camera,
        look: &Look,
        free: Option<u64>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Option<Scene> {
        if cancelled() || self.fine.is_empty() {
            return None;
        }
        // The memory guard: none where the copy doesn't fit, and as many finer triangles as what is left holds.
        let budget = match free.map(crate::memory::room) {
            None => budget(),
            Some(room) => {
                let copied = (self.tris.len() as u64).saturating_mul(COPIED);
                if copied > room {
                    return None;
                }
                budget().min(usize::try_from((room - copied) / ADDED).unwrap_or(usize::MAX))
            }
        };
        let sight = Sight::of(cam)?;
        let cut = look.section_box();
        let mut asked: Vec<Asked> = Vec::new();
        for (n, (id, of)) in self.fine.iter().enumerate() {
            if n % 4096 == 0 && cancelled() {
                return None;
            }
            let (Some(span), Some(Element::Component(c))) = (self.span(*id), m.elements.get(*id)) else { continue };
            // Its box as far as a finer build may reach: a curve of it may show on the picture though its default box doesn't.
            let seen = of.reach().filter(|_| look.shows(*id)).filter(|k| cut.is_none_or(|b| in_box(k, b))).and_then(|k| sight.sees(&k));
            let (want, near) = match seen {
                Some((scale, near)) => (of.at.banded(scale, of.shape.needs), near),
                None => (of.at, f64::INFINITY),
            };
            if want == Fineness::DEFAULT && of.at == Fineness::DEFAULT {
                continue;
            }
            asked.push(Asked { id: *id, c, of, want, near, tris: span.at.len() });
        }
        // Nearest first: the budget runs out on the farthest.
        asked.sort_by(|a, b| a.near.total_cmp(&b.near).then(a.id.cmp(&b.id)));
        let (mut spent, mut over) = (0usize, false);
        let mut components = Components::kept(m);
        let mut finer = Finer::new();
        let mut changed: Vec<Id> = Vec::new();
        for batch in asked.chunks(AT_ONCE) {
            if cancelled() {
                return None;
            }
            if !over {
                let jobs: Vec<_> = batch
                    .iter()
                    .filter(|a| a.want != Fineness::DEFAULT && a.want != a.of.at)
                    .map(|a| (Arc::clone(&a.of.shape.key), a.c, a.want))
                    .collect();
                components.build_each(&jobs);
            }
            for a in batch {
                let at = if a.want == Fineness::DEFAULT || over {
                    Fineness::DEFAULT
                } else {
                    let cost = if a.want == a.of.at {
                        a.tris
                    } else {
                        components.built(&a.of.shape.key, a.c, a.want).pieces.iter().map(|p| p.tris.len()).sum()
                    };
                    if spent.saturating_add(cost) > budget {
                        over = true;
                        Fineness::DEFAULT
                    } else {
                        spent += cost;
                        a.want
                    }
                };
                if at != Fineness::DEFAULT {
                    finer.insert(a.id, at);
                }
                if at != a.of.at {
                    changed.push(a.id);
                }
            }
        }
        if changed.is_empty() || cancelled() {
            return None;
        }
        drop(components);
        changed.sort_unstable();
        let elements: Vec<(Id, &Element)> = changed.iter().filter_map(|id| Some((*id, m.elements.get(*id)?))).collect();
        let made = update::element_tris(m, &elements, None, Some(&finer));
        if cancelled() {
            return None;
        }
        Some(self.spliced(m, made, Some(&finer), cancelled)?.compacted())
    }

    /// The scene without the finer shapes no copy is drawn from any more: what refining back and forth would pile up. The
    /// default's, the tubes' and the curtain walls' parts are kept, as a scene made whole has them.
    fn compacted(mut self) -> Scene {
        let mut keep = vec![false; self.shapes.len()];
        for (_, c) in &self.placements {
            if let Some(k) = keep.get_mut(c.shape) {
                *k = true;
            }
        }
        for (key, n) in &self.shape_keys {
            if !matches!(key, ShapeKey::Family(_, f) if *f != Fineness::DEFAULT)
                && let Some(k) = keep.get_mut(*n)
            {
                *k = true;
            }
        }
        if keep.iter().all(|k| *k) {
            return self;
        }
        let mut to = vec![usize::MAX; self.shapes.len()];
        let mut shapes = Vec::with_capacity(keep.iter().filter(|k| **k).count());
        for (n, shape) in std::mem::take(&mut self.shapes).into_iter().enumerate() {
            if keep.get(n).copied().unwrap_or(false) {
                to[n] = shapes.len();
                shapes.push(shape);
            }
        }
        self.shapes = shapes;
        self.shape_keys.retain(|_, n| to.get(*n).is_some_and(|t| *t != usize::MAX));
        for n in self.shape_keys.values_mut() {
            *n = to[*n];
        }
        for (_, c) in &mut self.placements {
            c.shape = to.get(c.shape).copied().unwrap_or(c.shape);
        }
        self
    }

    /// The fineness each component of the scene is drawn at, where it isn't the default.
    #[cfg(test)]
    pub(crate) fn finer(&self) -> Finer {
        self.fine.iter().filter(|(_, f)| f.at != Fineness::DEFAULT).map(|(id, f)| (*id, f.at)).collect()
    }

    /// The fineness component `id`'s triangles are asked at: drawn with its default build where it is too fine to cut at it.
    pub(crate) fn fineness_of(&self, id: Id) -> Fineness {
        self.fine.get(&id).map_or(Fineness::DEFAULT, |f| f.at)
    }

    /// The face of component `id` a point `at` on it lies on, seen along `normal` ([`Model::component_face`]), as the scene
    /// draws it (GEO-2c-2c): looked for in its build at the fineness its triangles are at, so a click on a refined curve, which
    /// may lie farther from the default's chords than the millimetre a face is looked for within, finds the face it is on; and
    /// where that build can't be made (too fine to cut), in its default one, which the scene draws then, as the store keeps it.
    pub fn component_face(&self, m: &Model, id: Id, at: P3, normal: P3) -> Option<crate::paint::FaceAt> {
        let Some(Element::Component(c)) = m.elements.get(id) else { return None };
        let fineness = self.fineness_of(id);
        let built = fineness.scoped(|| m.component_pieces(c));
        let pieces = if built.is_empty() && fineness != Fineness::DEFAULT { m.component_pieces(c) } else { built };
        crate::family::face_among(&pieces, at, normal)
    }

    /// How many of its components are drawn finer than the default.
    pub fn refined_count(&self) -> usize {
        self.fine.values().filter(|f| f.at != Fineness::DEFAULT).count()
    }
}
