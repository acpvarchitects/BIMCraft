//! GEO-2c: how near a family's curves are drawn. A form's arcs, circles and revolves are cut into chords: by default
//! [`super::SEGMENTS`] a turn, as always. A view that shows them larger asks for a [`Fineness`], a distance in mm (a power
//! of two) that no chord may stray from its curve by; each curve then takes as many chords as its own radius needs, never
//! fewer than the default and never more than [`Fineness::MOST`] a turn.
//!
//! The fineness is the build's, not each call's: [`Fineness::scoped`] sets it for whatever is built inside it (on this
//! thread; a family is built on one), and the places that chord a curve read it with [`steps`].

use std::cell::Cell;

/// How near a build's curves are drawn: none, the default's [`super::SEGMENTS`] chords a turn; or within `2^e` mm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fineness(Option<i8>);

/// The finest a view may ask for, `2^FINEST` mm, and the coarsest that still changes anything.
const FINEST: i8 = -4;
const COARSEST: i8 = 8;
/// GEO-2c-2b: how far past its band's edge, in powers of two, a view's scale comes before a build is drawn in another band.
const HALF_STEP: f64 = 0.5;

thread_local! {
    static NOW: Cell<Fineness> = const { Cell::new(Fineness(None)) };
    /// GEO-2g: while a build is [`recording`], the coarsest fineness any curve chorded so far would take more chords at.
    static NEEDS: Cell<Option<Needs>> = const { Cell::new(None) };
}

/// What a build recorded of its curves ([`recording`]): the coarsest fineness, `2^e` mm, at which any of them would take more
/// chords than the default gives it; none where no fineness would. A build at a fineness no coarser than that may differ from
/// the default one; at any other it is the default one, bit for bit, since [`steps`] is all a fineness changes.
pub type Needs = Option<i8>;

impl Fineness {
    /// The default: every curve [`super::SEGMENTS`] chords a turn.
    pub const DEFAULT: Fineness = Fineness(None);
    /// The most chords a curve takes a turn, however near a view comes.
    pub const MOST: usize = 2048;

    /// What a view showing `scale` pixels to the mm asks for: chords within about half a pixel, rounded down to a power of
    /// two so views near in scale share a build. The default for a scale that isn't a positive number.
    pub fn for_view(scale: f64) -> Fineness {
        if !(scale.is_finite() && scale > 0.0) {
            return Fineness::DEFAULT;
        }
        let e = (0.5 / scale).log2().floor();
        if e >= f64::from(COARSEST) {
            return Fineness::DEFAULT;
        }
        Fineness(Some(e.max(f64::from(FINEST)) as i8))
    }

    /// How far a chord may stray from its curve, mm; none for the default.
    pub fn tolerance(self) -> Option<f64> {
        self.0.map(|e| 2f64.powi(i32::from(e)))
    }

    /// Whether a build at this fineness may differ from the default one, given what the default one recorded
    /// ([`recording`]). False for the default itself.
    pub fn changes(self, needs: Needs) -> bool {
        matches!((self.0, needs), (Some(e), Some(n)) if e <= n)
    }

    /// GEO-2c-2b: the fineness a build drawn at this one, whose default build recorded `needs`, is drawn at where a view shows
    /// it at `scale` px a mm. A band is a power of two of the scale, as [`Fineness::for_view`] rounds it; the default's is
    /// every scale that asks for a fineness no finer than its curves need. While the scale is within half a step (half a power
    /// of two) of this one's band, this one, so a view sitting on a band's edge (an orbit, a zoom by a hair) doesn't build
    /// back and forth; past that, what [`Fineness::for_view`] asks. Either is the default where it builds nothing otherwise,
    /// and this one where the scale isn't a positive number.
    pub fn banded(self, scale: f64, needs: Needs) -> Fineness {
        let now = if self.changes(needs) { self } else { Fineness::DEFAULT };
        let Some(n) = needs else { return Fineness::DEFAULT };
        let u = (0.5 / scale).log2();
        if !(scale.is_finite() && scale > 0.0 && u.is_finite()) {
            return now;
        }
        // Where its band is, in powers of two of half a pixel's mm: the finest's reaching down, the default's up.
        let (lo, hi) = match now.0 {
            Some(e) if e <= FINEST => (f64::NEG_INFINITY, f64::from(e) + 1.0),
            Some(e) => (f64::from(e), f64::from(e) + 1.0),
            None => (f64::from(n) + 1.0, f64::INFINITY),
        };
        if u >= lo - HALF_STEP && u < hi + HALF_STEP {
            return now;
        }
        let asked = Fineness::for_view(scale);
        if asked.changes(needs) { asked } else { Fineness::DEFAULT }
    }

