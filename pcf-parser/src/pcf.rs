use byteorder::{BigEndian, ByteOrder, LittleEndian};
use std::collections::HashMap;

use super::glyph::Glyph;

// From https://fontforge.org/docs/techref/pcf-format.html
// type field
const PCF_PROPERTIES: i32 = 1 << 0;
const PCF_ACCELERATORS: i32 = 1 << 1;
const PCF_METRICS: i32 = 1 << 2;
const PCF_BITMAPS: i32 = 1 << 3;
const PCF_INK_METRICS: i32 = 1 << 4;
const PCF_BDF_ENCODINGS: i32 = 1 << 5;
const PCF_SWIDTHS: i32 = 1 << 6;
const PCF_GLYPH_NAMES: i32 = 1 << 7;
const PCF_BDF_ACCELERATORS: i32 = 1 << 8;

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
    offset: i32,
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
    left_side_bearing: u8,
    right_side_bearing: u8,
    character_width: u8,
    character_ascent: u8,
    character_descent: u8,
    character_attributes: u8,
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
    min_byte2: i16,
    max_byte2: i16,
    min_byte1: i16,
    max_byte1: i16,
    default_char: i16,
}

#[derive(Debug, Default, PartialEq)]
struct Bitmap {
    glyph_count: i32,
    bitmap_sizes: i32,
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
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

type Tables = HashMap<i32, Table>;

#[derive(Debug, Default)]
pub struct PcfFont<'a> {
    pub glyphs: HashMap<i32, Glyph>,
    tables: Tables,
    bytes: &'a [u8],
    accelerators: Accelerators,
    encoding: Encoding,
    bitmap: Bitmap,
    pub bounding_box: BoundingBox,
    metadata: Metadata,
}

#[derive(Debug, Default, PartialEq)]
struct Metadata {
    indices_offset: i32,
    bitmap_offset_offsets: i32,
    first_bitmap_offset: i32,
    metrics_compressed_raw: i32,
    is_metrics_compressed: bool,
    first_metric_offset: i32,
    metrics_size: i32,
}

