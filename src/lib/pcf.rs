use byteorder::{BigEndian, ByteOrder, LittleEndian};
use hashbrown::HashMap;

// From https://fontforge.org/docs/techref/pcf-format.html
// type field
const _PCF_PROPERTIES: u32 = 1 << 0;
const _PCF_ACCELERATORS: u32 = 1 << 1;
const _PCF_METRICS: u32 = 1 << 2;
const _PCF_BITMAPS: u32 = 1 << 3;
const _PCF_INK_METRICS: u32 = 1 << 4;
const _PCF_BDF_ENCODINGS: u32 = 1 << 5;
const _PCF_SWIDTHS: u32 = 1 << 6;
const _PCF_GLYPH_NAMES: u32 = 1 << 7;
const _PCF_BDF_ACCELERATORS: u32 = 1 << 8;

// format field
const _PCF_DEFAULT_FORMAT: u32 = 0x00000000;
const _PCF_INKBOUNDS: u32 = 0x00000200;
const _PCF_ACCEL_W_INKBOUNDS: u32 = 0x00000100;
const _PCF_COMPRESSED_METRICS: u32 = 0x00000100;

// format field modifiers
const _PCF_GLYPH_PAD_MASK: u32 = 3; // See the bitmap table for explanation
const _PCF_BYTE_MASK: u32 = 1 << 2; // If set then Most Sig Byte First
const _PCF_BIT_MASK: u32 = 1 << 3; // If set then Most Sig Bit First
const _PCF_SCAN_UNIT_MASK: u32 = 3 << 4; // See the bitmap table for explanation

#[derive(Debug, PartialEq)]
struct Table {
    format: u32,
    size: u32,
    offset: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Metrics {
    left_side_bearing: i16,
    right_side_bearing: i16,
    character_width: i16,
    character_ascent: i16,
    character_descent: i16,
    character_attributes: u16,
}

#[derive(Debug, Default, PartialEq)]
struct Accelerators {
    no_overlap: u8,
    constant_metrics: u8,
    terminal_font: u8,
    constant_width: u8,
    ink_inside: u8,
    ink_metrics: u8,
    draw_direction: u8,
    padding: u8,
    font_ascent: i32,
    font_descent: i32,
    max_overlap: i32,
    minbounds: Metrics,
    maxbounds: Metrics,
    ink_minbounds: Metrics,
    ink_maxbounds: Metrics,
}

#[derive(Debug, Default, PartialEq)]
struct Encoding {
    min_byte2: i16,
    max_byte2: i16,
    min_byte1: i16,
    max_byte1: i16,
    default_char: i16,
}

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

type Tables = HashMap<u32, Table>;

#[derive(Debug)]
pub struct Pcf<'a> {
    glyph_cache: GlyphCache,
    tables: Tables,
    bytes: &'a [u8],
    accelerators: Accelerators,
    encoding: Encoding,
}

