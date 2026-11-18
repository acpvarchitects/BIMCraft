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