impl PcfFont<'_> {
    pub fn new(font: &[u8]) -> PcfFont {
        let mut pcf = PcfFont {
            bytes: font,
            ..Default::default()
        };

        pcf.tables = pcf.read_tables();
        pcf.accelerators = pcf.read_accelerators();
        pcf.encoding = pcf.read_encoding();
        pcf.bitmap = pcf.read_bitmap();
        pcf.bounding_box = pcf.get_bounding_box();
        pcf.metadata = pcf.load_metadata();

        pcf
    }

    // "1fcp"
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

    fn bitmap_format(&self) -> i32 {
        self.tables.get(&PCF_BITMAPS).unwrap().format
    }

    fn read_tables(&self) -> HashMap<i32, Table> {
        let mut tables = HashMap::new();
        let mut cursor = 8;
        for _ in 0..self.table_count() {
            let r#type = LittleEndian::read_i32(&self.bytes[cursor..cursor + 4]);
            let table = Table {
                format: LittleEndian::read_i32(&self.bytes[cursor + 4..cursor + 8]),
                size: LittleEndian::read_i32(&self.bytes[cursor + 8..cursor + 12]),
                offset: LittleEndian::read_i32(&self.bytes[cursor + 12..cursor + 16]),
            };
            tables.insert(r#type, table);
            cursor += 16;
        }

        tables
    }

    fn read_accelerators(&self) -> Accelerators {
        let accelerators = self
            .tables
            .get(&PCF_BDF_ACCELERATORS)
            .or_else(|| self.tables.get(&PCF_ACCELERATORS));

        assert!(accelerators.is_some(), "No accelerator table found");

        let table = accelerators.unwrap();

        let mut cursor = table.offset as usize;
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
        let left_side_bearing = self.bytes[cursor] - 0x80;
        let right_side_bearing = self.bytes[cursor + 1] - 0x80;
        let character_width = self.bytes[cursor + 2] - 0x80;
        let character_ascent = self.bytes[cursor + 3] - 0x80;
        let character_descent = self.bytes[cursor + 4] - 0x80;

        CompressedMetrics {
            left_side_bearing,
            right_side_bearing,
            character_width,
            character_ascent,
            character_descent,
            character_attributes: 0,
        }
    }

    #[allow(clippy::bad_bit_mask)]
    fn read_encoding(&self) -> Encoding {
        let encoding = self.tables.get(&PCF_BDF_ENCODINGS);
        let table = encoding.expect("No encoding table found");

        let mut cursor = table.offset as usize;
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
            min_byte2,
            max_byte2,
            min_byte1,
            max_byte1,
            default_char,
        }
    }

    #[allow(clippy::bad_bit_mask)]
    fn read_bitmap(&self) -> Bitmap {
        let bitmap = self.tables.get(&PCF_BITMAPS);
        let table = bitmap.expect("No bitmap table found");

        let mut cursor = table.offset as usize;
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

        let bitmap_sizes = [one, two, three, four][(format & 3) as usize];

        Bitmap {
            glyph_count,
            bitmap_sizes,
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

    fn load_indices(&self, code_points: &[&i32]) -> Vec<Option<i32>> {
        let mut indices = vec![None; code_points.len()];

        for (i, code_point) in code_points.iter().enumerate() {
            let enc1 = (*code_point >> 8) & 0xFF;
            let enc2 = *code_point & 0xFF;

            if enc1 < self.encoding.min_byte1.into() || enc1 > self.encoding.max_byte1.into() {
                continue;
            }

            if enc2 < self.encoding.min_byte2.into() || enc2 > self.encoding.max_byte2.into() {
                continue;
            }

            let encoding_idx = (enc1 - self.encoding.min_byte1 as i32)
                * (self.encoding.max_byte2 as i32 - self.encoding.min_byte2 as i32 + 1)
                + enc2
                - self.encoding.min_byte2 as i32;
            let cursor: usize = (self.metadata.indices_offset + 2 * encoding_idx)
                .try_into()
                .expect("glyph_idx conversion failed");
            let glyph_idx = BigEndian::read_u16(&self.bytes[cursor..cursor + 2]);
            if glyph_idx != 65535 {
                indices[i] = Some(glyph_idx as i32);
            }
        }

        indices
    }

    fn load_all_metrics(
        &self,
        code_points: &[&i32],
        indices: &[Option<i32>],
    ) -> Vec<Option<CompressedMetrics>> {
        let mut all_metrics = vec![None; code_points.len()];
        for i in 0..code_points.len() {
            if let Some(index) = indices[i] {
                let cursor: usize = (self.metadata.first_metric_offset
                    + self.metadata.metrics_size * index)
                    .try_into()
                    .expect("compressed metrics usize conversion failed");
                let metrics = self.read_compressed_metrics(cursor);

                all_metrics[i] = Some(metrics);
            } else {
                continue;
            }
        }

        all_metrics
    }

    fn load_bitmap_offsets(
        &self,
        code_points: &[&i32],
        indices: &[Option<i32>],
    ) -> Vec<Option<i32>> {
        let mut bitmap_offsets = vec![None; code_points.len()];
        for i in 0..code_points.len() {
            if let Some(index) = indices[i] {
                let cursor: usize = (self.metadata.bitmap_offset_offsets + 4 * index)
                    .try_into()
                    .expect("bitmap_offset usize conversion failed");
                let bitmap_offset = BigEndian::read_u32(&self.bytes[cursor..cursor + 4]) as i32;
                bitmap_offsets[i] = Some(bitmap_offset);
            } else {
                continue;
            }
        }

        bitmap_offsets
    }

    fn load_glyphs(&mut self, code_points: &[i32]) {
        // if isinstance(code_points, int):
        //     code_points = (code_points,)
        // elif isinstance(code_points, str):
        //     code_points = [ord(c) for c in code_points]

        let code_points = code_points
            .iter()
            .filter(|cp| !self.glyphs.contains_key(cp))
            .collect::<Vec<_>>();

        if code_points.is_empty() {
            return;
        };

        let indices = self.load_indices(&code_points);

        if !self.metadata.is_metrics_compressed {
            panic!("uncompressed metrics unimplemented");
        }

        let all_metrics = self.load_all_metrics(&code_points, &indices);
        let bitmap_offsets = self.load_bitmap_offsets(&code_points, &indices);

        let mut index_to_code_point = vec![None; code_points.len()];
        for i in 0..all_metrics.len() {
            if let Some(metrics) = all_metrics[i] {
                let width: usize = (metrics.right_side_bearing - metrics.left_side_bearing)
                    .try_into()
                    .expect("width conversion failed");
                let height: usize = (metrics.character_ascent + metrics.character_descent)
                    .try_into()
                    .expect("height conversion failed");
                let bitmap = vec![0u8; width * height];
                let code_point = *code_points[i];
                let encoding = u32::try_from(code_point).ok().and_then(std::char::from_u32);

                index_to_code_point[i] = Some(code_point);

                let glyph = Glyph {
                    bitmap,
                    code_point,
                    encoding,
                    width,
                    height,
                    dx: metrics.left_side_bearing as i32,
                    dy: -(metrics.character_descent as i32),
                    shift_x: metrics.character_width as i32,
                    shift_y: 0,
                    tile_index: 0,
                };

                self.glyphs.insert(*code_points[i], glyph);
            }
        }

        for i in 0..code_points.len() {
            if let Some(metrics) = all_metrics[i] {
                let offset: usize = (self.metadata.first_bitmap_offset
                    + bitmap_offsets[i].unwrap())
                .try_into()
                .unwrap();
                let width: usize = (metrics.right_side_bearing - metrics.left_side_bearing)
                    .try_into()
                    .unwrap();
                let height: usize = (metrics.character_ascent + metrics.character_descent)
                    .try_into()
                    .unwrap();

                let words_per_row = (width + 31) / 32;
                let bytes_per_row = 4 * words_per_row;
                let code_point = index_to_code_point[i].as_mut().expect("no bitmap found");
                let glyph = self.glyphs.get_mut(code_point).unwrap();
                for y in 0..height {
                    for x in 0..width {
                        let idx = offset + (bytes_per_row * y);
                        let byte = self.bytes[idx];
                        let mask = 128 >> (x % 8);
                        let masked = byte & mask;
                        let on = masked != 0;

                        if on {
                            glyph.bitmap[y * width + x] = 1;
                        }
                    }
                }
            } else {
                continue;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn it_parses_bitmap_format() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        assert_eq!(0xE, pcf.bitmap_format());
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
    fn it_loads_indices() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        let indices = vec![Some(35)];
        assert_eq!(indices, pcf.load_indices(&[&65]));
    }

    #[test]
    fn it_loads_all_metrics() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        let indices = pcf.load_indices(&[&65]);
        let compressed_metrics = CompressedMetrics {
            left_side_bearing: 0,
            right_side_bearing: 7,
            character_width: 8,
            character_ascent: 9,
            character_descent: 0,
            character_attributes: 0,
        };

        assert_eq!(
            vec![Some(compressed_metrics)],
            pcf.load_all_metrics(&[&65], &indices)
        );
    }

    #[test]
    fn it_loads_bitmap_offsets() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let pcf = PcfFont::new(&font[..]);
        let indices = pcf.load_indices(&[&65]);

        assert_eq!(vec![Some(960)], pcf.load_bitmap_offsets(&[&65], &indices));
    }

    // from python
    // 000000001
    // 000001110
    // 001111000
    // 110001000
    // 011001000
    // 000111000
    // 000000111
    #[test]
    fn it_has_an_uppercase_a() {
        let font = include_bytes!("../../assets/OpenSans-Regular-12.pcf");
        let mut pcf = PcfFont::new(&font[..]);
        pcf.load_glyphs(&[65]);
        #[rustfmt::skip]
        let expected = Glyph {
            code_point: 65,
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
            width: 7,
            height: 9,
            dx: 0,
            dy: 0,
            shift_x: 8,
            shift_y: 0,
            tile_index: 0,
        };
        let glyph = pcf.glyphs.get(&65).unwrap();
        assert_eq!(expected, *glyph);
    }
}
