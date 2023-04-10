#![allow(dead_code)]
use byteorder::{BigEndian, ByteOrder, LittleEndian, ReadBytesExt};
use std::{
    collections::HashMap,
    io::{Cursor, Seek, SeekFrom},
};

// From https://fontforge.org/docs/techref/pcf-format.html
// type field
const PCF_PROPERTIES: usize = 1 << 0;
const PCF_ACCELERATORS: usize = 1 << 1;
const PCF_METRICS: usize = 1 << 2;
const PCF_BITMAPS: usize = 1 << 3;
const PCF_INK_METRICS: usize = 1 << 4;
const PCF_BDF_ENCODINGS: usize = 1 << 5;
const PCF_SWIDTHS: usize = 1 << 6;
const PCF_GLYPH_NAMES: usize = 1 << 7;
const PCF_BDF_ACCELERATORS: usize = 1 << 8;

// format field
const PCF_DEFAULT_FORMAT: i32 = 0x00000000;
const PCF_INKBOUNDS: i32 = 0x00000200;
const PCF_ACCEL_W_INKBOUNDS: i32 = 0x00000100;
const PCF_COMPRESSED_METRICS: i32 = 0x00000100;

// format field modifiers
const PCF_GLYPH_PAD_MASK: i32 = 3; // See the bitmap table for explanation
const PCF_BYTE_MASK: i32 = 1 << 2; // If set then Most Sig Byte First
const PCF_BIT_MASK: i32 = 1 << 3; // If set then Most Sig Bit First
const PCF_SCAN_UNIT_MASK: i32 = 3 << 4; // See the bitmap table for explanation

