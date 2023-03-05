#![no_std]

use byteorder::{ByteOrder, LittleEndian};
use hashbrown::HashMap;

// From https://fontforge.org/docs/techref/pcf-format.html
// type field
const _PCF_PROPERTIES: i32 = 1 << 0;
const _PCF_ACCELERATORS: i32 = 1 << 1;
const _PCF_METRICS: i32 = 1 << 2;
const _PCF_BITMAPS: i32 = 1 << 3;
const _PCF_INK_METRICS: i32 = 1 << 4;
const _PCF_BDF_ENCODINGS: i32 = 1 << 5;
const _PCF_SWIDTHS: i32 = 1 << 6;
const _PCF_GLYPH_NAMES: i32 = 1 << 7;
const _PCF_BDF_ACCELERATORS: i32 = 1 << 8;

// format field
const _PCF_DEFAULT_FORMAT: i32 = 0x00000000;
const _PCF_INKBOUNDS: i32 = 0x00000200;
const _PCF_ACCEL_W_INKBOUNDS: i32 = 0x00000100;
const _PCF_COMPRESSED_METRICS: i32 = 0x00000100;

// format field modifiers
const _PCF_GLYPH_PAD_MASK: i32 = 3; // See the bitmap table for explanation
const _PCF_BYTE_MASK: i32 = 1 << 2; // If set then Most Sig Byte First
const _PCF_BIT_MASK: i32 = 1 << 3; // If set then Most Sig Bit First
const _PCF_SCAN_UNIT_MASK: i32 = 3 << 4; // See the bitmap table for explanation

#[derive(Debug)]
struct Table {
    format: i32,
    size: i32,
    offset: i32,
}

//#[derive(Debug)]
//struct Metrics {
//    left_side_bearing: bool,
//    right_side_bearing: bool,
//    character_width: bool,
//    character_ascent: bool,
//    character_descent: bool,
//    character_attributes: bool,
//}
//
//#[derive(Debug)]
//struct Accelerators {
//    no_overlap: bool,
//    constant_metrics: bool,
//    terminal_font: bool,
//    constant_width: bool,
//    ink_inside: bool,
//    ink_metrics: bool,
//    draw_direction: bool,
//    font_ascent: bool,
//    font_descent: bool,
//    max_overlap: bool,
//    minbounds: bool,
//    maxbounds: bool,
//    ink_minbounds: bool,
//    ink_maxbounds: bool,
//}
//
//#[derive(Debug)]
//struct Encoding {
//    min_byte2: bool,
//    max_byte2: bool,
//    min_byte1: bool,
//    max_byte1: bool,
//    default_char: bool,
//}
//
//#[derive(Debug)]
//struct Bitmap {
//    glyph_count: bool,
//    bitmap_sizes: bool,
//}
//
//#[derive(Debug)]
//struct Glyph {
//    bitmap: bool,
//    width: bool,
//    height: bool,
//    dx: bool,
//    dy: bool,
//    shift_x: bool,
//    shift_y: bool,
//    tile_index: bool,
//}

#[derive(Debug)]
struct GlyphCache {
    glyphs: HashMap<i32, i32>,
}

impl GlyphCache {
    fn new() -> Self {
        Self {
            glyphs: HashMap::new(),
        }
    }

    fn load_glyphs(self, code_points: i32) {}

    fn get_glyphs(self, code_point: i32) -> i32 {
        1
    }
}

#[derive(Debug)]
pub struct Pcf<'a> {
    glyph_cache: GlyphCache,
    tables: HashMap<i32, Table>,
    bytes: &'a [u8],
}

impl Pcf<'_> {
    pub fn new(font: &[u8]) -> Pcf {
        let mut pcf = Pcf {
            bytes: font,
            glyph_cache: GlyphCache::new(),
            tables: HashMap::new(),
        };

        let mut cursor = 8;
        for _ in 0..pcf.table_count() {
            let r#type = LittleEndian::read_i32(&font[cursor..cursor + 4]);
            let table = Table {
                format: LittleEndian::read_i32(&font[cursor + 4..cursor + 8]),
                size: LittleEndian::read_i32(&font[cursor + 8..cursor + 12]),
                offset: LittleEndian::read_i32(&font[cursor + 12..cursor + 16]),
            };
            pcf.tables.insert(r#type, table);
            cursor += 16;
        }

        pcf
    }

    // 1, 102, 99, 112
    // 1885562369 lsbi32
    fn header(&self) -> i32 {
        LittleEndian::read_i32(&self.bytes[0..4])
    }

    fn table_count(&self) -> i32 {
        LittleEndian::read_i32(&self.bytes[4..8])
    }

    fn bitmap_format(&self) -> i32 {
        self.tables.get(&_PCF_BITMAPS).unwrap().format
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_parses_header() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = Pcf::new(&font[..]);
        assert_eq!(1885562369, pcf.header());
    }

    #[test]
    fn it_parses_table_count() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = Pcf::new(&font[..]);
        assert_eq!(8, pcf.table_count());
    }

    #[test]
    fn it_parses_bitmap_format() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = Pcf::new(&font[..]);
        assert_eq!(0xE, pcf.bitmap_format());
    }
}
