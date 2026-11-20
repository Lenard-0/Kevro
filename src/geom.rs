//! Plain geometry types shared by the pure modules.
//!
//! All `Rect`s in Kevro use the **AX / CoreGraphics global coordinate space**:
//! origin at the top-left of the primary display, y increasing downwards.
//! Conversion from AppKit (bottom-left origin) happens once, in `screens.rs`.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Rect { x, y, w, h }
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    /// Half-open containment: a point on the right/bottom edge belongs to the
    /// next rect, so adjacent screens never both claim a point.
    pub fn contains(&self, (px, py): (f64, f64)) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    pub fn intersection_area(&self, other: &Rect) -> f64 {
        let w = self.right().min(other.right()) - self.x.max(other.x);
        let h = self.bottom().min(other.bottom()) - self.y.max(other.y);
        if w > 0.0 && h > 0.0 { w * h } else { 0.0 }
    }

    /// True if every edge is within `tolerance` points of `other`'s.
    pub fn approx_eq(&self, other: &Rect, tolerance: f64) -> bool {
        (self.x - other.x).abs() <= tolerance
            && (self.y - other.y).abs() <= tolerance
            && (self.right() - other.right()).abs() <= tolerance
            && (self.bottom() - other.bottom()).abs() <= tolerance
    }

    pub fn rounded(&self) -> Rect {
        let x = self.x.round();
        let y = self.y.round();
        Rect { x, y, w: (self.right().round() - x).max(1.0), h: (self.bottom().round() - y).max(1.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_is_half_open() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        assert!(r.contains((0.0, 0.0)));
        assert!(r.contains((99.9, 49.9)));
        assert!(!r.contains((100.0, 10.0)));
        assert!(!r.contains((10.0, 50.0)));
    }

    #[test]
    fn intersection() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(a.intersection_area(&Rect::new(50.0, 50.0, 100.0, 100.0)), 2500.0);
        assert_eq!(a.intersection_area(&Rect::new(100.0, 0.0, 10.0, 10.0)), 0.0);
    }

    #[test]
    fn rounding_keeps_edges() {
        let r = Rect::new(10.4, 20.6, 100.2, 50.5).rounded();
        assert_eq!(r, Rect::new(10.0, 21.0, 101.0, 50.0));
    }
}
