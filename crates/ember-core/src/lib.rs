use std::fmt;
use std::ops::{Add, Sub};

pub mod host;

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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    I32,
    F32,
    Bool,
    Str,
    Void,
}

impl Ty {
    pub fn name(self) -> &'static str {
        match self {
            Ty::I32 => "i32",
            Ty::F32 => "f32",
            Ty::Bool => "bool",
            Ty::Str => "string",
            Ty::Void => "void",
        }
    }

    pub fn from_name(name: &str) -> Option<Ty> {
        match name {
            "i32" => Some(Ty::I32),
            "f32" => Some(Ty::F32),
            "bool" => Some(Ty::Bool),
            "string" => Some(Ty::Str),
            "void" => Some(Ty::Void),
            _ => None,
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
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