    /// `f`, with what it builds at this fineness. The fineness before is back after it, however `f` ends.
    pub fn scoped<T>(self, f: impl FnOnce() -> T) -> T {
        struct Back(Fineness);
        impl Drop for Back {
            fn drop(&mut self) {
                NOW.with(|n| n.set(self.0));
            }
        }
        let _back = Back(NOW.with(|n| n.replace(self)));
        f()
    }

    /// The fineness what is being built now is at.
    pub fn now() -> Fineness {
        NOW.with(Cell::get)
    }
}

/// Chords for a curve of `radius` turning `turn` degrees, at the fineness now: the default's `turn / 360 *`
/// [`super::SEGMENTS`] rounded up (at least one), or more where that strays further than the fineness allows, up to
/// [`Fineness::MOST`] a turn.
pub(crate) fn steps(turn: f64, radius: f64) -> usize {
    if let Some(was) = NEEDS.get() {
        // The coarsest fineness this curve would take more chords at: the finer, the more it takes.
        let mine = (FINEST..COARSEST).rev().find(|e| steps_at(turn, radius, Some(2f64.powi(i32::from(*e)))) > steps_at(turn, radius, None));
        NEEDS.set(Some(was.max(mine)));
    }
    steps_at(turn, radius, Fineness::now().tolerance())
}

/// [`steps`] within `tolerance` mm, or at the default.
fn steps_at(turn: f64, radius: f64, tolerance: Option<f64>) -> usize {
    let turns = turn.abs() / 360.0;
    let default = (turns * super::SEGMENTS as f64 - 1e-9).ceil().max(1.0) as usize;
    let Some(tol) = tolerance else { return default };
    if !(radius.is_finite() && radius > tol && turns.is_finite()) {
        return default;
    }
    // A chord of a turn `t` strays `radius (1 - cos(t / 2))` from its arc.
    let per_turn = (std::f64::consts::PI / (1.0 - tol / radius).acos()).min(Fineness::MOST as f64);
    let needed = (turns * per_turn - 1e-9).ceil();
    if needed.is_finite() && needed > default as f64 { needed as usize } else { default }
}

/// `f`, and what the curves it chords record ([`Needs`]): what it builds says which finenesses would build it otherwise. One
/// recording inside another adds to it.
pub fn recording<T>(f: impl FnOnce() -> T) -> (T, Needs) {
    struct Back(Option<Needs>);
    impl Drop for Back {
        fn drop(&mut self) {
            let inner = NEEDS.get().flatten();
            NEEDS.set(self.0.map(|outer| outer.max(inner)));
        }
    }
    let back = Back(NEEDS.replace(Some(None)));
    let out = f();
    let needs = NEEDS.get().flatten();
    drop(back);
    (out, needs)
}

/// The chords of a whole turn, as [`steps`] gives them: what a circle is drawn with.
pub(crate) fn ring(radius: f64) -> usize {
    steps(360.0, radius)
}

/// GEO-2c-2a: how far past the box of a family's default build a finer build of it may reach, as a share of the box's
/// diagonal. A section that draws a component finer culls it, and finds whether its line cuts it, by its box widened by this
/// much ([`finer_reach`]).
///
/// A fineness changes only the curves [`steps`] chords: a sketch's arcs and circles and a revolve's turn (a family's sketches
/// have no ellipses or splines). Each is cut evenly at corners on its curve, and at the default no chord turns more than a
/// [`super::SEGMENTS`]th of a turn, its count rounded up (`steps_at`). A chord turning `t` strays from its arc by
/// `tan(t / 4) / 2` of its length, and both its ends are corners of the default build, so it is no longer than the box's
/// diagonal. A surface curved two ways (an arc revolved, or swept along an arc) strays from each quad of the default's
/// corners by the two strays at once, its middle further out than either edge's: a point of it lies within one stray of a
/// chord of its profile, and that chord, turned or swept, within the other of the quad's four corners. A sweep's mitred
/// profile lies, out on a bend, no farther than the default's mitre puts it, so within the same reach. A finer build's
/// corners lie on those surfaces and its triangles between them, so all of it lies within `tan(t / 4)` of the diagonal of
/// the box: `tan(pi / 64)`, 0.0491, rounded up. No form is curved three ways. A void only takes away: where it takes the
/// default's corners either side of a curve's widest point, its own corners where it crosses the chords stand for them; only
/// a void that grazes a chord, crossing none, could leave a sliver of the finer curve past what it leaves.
pub(crate) const STRAY: f64 = 0.05;

/// `b`, the box of a component's default build, widened each way by [`STRAY`] of its diagonal: as far as a finer build of it
/// may reach. One whose diagonal isn't a finite number, as it is.
pub(crate) fn finer_reach([lo, hi]: [[f64; 3]; 2]) -> [[f64; 3]; 2] {
    let d = STRAY * (0..3).map(|i| (hi[i] - lo[i]).powi(2)).sum::<f64>().sqrt();
    if !d.is_finite() {
        return [lo, hi];
    }
    [lo.map(|v| v - d), hi.map(|v| v + d)]
}
