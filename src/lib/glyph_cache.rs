use hashbrown::HashMap;

use super::glyph::Glyph;

#[derive(Debug, Default)]
pub struct GlyphCache {
    glyphs: HashMap<i32, Glyph>,
}

impl GlyphCache {
    pub fn new() -> Self {
        Self {
            glyphs: HashMap::new(),
        }
    }

    fn load_glyphs(self, code_points: i32) {}

    fn get_glyph(self, code_point: i32) -> i32 {
        1
    }
}
