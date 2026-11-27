//! GEO-2c-2b: once the 3D view's camera settles, a worker refines its scene for it (its components chorded as finely as the
//! camera shows them) and the view swaps it in; a camera that moves on before it is made never gets it.

use super::*;
use bimcraft_core::view3d::{Camera, Scene};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A drum 1600 mm across and 2 m tall, standing at the origin.
const DRUM: &str = r#"family {"name": "Drum", "category": "generic", "types": {"T": {}}, "forms": [{"sketch": [{"centre": [0, 0], "radius": 800}], "from": 0, "to": 2000}]}
component {"family": "Drum", "type": "T", "at": [0, 0]}"#;

/// The scene the 3D view shows.
fn shown(h: &Harness<BimApp>) -> Arc<Scene> {
    h.state().orbit.scene.as_ref().map(|(_, s)| Arc::clone(s)).unwrap_or_else(|| panic!("a scene shown"))
}

/// The 3D view's camera looking north, level, `scale` px a mm, at `x` east and 1 m up: on the drum's side at `x` = 700.
fn look_at(h: &mut Harness<BimApp>, scale: f64, x: f64) {
    let cam = h.state().orbit.camera.unwrap_or_else(|| panic!("the 3D view has shown"));
    h.state_mut().orbit.camera = Some(Camera { azimuth: -90.0, elevation: 0.0, scale, target: [x, 0.0, 1000.0], lens: None, ..cam });
}

/// Frames until `done`, within a deadline.
fn until(h: &mut Harness<BimApp>, what: &str, done: impl Fn(&Harness<BimApp>) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !done(h) {
        assert!(Instant::now() < deadline, "{what}");
        h.step();
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The 3D view: zoomed in on the drum, once the camera settles its scene is refined, its drum drawn finer.
#[test]
fn zoomed_in_on_a_drum_the_3d_view_refines_its_scene_once_the_camera_settles() {
    let mut h = app(DRUM);
    press(&mut h, "Default 3D View");
    h.state_mut().orbit.refines = true;
    let before = shown(&h);
    assert_eq!(before.refined_count(), 0, "made at the default");
    look_at(&mut h, 2.0, 700.0);
    until(&mut h, "the scene refined", |h| shown(h).refined_count() == 1);
    assert!(!Arc::ptr_eq(&shown(&h), &before), "the refined scene swapped in");
    // Still there, the camera as it was: nothing more to refine.
    let refined = shown(&h);
    for _ in 0..4 {
        h.step();
    }
    until(&mut h, "the worker idle", |h| !h.state().orbit.refining());
    assert!(Arc::ptr_eq(&shown(&h), &refined), "nothing refined again");
}

/// GEO-2c-2d: in fly mode, with no key held and the mouse still, the camera stands still to the bit, so the view refines
/// its scene as it does for any camera that settles (it used to move by a rounding error each frame and never refine).
#[test]
fn flying_still_close_to_a_drum_the_3d_view_refines_its_scene() {
    let mut h = app(DRUM);
    press(&mut h, "Default 3D View");
    shortcut(&mut h, "FY");
    h.run();
    assert!(h.state().orbit.fly.is_some(), "flying: {:?}", h.state().status);
    // Looking down at the drum's side from 2 m off, at an angle.
    let cam = h.state().orbit.camera.unwrap_or_else(|| panic!("the 3D view has shown"));
    let (w, ht) = (cam.width, cam.height);
    h.state_mut().orbit.camera = Some(Camera::at([1_500.0, -2_300.0, 1_600.0], [0.0, -800.0, 1_000.0], 35.0, w, ht));
    h.state_mut().orbit.refines = true;
    h.step();
    let still = h.state().orbit.camera;
    until(&mut h, "the scene refined while flying still", |h| shown(h).refined_count() == 1);
    assert!(h.state().orbit.fly.is_some(), "still flying");
    assert_eq!(h.state().orbit.camera, still, "the camera stood where it was");
}

/// A refinement still being made when the camera moves on is dropped: the view never shows it, and refines for where it
/// looks now (away from the drum: nothing).
#[test]
fn a_camera_that_moves_on_before_its_refinement_is_made_never_gets_it() {
    let mut h = app(DRUM);
    press(&mut h, "Default 3D View");
    h.state_mut().orbit.refines = true;
    let before = shown(&h);
    look_at(&mut h, 2.0, 700.0);
    h.step();
    // The worker that refines for it held until the camera has moved on.
    let (entered, enter) = mpsc::channel();
    let (release, wait) = mpsc::channel::<()>();
    let ctx = h.ctx.clone();
    let mut first = true;
    h.state_mut().orbit.refine_worker = Some(crate::view_worker::Latest::new(
        move |job, cancel| {
            if first {
                first = false;
                entered.send(()).ok()?;
                wait.recv().ok()?;
            }
            crate::orbit::refine_scene(job, cancel)
        },
        move || ctx.request_repaint(),
    ));
    until(&mut h, "the refinement asked", |h| h.state().orbit.refine_asked.is_some());
    enter.recv_timeout(Duration::from_secs(20)).unwrap_or_else(|e| panic!("the refinement began: {e}"));
    // Far east of the drum: nothing of it on the screen.
    look_at(&mut h, 2.0, 50_000.0);
    h.step();
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    until(&mut h, "refined for where it looks now", |h| {
        let o = &h.state().orbit;
        o.refine_asked.as_ref().is_some_and(|(_, _, c)| c.target[0] == 50_000.0) && !o.refining()
    });
    for _ in 0..4 {
        h.step();
    }
    assert!(Arc::ptr_eq(&shown(&h), &before), "the refinement for the camera before never shown");
    assert_eq!(shown(&h).refined_count(), 0);
}
