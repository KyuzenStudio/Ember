use std::ops::{Add, Sub};

#[derive(Copy, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const fn zero() -> Self {
        Self::new(0.0, 0.0)
    }

    pub fn scale(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s)
    }
}

impl Add for Vec2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_combines_components() {
        assert_eq!(
            Vec2::new(1.0, 2.0) + Vec2::new(3.0, 4.0),
            Vec2::new(4.0, 6.0)
        );
    }

    #[test]
    fn sub_combines_components() {
        assert_eq!(
            Vec2::new(5.0, 7.0) - Vec2::new(2.0, 3.0),
            Vec2::new(3.0, 4.0)
        );
    }

    #[test]
    fn scale_multiplies_components() {
        assert_eq!(Vec2::new(2.0, -3.0).scale(2.5), Vec2::new(5.0, -7.5));
    }

    #[test]
    fn zero_is_origin() {
        assert_eq!(Vec2::zero(), Vec2::new(0.0, 0.0));
    }
}
