/// Logical pixel scalar.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Px(pub f32);

impl Px {
    pub const ZERO: Self = Self(0.0);

    pub const fn new(value: f32) -> Self {
        Self(value)
    }
}

/// A point in an area-local coordinate system.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn translate(self, v: Vector) -> Self {
        Self {
            x: self.x + v.x,
            y: self.y + v.y,
        }
    }
}

/// A vector in an area-local coordinate system.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector {
    pub x: f32,
    pub y: f32,
}

impl Vector {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A two-dimensional size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// Axis-aligned rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}

impl Rect {
    pub const EMPTY: Self = Self {
        min: Point::ZERO,
        max: Point::ZERO,
    };

    pub fn from_min_size(min: Point, size: Size) -> Self {
        Self {
            min,
            max: Point {
                x: min.x + size.width,
                y: min.y + size.height,
            },
        }
    }

    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn contains(self, point: Point) -> bool {
        self.min.x <= point.x
            && point.x <= self.max.x
            && self.min.y <= point.y
            && point.y <= self.max.y
    }

    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            min: Point::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Point::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }
}

/// 2D affine transform in `[a, b, c, d, e, f]` form.
///
/// It maps `(x, y)` to `(a*x + c*y + e, b*x + d*y + f)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine2 {
    pub m: [f32; 6],
}

impl Affine2 {
    pub const IDENTITY: Self = Self {
        m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    };

    pub const fn new(m: [f32; 6]) -> Self {
        Self { m }
    }

    pub const fn translation(v: Vector) -> Self {
        Self {
            m: [1.0, 0.0, 0.0, 1.0, v.x, v.y],
        }
    }

    pub const fn rotate_90_cw() -> Self {
        Self {
            m: [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
        }
    }

    pub const fn rotate_90_ccw() -> Self {
        Self {
            m: [0.0, -1.0, 1.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn transform_point(self, p: Point) -> Point {
        let [a, b, c, d, e, f] = self.m;
        Point::new(a.mul_add(p.x, c.mul_add(p.y, e)), b.mul_add(p.x, d.mul_add(p.y, f)))
    }

    pub fn then(self, next: Self) -> Self {
        let [a0, b0, c0, d0, e0, f0] = self.m;
        let [a1, b1, c1, d1, e1, f1] = next.m;
        Self::new([
            a1 * a0 + c1 * b0,
            b1 * a0 + d1 * b0,
            a1 * c0 + c1 * d0,
            b1 * c0 + d1 * d0,
            a1 * e0 + c1 * f0 + e1,
            b1 * e0 + d1 * f0 + f1,
        ])
    }

    pub fn transform_rect_aabb(self, rect: Rect) -> Rect {
        let p0 = self.transform_point(rect.min);
        let p1 = self.transform_point(Point::new(rect.max.x, rect.min.y));
        let p2 = self.transform_point(rect.max);
        let p3 = self.transform_point(Point::new(rect.min.x, rect.max.y));
        [p1, p2, p3].into_iter().fold(Rect { min: p0, max: p0 }, |acc, p| {
            acc.union(Rect { min: p, max: p })
        })
    }
}

impl Default for Affine2 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::{Affine2, Point, Rect, Size};

    #[test]
    fn rotate_90_clockwise_maps_y_axis_to_negative_x() {
        let actual = Affine2::rotate_90_cw().transform_point(Point::new(0.0, 2.0));
        assert_eq!(actual, Point::new(-2.0, 0.0));
    }

    #[test]
    fn transformed_rect_uses_aabb() {
        let rect = Rect::from_min_size(Point::new(0.0, 0.0), Size::new(2.0, 3.0));
        let actual = Affine2::rotate_90_cw().transform_rect_aabb(rect);
        assert_eq!(actual.min, Point::new(-3.0, 0.0));
        assert_eq!(actual.max, Point::new(0.0, 2.0));
    }
}