#[derive(Debug, PartialEq)]
struct Table {
    format: i32,
    size: i32,
    offset: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct UncompressedMetrics {
    left_side_bearing: i16,
    right_side_bearing: i16,
    character_width: i16,
    character_ascent: i16,
    character_descent: i16,
    character_attributes: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CompressedMetrics {
    left_side_bearing: i16,
    right_side_bearing: i16,
    character_width: i16,
    character_ascent: i16,
    character_descent: i16,
    character_attributes: i16,
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
    minbounds: UncompressedMetrics,
    maxbounds: UncompressedMetrics,
    ink_minbounds: UncompressedMetrics,
    ink_maxbounds: UncompressedMetrics,
}

#[derive(Debug, Default, PartialEq)]
struct Encoding {
    min_byte2: usize,
    max_byte2: usize,
    min_byte1: usize,
    max_byte1: usize,
    default_char: usize,
}

#[derive(Debug, Default, PartialEq)]
struct Bitmap {
    glyph_count: usize,
    bitmap_sizes: usize,
}

#[derive(Debug, Default, PartialEq)]
pub struct BoundingBox {
    pub size: Coord,
    pub offset: Coord,
}

#[derive(Debug, Default, PartialEq)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

impl Coord {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

type Tables = HashMap<usize, Table>;

#[derive(Debug, Default)]
pub struct PcfFont<'a> {
    pub glyphs: HashMap<i32, Glyph>,
    tables: Tables,
    bytes: Cursor<&'a [u8]>,
    accelerators: Accelerators,
    encoding: Encoding,
    bitmap: Bitmap,
    pub bounding_box: BoundingBox,
    metadata: Metadata,
}

#[derive(Debug, Default, PartialEq)]
struct Metadata {
    indices_offset: usize,
    bitmap_offset_offsets: usize,
    first_bitmap_offset: usize,
    metrics_compressed_raw: i32,
    is_metrics_compressed: bool,
    first_metric_offset: usize,
    metrics_size: usize,
}

#[derive(Debug, PartialEq)]
pub struct Glyph {
    pub code_point: i32,
    pub encoding: Option<char>,
    pub bitmap: Vec<u8>,
    pub bounding_box: BoundingBox,
    pub shift_x: i32,
    pub shift_y: i32,
    pub tile_index: i32,
}

impl Glyph {
    pub fn pixel(&self, x: usize, y: usize) -> bool {
        let width = usize::try_from(self.bounding_box.size.x).expect("pixel width failed");
        self.bitmap[y * width + x] != 0
    }
}

impl PcfFont<'_> {
    pub fn new(font: &[u8]) -> PcfFont {
        let mut pcf = PcfFont {
            bytes: Cursor::new(font),
            ..Default::default()
        };

        pcf.header(); // TODO maybe panic if magic string is not there?
        pcf.tables = pcf.read_tables();
        pcf.accelerators = pcf.read_accelerators();
        pcf.encoding = pcf.read_encoding();
        pcf.bitmap = pcf.read_bitmap();
        pcf.bounding_box = pcf.get_bounding_box();
        pcf.metadata = pcf.load_metadata();
        pcf.load_glyphs();

        pcf
    }

    // "1fcp"
    // 1, 102, 99, 112
    // 1885562369 lsbi32
    fn header(&self) -> i32 {
        self.bytes.read_i32::<LittleEndian>().unwrap()
    }

    fn table_count(&self) -> i32 {
        // test assumes header was called
        self.bytes.read_i32::<LittleEndian>().unwrap()
    }

    fn tables(&self) -> &Tables {
        &self.tables
    }

    fn read_tables(&self) -> HashMap<usize, Table> {
        // assumes header was called (since table_count assumes that)
        // TODO: this can be a map I think now.
        (0..self.table_count()).fold(HashMap::new(), |mut tables, _| {
            let r#type = self
                .bytes
                .read_i32::<LittleEndian>()
                .unwrap()
                .try_into()
                .expect("unable to convert type i32 into usize");
            let format = self.bytes.read_i32::<LittleEndian>().unwrap();
            let size = self.bytes.read_i32::<LittleEndian>().unwrap();
            let offset = self
                .bytes
                .read_i32::<LittleEndian>()
                .unwrap()
                .try_into()
                .expect("unable to convert offset i32 into usize");

            let table = Table {
                format,
                size,
                offset,
            };

            tables.insert(r#type, table);

            tables
        })
    }

    fn read_accelerators(&self) -> Accelerators {
        let accelerators = self
            .tables
            .get(&PCF_BDF_ACCELERATORS)
            .or_else(|| self.tables.get(&PCF_ACCELERATORS));

        assert!(accelerators.is_some(), "No accelerator table found");

        let table = accelerators.unwrap();

        let mut cursor = table.offset;
        let format = LittleEndian::read_i32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;

        assert!(format & PCF_BYTE_MASK != 0, "Only big endian supported");

        let has_inkbounds = format & PCF_ACCEL_W_INKBOUNDS;

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

    fn read_uncompressed_metrics(&self, cursor: &mut usize) -> UncompressedMetrics {
        let left_side_bearing = BigEndian::read_i16(&self.bytes[*cursor..(*cursor + 2)]);
        let right_side_bearing = BigEndian::read_i16(&self.bytes[(*cursor + 2)..(*cursor + 4)]);
        let character_width = BigEndian::read_i16(&self.bytes[(*cursor + 4)..(*cursor + 6)]);
        let character_ascent = BigEndian::read_i16(&self.bytes[(*cursor + 6)..(*cursor + 8)]);
        let character_descent = BigEndian::read_i16(&self.bytes[(*cursor + 8)..(*cursor + 10)]);
        let character_attributes = BigEndian::read_u16(&self.bytes[(*cursor + 10)..(*cursor + 12)]);

        *cursor += 12;

        UncompressedMetrics {
            left_side_bearing,
            right_side_bearing,
            character_width,
            character_ascent,
            character_descent,
            character_attributes,
        }
    }

    fn read_compressed_metrics(&self, cursor: usize) -> CompressedMetrics {
        let left_side_bearing: i16 = self.bytes[cursor].into();
        let right_side_bearing: i16 = self.bytes[cursor + 1].into();
        let character_width: i16 = self.bytes[cursor + 2].into();
        let character_ascent: i16 = self.bytes[cursor + 3].into();
        let character_descent: i16 = self.bytes[cursor + 4].into();

        CompressedMetrics {
            left_side_bearing: left_side_bearing - 0x80,
            right_side_bearing: right_side_bearing - 0x80,
            character_width: character_width - 0x80,
            character_ascent: character_ascent - 0x80,
            character_descent: character_descent - 0x80,
            character_attributes: 0,
        }
    }

    #[allow(clippy::bad_bit_mask)]
    fn read_encoding(&self) -> Encoding {
        let encoding = self.tables.get(&PCF_BDF_ENCODINGS);
        let table = encoding.expect("No encoding table found");

        let mut cursor = table.offset;
        let format = LittleEndian::read_i32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;

        assert!(
            format & PCF_DEFAULT_FORMAT == 0,
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
            min_byte2: min_byte2.try_into().unwrap(),
            max_byte2: max_byte2.try_into().unwrap(),
            min_byte1: min_byte1.try_into().unwrap(),
            max_byte1: max_byte1.try_into().unwrap(),
            default_char: default_char.try_into().unwrap(),
        }
    }

    #[allow(clippy::bad_bit_mask)]
    fn read_bitmap(&self) -> Bitmap {
        let bitmap = self.tables.get(&PCF_BITMAPS);
        let table = bitmap.expect("No bitmap table found");

        let mut cursor = table.offset;
        let format = LittleEndian::read_i32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;

        assert!(
            format & PCF_DEFAULT_FORMAT == 0,
            "Bitmap is not default format"
        );

        let glyph_count = BigEndian::read_i32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;
        cursor += (4 * glyph_count) as usize;

        let one = BigEndian::read_i32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;
        let two = BigEndian::read_i32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;
        let three = BigEndian::read_i32(&self.bytes[cursor..cursor + 4]);
        cursor += 4;
        let four = BigEndian::read_i32(&self.bytes[cursor..cursor + 4]);

        let bitmap_sizes = [one, two, three, four][format as usize & 3];

        Bitmap {
            glyph_count: glyph_count.try_into().unwrap(),
            bitmap_sizes: bitmap_sizes.try_into().unwrap(),
        }
    }

    fn get_bounding_box(&self) -> BoundingBox {
        let minbounds = self.accelerators.ink_minbounds;
        let maxbounds = self.accelerators.ink_maxbounds;
        let width = maxbounds.right_side_bearing - minbounds.left_side_bearing;
        let height = maxbounds.character_ascent + maxbounds.character_descent;

        BoundingBox {
            size: Coord::new(width.into(), height.into()),
            offset: Coord::new(
                minbounds.left_side_bearing.into(),
                (-maxbounds.character_descent).into(),
            ),
        }
    }

    fn load_metadata(&self) -> Metadata {
        let indices_offset = self.tables[&PCF_BDF_ENCODINGS].offset + 14;
        let bitmap_offset_offsets = self.tables[&PCF_BITMAPS].offset + 8;
        let first_bitmap_offset =
            self.tables[&PCF_BITMAPS].offset + 4 * (6 + self.bitmap.glyph_count);
        let metrics_compressed_raw = self.tables[&PCF_METRICS].format & PCF_COMPRESSED_METRICS;
        let is_metrics_compressed = metrics_compressed_raw != 0;
        let first_metric_offset =
            self.tables[&PCF_METRICS].offset + (if is_metrics_compressed { 6 } else { 8 });
        let metrics_size = if is_metrics_compressed { 5 } else { 12 };

        Metadata {
            indices_offset,
            bitmap_offset_offsets,
            first_bitmap_offset,
            metrics_compressed_raw,
            is_metrics_compressed,
            first_metric_offset,
            metrics_size,
        }
    }

    fn load_glyphs(&mut self) {
        let indices = self.load_glyph_indices();

        if !self.metadata.is_metrics_compressed {
            panic!("uncompressed metrics unimplemented");
        }

        let all_metrics = self.load_all_metrics(&indices);
        let bitmap_offsets = self.load_bitmap_offsets(&indices);
        let glyphs = self.create_glyphs(&all_metrics);
        self.glyphs = self.fill_glyph_bitmaps(glyphs, &bitmap_offsets);
    }

    fn load_glyph_indices(&self) -> HashMap<i32, usize> {
        (0..=(u16::MAX as i32))
            .filter_map(|code_point| {
                let enc1 = ((code_point >> 8) & 0xFF) as usize;
                let enc2 = (code_point & 0xFF) as usize;

                if enc1 < self.encoding.min_byte1 || enc1 > self.encoding.max_byte1 {
                    return None;
                }

                if enc2 < self.encoding.min_byte2 || enc2 > self.encoding.max_byte2 {
                    return None;
                }

                let encoding_idx = (enc1 - self.encoding.min_byte1)
                    * (self.encoding.max_byte2 - self.encoding.min_byte2 + 1)
                    + enc2
                    - self.encoding.min_byte2;

                let cursor: usize = self.metadata.indices_offset + 2 * encoding_idx;
                let glyph_idx: usize = BigEndian::read_u16(&self.bytes[cursor..cursor + 2]).into();
                if glyph_idx != 65535 {
                    Some((code_point, glyph_idx))
                } else {
                    None
                }
            })
            .collect()
    }

    fn load_all_metrics(&self, indices: &HashMap<i32, usize>) -> HashMap<i32, CompressedMetrics> {
        indices
            .iter()
            .map(|(code_point, index)| {
                let cursor: usize =
                    self.metadata.first_metric_offset + self.metadata.metrics_size * index;
                let metrics = self.read_compressed_metrics(cursor);

                (*code_point, metrics)
            })
            .collect()
    }

    fn load_bitmap_offsets(&self, indices: &HashMap<i32, usize>) -> HashMap<i32, usize> {
        indices
            .iter()
            .map(|(code_point, index)| {
                let cursor: usize = self.metadata.bitmap_offset_offsets + 4 * index;
                let bitmap_offset: usize = BigEndian::read_u32(&self.bytes[cursor..cursor + 4])
                    .try_into()
                    .unwrap();

                (*code_point, bitmap_offset)
            })
            .collect()
    }

    fn create_glyphs(&self, all_metrics: &HashMap<i32, CompressedMetrics>) -> HashMap<i32, Glyph> {
        all_metrics
            .iter()
            .map(|(code_point, metrics)| {
                let width: i32 = (metrics.right_side_bearing - metrics.left_side_bearing)
                    .try_into()
                    .unwrap();
                let height: i32 = (metrics.character_ascent + metrics.character_descent)
                    .try_into()
                    .unwrap();
                let len = (width * height).try_into().expect("width * height failed");
                let bitmap = vec![0u8; len];
                let encoding = u32::try_from(*code_point)
                    .ok()
                    .and_then(std::char::from_u32);

                let glyph = Glyph {
                    bitmap,
                    code_point: *code_point,
                    encoding,
                    bounding_box: BoundingBox {
                        size: Coord::new(width, height),
                        offset: Coord::new(
                            metrics.left_side_bearing as i32,
                            -(metrics.character_descent as i32),
                        ),
                    },
                    shift_x: metrics.character_width as i32,
                    shift_y: 0,
                    tile_index: 0,
                };

                (*code_point, glyph)
            })
            .collect()
    }

    fn fill_glyph_bitmaps(
        &self,
        glyphs: HashMap<i32, Glyph>,
        bitmap_offsets: &HashMap<i32, usize>,
    ) -> HashMap<i32, Glyph> {
        glyphs
            .into_iter()
            .map(|(code_point, mut glyph)| {
                let offset = self.metadata.first_bitmap_offset + bitmap_offsets[&code_point];
                let width = glyph.bounding_box.size.x as usize;
                let height = glyph.bounding_box.size.y as usize;
                let words_per_row = (width + 31) / 32;
                let bytes_per_row = 4 * words_per_row;
                for y in 0..height {
                    let start = offset + bytes_per_row * y;
                    let end = start + bytes_per_row;
                    let row = &self.bytes[start..end];
                    for x in 0..width {
                        let idx = x / 8;
                        let byte = row[idx];
                        let mask = 128 >> (x % 8);
                        let masked = byte & mask;
                        let on = masked != 0;

                        if on {
                            glyph.bitmap[y * width + x] = 1;
                        }
                    }
                }

                (code_point, glyph)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UPPERCASE_A: i32 = 65;
    const UPPERCASE_J: i32 = 74;
    const UPPERCASE_W: i32 = 87;

    #[test]
    fn it_parses_header() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        assert_eq!(1885562369, pcf.header());
    }

    #[test]
    fn it_parses_table_count() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
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
        let pcf = PcfFont::new(&font[..]);
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
            minbounds: UncompressedMetrics {
                left_side_bearing: -1,
                right_side_bearing: 1,
                character_width: 0,
                character_ascent: -1,
                character_descent: -7,
                character_attributes: 0,
            },
            maxbounds: UncompressedMetrics {
                left_side_bearing: 3,
                right_side_bearing: 11,
                character_width: 11,
                character_ascent: 9,
                character_descent: 3,
                character_attributes: 0,
            },
            ink_minbounds: UncompressedMetrics {
                left_side_bearing: -1,
                right_side_bearing: 1,
                character_width: 0,
                character_ascent: -1,
                character_descent: -7,
                character_attributes: 0,
            },
            ink_maxbounds: UncompressedMetrics {
                left_side_bearing: 3,
                right_side_bearing: 11,
                character_width: 11,
                character_ascent: 9,
                character_descent: 3,
                character_attributes: 0,
            },
        };

        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
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
        let pcf = PcfFont::new(&font[..]);
        assert_eq!(encoding, pcf.encoding);
    }

    #[test]
    fn it_parses_bitmap_correctly() {
        let bitmap = Bitmap {
            glyph_count: 97,
            bitmap_sizes: 2988,
        };

        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        assert_eq!(bitmap, pcf.bitmap);
    }

    #[test]
    fn it_has_a_bounding_box() {
        let bounding_box = BoundingBox {
            size: Coord::new(12, 12),
            offset: Coord::new(-1, -3),
        };

        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        assert_eq!(bounding_box, pcf.bounding_box);
    }

    #[test]
    fn it_loads_metadata() {
        let metadata = Metadata {
            indices_offset: 5406,
            bitmap_offset_offsets: 2000,
            first_bitmap_offset: 2404,
            metrics_compressed_raw: 256,
            is_metrics_compressed: true,
            first_metric_offset: 1506,
            metrics_size: 5,
        };

        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);

        assert_eq!(metadata, pcf.metadata);
    }

    #[test]
    fn it_loads_indices_for_uppercase_a() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        assert_eq!(35, pcf.load_glyph_indices()[&UPPERCASE_A]);
    }

    #[test]
    fn it_loads_indices_for_uppercase_j() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        assert_eq!(44, pcf.load_glyph_indices()[&UPPERCASE_J]);
    }

