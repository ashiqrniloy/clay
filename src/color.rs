//! Renderer-neutral RGBA color used by theme validation and frontend projection.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Color {
    rgba: [u8; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self::from_rgba8(0, 0, 0, 0);

    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba8(r, g, b, u8::MAX)
    }

    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { rgba: [r, g, b, a] }
    }

    pub const fn to_rgba8(self) -> Rgba8 {
        Rgba8 {
            r: self.rgba[0],
            g: self.rgba[1],
            b: self.rgba[2],
            a: self.rgba[3],
        }
    }

    pub const fn components(self) -> [u8; 4] {
        self.rgba
    }

    pub fn split(self) -> ([u8; 3], f32) {
        (
            [self.rgba[0], self.rgba[1], self.rgba[2]],
            f32::from(self.rgba[3]) / 255.0,
        )
    }
}
