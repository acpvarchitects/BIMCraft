//! GEO-2c-2e: the wall a door's or a window's family stands in, as its elevation's picture shows it. The picture has already
//! worked out the opening, what lies nearer and the view's graphics (a wall hidden or see-through shows nothing of it), so a
//! pick behind the wall is no one's where the picture's pixel is the wall's. Rows of pixels with the same runs of wall are
//! kept together, so a straight wall takes a few bands. (Ported from ec's d381329a2.)

use super::*;
use crate::geom::SelectBox;
use std::sync::{Arc, OnceLock};

pub(super) struct HostCover {
    view: View,
    raster: [u32; 2],
    hit: Arc<Vec<u32>>,
    owners: Arc<Vec<Id>>,
    wall: Id,
    bands: OnceLock<Vec<Band>>,
}

/// Rows `top` to `bottom` (exclusive) of the picture, each with the same runs of the wall's pixels, `spans`, left to right.
struct Band {
    top: u32,
    bottom: u32,
    spans: Vec<(u32, u32)>,
}

impl HostCover {
    /// Wall `wall` as `picture`, drawn through `view`, shows it; none where it shows none of it.
    pub(super) fn new(view: View, picture: &view3d::Image, wall: Id) -> Option<Self> {
        let (hit, owners) = picture.shared_hits();
        owners.contains(&wall).then(|| Self { view, raster: [picture.width, picture.height], hit, owners, wall, bands: OnceLock::new() })
    }

    fn bands(&self) -> &[Band] {
        self.bands.get_or_init(|| {
            let mut bands: Vec<Band> = Vec::new();
            let mut spans = Vec::new();
            for (y, row) in self.hit.chunks(self.raster[0].max(1) as usize).enumerate() {
                let y = y as u32;
                spans.clear();
                let mut start = None;
                for (x, &triangle) in row.iter().enumerate() {
                    let x = x as u32;
                    if self.owners.get(triangle as usize) == Some(&self.wall) {
                        start.get_or_insert(x);
                    } else if let Some(left) = start.take() {
                        spans.push((left, x));
                    }
                }
                if let Some(left) = start {
                    spans.push((left, self.raster[0]));
                }
                if let Some(last) = bands.last_mut().filter(|b| b.spans == spans) {
                    last.bottom = y + 1;
                } else {
                    bands.push(Band { top: y, bottom: y + 1, spans: spans.clone() });
                }
            }
            bands
        })
    }

    fn pixel(&self, at: P2) -> P2 {
        let (x, y) = self.view.px(at);
        P2(x.into(), y.into())
    }

    /// Whether the picture shows the wall at `at`, in the view's millimetres.
    pub(super) fn hides(&self, at: P2) -> bool {
        let P2(x, y) = self.pixel(at);
        if !(x >= 0.0 && y >= 0.0 && x < f64::from(self.raster[0]) && y < f64::from(self.raster[1])) {
            return false;
        }
        let at = y as usize * self.raster[0] as usize + x as usize;
        self.hit.get(at).and_then(|&t| self.owners.get(t as usize)) == Some(&self.wall)
    }

    /// The spans (as fractions from `a` to `b`) of the edge from `a` to `b`, in the view's millimetres, over the wall's pixels,
    /// added to `hidden`: an edge may leave the opening and come back into it.
    pub(super) fn hide_edge(&self, a: P2, b: P2, hidden: &mut Vec<(f64, f64)>) {
        let (a, b) = (self.pixel(a), self.pixel(b));
        for band in self.bands().iter().skip_while(|r| f64::from(r.bottom) <= a.1.min(b.1)).take_while(|r| f64::from(r.top) <= a.1.max(b.1))
        {
            let Some((lo, hi)) = interval(a.1, b.1, band.top, band.bottom) else { continue };
            for &(left, right) in &band.spans {
                if let Some((x0, x1)) = interval(a.0, b.0, left, right) {
                    let span = (lo.max(x0), hi.min(x1));
                    if span.0 < span.1 {
                        hidden.push(span);
                    }
                }
            }
        }
    }