impl Pcf<'_> {
    pub fn new(font: &[u8]) -> Pcf {
        let mut pcf = Pcf {
            bytes: font,
            glyph_cache: GlyphCache::new(),
            tables: HashMap::new(),
            accelerators: Default::default(),
            encoding: Default::default(),
        };

        let mut cursor = 8;
        for _ in 0..pcf.table_count() {
            let r#type = LittleEndian::read_u32(&font[cursor..cursor + 4]);
            let table = Table {
                format: LittleEndian::read_u32(&font[cursor + 4..cursor + 8]),
                size: LittleEndian::read_u32(&font[cursor + 8..cursor + 12]),
                offset: LittleEndian::read_u32(&font[cursor + 12..cursor + 16]),
            };
            pcf.tables.insert(r#type, table);
            cursor += 16;
        }

        pcf.accelerators = pcf.read_accelerators();
        pcf.encoding = pcf.read_encoding();

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

    fn tables(&self) -> &Tables {
        &self.tables
    }

    fn bitmap_format(&self) -> u32 {
        self.tables.get(&_PCF_BITMAPS).unwrap().format
    }

    fn read_accelerators(&self) -> Accelerators {
        let accelerators = self
            .tables
            .get(&_PCF_BDF_ACCELERATORS)
            .or_else(|| self.tables.get(&_PCF_ACCELERATORS));

        assert!(accelerators.is_some(), "No accelerator table found");

        let table = accelerators.unwrap();

        let mut cursor = table.offset as usize;
        let format = LittleEndian::read_u32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;

        assert!(format & _PCF_BYTE_MASK != 0, "Only big endian supported");

        let has_inkbounds = format & _PCF_ACCEL_W_INKBOUNDS;

        let no_overlap = self.bytes[cursor];
        let constant_metrics = self.bytes[cursor + 1];
        let terminal_font = self.bytes[cursor + 2];
        let constant_width = self.bytes[cursor + 3];
        let ink_inside = self.bytes[cursor + 4];
        let ink_metrics = self.bytes[cursor + 5];
        let draw_direction = self.bytes[cursor + 6];
        let padding = self.bytes[cursor + 7];
        cursor += 8;
        let font_ascent = BigEndian::read_i32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;
        let font_descent = BigEndian::read_i32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;
        let max_overlap = BigEndian::read_i32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;

        let minbounds = self.read_uncompressed_metrics(&mut cursor);
        let maxbounds = self.read_uncompressed_metrics(&mut cursor);
        let (ink_minbounds, ink_maxbounds) = if has_inkbounds != 0 {
            (
                self.read_uncompressed_metrics(&mut cursor),
                self.read_uncompressed_metrics(&mut cursor),
            )
        } else {
            (minbounds, maxbounds)
        };

        Accelerators {
            no_overlap,
            constant_metrics,
            terminal_font,
            constant_width,
            ink_inside,
            ink_metrics,
            draw_direction,
            padding,
            font_ascent,
            font_descent,
            max_overlap,
            minbounds,
            maxbounds,
            ink_minbounds,
            ink_maxbounds,
        }
    }

    fn read_uncompressed_metrics(&self, cursor: &mut usize) -> Metrics {
        let left_side_bearing = BigEndian::read_i16(&self.bytes[*cursor..(*cursor + 2)]);
        let right_side_bearing = BigEndian::read_i16(&self.bytes[(*cursor + 2)..(*cursor + 4)]);
        let character_width = BigEndian::read_i16(&self.bytes[(*cursor + 4)..(*cursor + 6)]);
        let character_ascent = BigEndian::read_i16(&self.bytes[(*cursor + 6)..(*cursor + 8)]);
        let character_descent = BigEndian::read_i16(&self.bytes[(*cursor + 8)..(*cursor + 10)]);
        let character_attributes = BigEndian::read_u16(&self.bytes[(*cursor + 10)..(*cursor + 12)]);

        *cursor += 12;

        Metrics {
            left_side_bearing,
            right_side_bearing,
            character_width,
            character_ascent,
            character_descent,
            character_attributes,
        }
    }

    #[allow(clippy::bad_bit_mask)]
    fn read_encoding(&self) -> Encoding {
        let encoding = self.tables.get(&_PCF_BDF_ENCODINGS);
        let table = encoding.expect("No encoding table found");

        let mut cursor = table.offset as usize;
        let format = LittleEndian::read_u32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;

        assert!(
            format & _PCF_DEFAULT_FORMAT == 0,
            "Encoding is not default format"
        );

        let min_byte2 = BigEndian::read_i16(&self.bytes[cursor..cursor + 2]);
        cursor += 2;
        let max_byte2 = BigEndian::read_i16(&self.bytes[cursor..cursor + 2]);
        cursor += 2;
        let min_byte1 = BigEndian::read_i16(&self.bytes[cursor..cursor + 2]);
        cursor += 2;
        let max_byte1 = BigEndian::read_i16(&self.bytes[cursor..cursor + 2]);
        cursor += 2;
        let default_char = BigEndian::read_i16(&self.bytes[cursor..cursor + 2]);

        Encoding {
            min_byte2,
            max_byte2,
            min_byte1,
            max_byte1,
            default_char,
        }
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
    fn it_parses_tables() {
        let table_1 = Table {
            format: 14,
            size: 1264,
            offset: 136,
        };

        let table_2 = Table {
            format: 14,
            size: 100,
            offset: 1400,
        };

        let table_4 = Table {
            format: 270,
            size: 492,
            offset: 1500,
        };

        let table_8 = Table {
            format: 14,
            size: 3400,
            offset: 1992,
        };

        let table_32 = Table {
            format: 14,
            size: 268,
            offset: 5392,
        };

        let table_64 = Table {
            format: 14,
            size: 396,
            offset: 5660,
        };

        let table_128 = Table {
            format: 14,
            size: 840,
            offset: 6056,
        };

        let table_256 = Table {
            format: 14,
            size: 100,
            offset: 6896,
        };

        let mut tables = HashMap::new();
        tables.insert(1, table_1);
        tables.insert(2, table_2);
        tables.insert(4, table_4);
        tables.insert(8, table_8);
        tables.insert(32, table_32);
        tables.insert(64, table_64);
        tables.insert(128, table_128);
        tables.insert(256, table_256);

        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = Pcf::new(&font[..]);
        assert_eq!(tables, *pcf.tables());
    }

    #[test]
    fn it_parses_accelerators_correctly() {
        let accelerators = Accelerators {
            no_overlap: 0,
            constant_metrics: 0,
            terminal_font: 0,
            constant_width: 0,
            ink_inside: 0,
            ink_metrics: 0,
            draw_direction: 0,
            padding: 0,
            font_ascent: 10,
            font_descent: 2,
            max_overlap: 1,
            minbounds: Metrics {
                left_side_bearing: -1,
                right_side_bearing: 1,
                character_width: 0,
                character_ascent: -1,
                character_descent: -7,
                character_attributes: 0,
            },
            maxbounds: Metrics {
                left_side_bearing: 3,
                right_side_bearing: 11,
                character_width: 11,
                character_ascent: 9,
                character_descent: 3,
                character_attributes: 0,
            },
            ink_minbounds: Metrics {
                left_side_bearing: -1,
                right_side_bearing: 1,
                character_width: 0,
                character_ascent: -1,
                character_descent: -7,
                character_attributes: 0,
            },
            ink_maxbounds: Metrics {
                left_side_bearing: 3,
                right_side_bearing: 11,
                character_width: 11,
                character_ascent: 9,
                character_descent: 3,
                character_attributes: 0,
            },
        };

        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = Pcf::new(&font[..]);
        assert_eq!(accelerators, pcf.accelerators);
    }
    #[test]
    fn it_parses_encoding_correctly() {
        let encoding = Encoding {
            min_byte2: 0,
            max_byte2: 126,
            min_byte1: 0,
            max_byte1: 0,
            default_char: 1,
        };

        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = Pcf::new(&font[..]);
        assert_eq!(encoding, pcf.encoding);
    }

    #[test]
    fn it_parses_bitmap_format() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = Pcf::new(&font[..]);
        assert_eq!(0xE, pcf.bitmap_format());
    }
}
