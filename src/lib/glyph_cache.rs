use std::collections::HashMap;

use super::glyph::Glyph;

#[derive(Debug, Default)]
pub struct GlyphCache {
    pub glyphs: HashMap<i32, Glyph>,
}

impl GlyphCache {
    pub fn new() -> Self {
        Self {
            glyphs: HashMap::new(),
        }
    }

    pub fn contains(&self, code_point: &i32) -> bool {
        self.glyphs.contains_key(code_point)
    }

    fn load_glyphs(self, _code_points: i32) {}

    fn get_glyph(self, _code_point: i32) -> i32 {
        1
    }
}