    /// Whether crossing box `boxed` meets triangle `tri` (both in the view's millimetres) where the wall leaves it clear.
    pub(super) fn crosses(&self, boxed: &SelectBox, tri: &[P2; 3]) -> bool {
        let (a, b) = (self.pixel(boxed.lo), self.pixel(boxed.hi));
        let bounds = SelectBox::dragged(a, b);
        let tri = tri.map(|p| self.pixel(p));
        for band in self.bands().iter().skip_while(|r| f64::from(r.bottom) <= bounds.lo.1).take_while(|r| f64::from(r.top) <= bounds.hi.1) {
            let y0 = bounds.lo.1.max(f64::from(band.top));
            let y1 = bounds.hi.1.min(f64::from(band.bottom).next_down());
            let mut left = 0;
            for (start, end) in band.spans.iter().copied().chain(std::iter::once((self.raster[0], self.raster[0]))) {
                let x0 = bounds.lo.0.max(f64::from(left));
                let x1 = bounds.hi.0.min(f64::from(start).next_down());
                if left < start && x0 <= x1 && y0 <= y1 && SelectBox::dragged(P2(x0, y0), P2(x1, y1)).touches(&tri) {
                    return true;
                }
                left = end;
            }
        }
        false
    }
}

/// Where one coordinate of a segment from `a` to `b` lies within pixels `start` to `end` (exclusive): as fractions along it.
fn interval(a: f64, b: f64, start: u32, end: u32) -> Option<(f64, f64)> {
    let (start, end) = (f64::from(start), f64::from(end));
    if a == b {
        return (start <= a && a < end).then_some((0.0, 1.0));
    }
    let (t, u) = ((start - a) / (b - a), (end - a) / (b - a));
    let (lo, hi) = (t.min(u).max(0.0), t.max(u).min(1.0));
    (lo < hi).then_some((lo, hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crossing box over the wall's first pixel on its left and its top takes nothing behind it; one reaching past it does.
    #[test]
    fn hosted_elevation_crossing_excludes_the_first_wall_pixel_at_its_left_and_top_edges() {
        let view = View { scale: 1.0, cx: 0.0, cy: 0.0, width: 8, height: 8, angle: 0.0 };
        let cover = HostCover {
            view,
            raster: [8, 8],
            hit: Arc::new((0..64).map(|k| if k % 8 >= 2 && k / 8 >= 2 { 0 } else { u32::MAX }).collect()),
            owners: Arc::new(vec![1]),
            wall: 1,
            bands: OnceLock::new(),
        };
        let tri = [(0.0, 0.0), (8.0, 0.0), (0.0, 8.0)].map(|p| view.mm(p));
        for (left, top) in [(2.0, 3.0), (3.0, 2.0), (2.0, 2.0)] {
            let first = view.mm((left, top));
            let last = view.mm((left + 1.0, top + 1.0));
            assert!(cover.hides(first));
            assert!(!cover.crosses(&SelectBox::dragged(last, first), &tri));
        }
        assert!(cover.crosses(&SelectBox::dragged(view.mm((3.0, 3.0)), view.mm((1.5, 2.5))), &tri));
    }

    /// A picture smaller than its view is read by its own width.
    #[test]
    fn hosted_elevation_uses_the_image_stride_for_a_view_larger_than_its_raster() {
        let view = View { scale: 1.0, cx: 0.0, cy: 0.0, width: 12, height: 10, angle: 0.0 };
        let mut hit = vec![u32::MAX; 8 * 6];
        hit[3 * 8 + 5] = 0;
        let cover = HostCover { view, raster: [8, 6], hit: Arc::new(hit), owners: Arc::new(vec![2]), wall: 2, bands: OnceLock::new() };
        assert!(cover.hides(view.mm((5.5, 3.5))));
        assert!(!cover.hides(view.mm((5.5, 2.5))));
        assert!(!cover.hides(view.mm((9.5, 3.5))));
        let mut covered = Vec::new();
        cover.hide_edge(view.mm((4.0, 3.5)), view.mm((8.0, 3.5)), &mut covered);
        assert_eq!(covered, vec![(0.25, 0.5)]);
    }
}
