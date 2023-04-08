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
