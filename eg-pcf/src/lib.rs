#![no_std]

use embedded_graphics::{
    iterator::raw::RawDataSlice,
    pixelcolor::raw::{LittleEndian, RawU1},
    prelude::*,
    primitives::Rectangle,
};

pub use eg_pcf_macros::include_pcf;

pub mod text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PcfFont<'a> {
    pub bounding_box: Rectangle,
    pub replacement_character: usize,
    pub line_height: u32,
    pub glyphs: &'a [PcfGlyph],
    pub data: &'a [u8],
}

impl<'a> PcfFont<'a> {
    fn get_glyph(&self, c: char) -> &'a PcfGlyph {
        self.glyphs
            .iter()
            .find(|g| g.character == c)
            .unwrap_or_else(|| &self.glyphs[self.replacement_character])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PcfGlyph {
    pub character: char,
    pub bounding_box: Rectangle,
    pub device_width: u32,
    pub start_index: usize,
}

impl PcfGlyph {
    fn draw<D: DrawTarget>(
        &self,
        position: Point,
        color: D::Color,
        data: &[u8],
        target: &mut D,
    ) -> Result<(), D::Error> {
        let mut data_iter = RawDataSlice::<RawU1, LittleEndian>::new(data).into_iter();

        if self.start_index > 0 {
            data_iter.nth(self.start_index - 1);
        }

        self.bounding_box
            .translate(position)
            .points()
            .zip(data_iter)
            .filter(|(_p, c)| *c == RawU1::new(1))
            .map(|(p, _c)| Pixel(p, color))
            .draw(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let font = include_pcf!("examples/OpenSans-Regular-12.pcf", 'A'..='B');
        assert!(font.line_height == 12);
    }
}