    #[test]
    fn it_loads_indices_for_uppercase_w() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        assert_eq!(57, pcf.load_glyph_indices()[&UPPERCASE_W]);
    }

    #[test]
    fn it_loads_all_metrics_for_uppercase_a() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        let indices = pcf.load_glyph_indices();
        let compressed_metrics = CompressedMetrics {
            left_side_bearing: 0,
            right_side_bearing: 7,
            character_width: 8,
            character_ascent: 9,
            character_descent: 0,
            character_attributes: 0,
        };

        assert_eq!(
            compressed_metrics,
            pcf.load_all_metrics(&indices)[&UPPERCASE_A]
        );
    }

    #[test]
    fn it_loads_all_metrics_for_uppercase_j() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        let indices = pcf.load_glyph_indices();
        let compressed_metrics = CompressedMetrics {
            left_side_bearing: -1,
            right_side_bearing: 2,
            character_width: 3,
            character_ascent: 9,
            character_descent: 2,
            character_attributes: 0,
        };

        assert_eq!(
            compressed_metrics,
            pcf.load_all_metrics(&indices)[&UPPERCASE_J]
        );
    }

    #[test]
    fn it_loads_all_metrics_for_uppercase_w() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        let indices = pcf.load_glyph_indices();
        let compressed_metrics = CompressedMetrics {
            left_side_bearing: 0,
            right_side_bearing: 11,
            character_width: 11,
            character_ascent: 9,
            character_descent: 0,
            character_attributes: 0,
        };

        assert_eq!(
            compressed_metrics,
            pcf.load_all_metrics(&indices)[&UPPERCASE_W]
        );
    }

    #[test]
    fn it_loads_bitmap_offsets_for_uppercase_a() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        let indices = pcf.load_glyph_indices();

        assert_eq!(960, pcf.load_bitmap_offsets(&indices)[&UPPERCASE_A]);
    }

    #[test]
    fn it_loads_bitmap_offsets_for_uppercase_j() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        let indices = pcf.load_glyph_indices();

        assert_eq!(1284, pcf.load_bitmap_offsets(&indices)[&UPPERCASE_J]);
    }

    #[test]
    fn it_loads_bitmap_offsets_for_uppercase_w() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        let indices = pcf.load_glyph_indices();

        assert_eq!(1768, pcf.load_bitmap_offsets(&indices)[&UPPERCASE_W]);
    }

    #[test]
    fn it_has_an_uppercase_a() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        #[rustfmt::skip]
        let expected = Glyph {
            code_point: UPPERCASE_A,
            encoding: Some('A'),
            bitmap: vec![
                0, 0, 0, 1, 0, 0, 0,
                0, 0, 0, 1, 1, 0, 0,
                0, 0, 1, 0, 1, 0, 0,
                0, 0, 1, 0, 0, 1, 0,
                0, 0, 1, 0, 0, 1, 0,
                0, 1, 1, 1, 1, 1, 0,
                0, 1, 0, 0, 0, 0, 1,
                0, 1, 0, 0, 0, 0, 1,
                1, 0, 0, 0, 0, 0, 1,
            ],
            bounding_box: BoundingBox {
                size: Coord::new(7, 9),
                offset: Coord::new(0, 0),
            },
            shift_x: 8,
            shift_y: 0,
            tile_index: 0,
        };
        let glyph = &pcf.glyphs[&UPPERCASE_A];
        assert_eq!(expected, *glyph);
    }

    #[test]
    fn it_has_an_uppercase_j() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        #[rustfmt::skip]
        let expected = Glyph {
            code_point: UPPERCASE_J,
            encoding: Some('J'),
            bitmap: vec![
                0, 0, 1,
                0, 0, 1,
                0, 0, 1,
                0, 0, 1,
                0, 0, 1,
                0, 0, 1,
                0, 0, 1,
                0, 0, 1,
                0, 0, 1,
                0, 0, 1,
                1, 1, 0,
            ],
            bounding_box: BoundingBox {
                size: Coord { x: 3, y: 11 },
                offset: Coord { x: -1, y: -2 },
            },
            shift_x: 3,
            shift_y: 0,
            tile_index: 0,
        };
        let glyph = &pcf.glyphs[&UPPERCASE_J];
        assert_eq!(expected, *glyph);
    }

    #[test]
    fn it_has_an_uppercase_w() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        #[rustfmt::skip]
        let expected = Glyph {
            code_point: UPPERCASE_W,
            encoding: Some('W'),
            bitmap: vec![
                1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
                0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1,
                0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0,
                0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0,
                0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0,
                0, 0, 1, 1, 0, 0, 0, 1, 0, 1, 0,
                0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0,
                0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0,
                0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0,
            ],
            bounding_box: BoundingBox {
                size: Coord { x: 11, y: 9 },
                offset: Coord { x: 0, y: 0 }
            },
            shift_x: 11,
            shift_y: 0,
            tile_index: 0,
        };
        let glyph = &pcf.glyphs[&UPPERCASE_W];
        assert_eq!(expected, *glyph);
    }
}
