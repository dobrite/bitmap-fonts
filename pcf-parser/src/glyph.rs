#[derive(Debug, PartialEq)]
pub struct Glyph {
    pub code_point: i32,
    pub encoding: Option<char>,
    pub bitmap: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub dx: i32,
    pub dy: i32,
    pub shift_x: i32,
    pub shift_y: i32,
    pub tile_index: i32,
}

impl Glyph {
    pub fn pixel(&self, x: usize, y: usize) -> bool {
        //let width = usize::try_from(self.bounding_box.size.x).unwrap();

        //let bytes_per_row = (width + 7) / 8;
        //let byte_offset = x / 8;
        //let bit_mask = 0x80 >> (x % 8);

        //self.bitmap[byte_offset + bytes_per_row * y] & bit_mask != 0

        println!(
            "{:?} {:?} {:?} {:?} {:?}",
            x,
            y,
            self.width,
            self.height,
            self.bitmap.len()
        );
        //self.bitmap[y * self.width + x] != 0
        true
    }
}
