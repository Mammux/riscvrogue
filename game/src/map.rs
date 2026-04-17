//! A hard-coded demo map.
//!
//! Real dungeon generation will live here eventually – for now we
//! just want *something* the player can walk around on to prove the
//! end-to-end pipeline works.

const W: usize = 40;
const H: usize = 12;

// `#` = wall, `.` = floor. Row lengths must equal `W`.
const TILES: [&[u8; W]; H] = [
    b"########################################",
    b"#......................................#",
    b"#....######.........................####",
    b"#....#....#.............................",
    b"#....#....#####.........................",
    b"#....#........#.........................",
    b"#....##########.........................",
    b"#.......................................",
    b"#..................########.............",
    b"#..................#......#.............",
    b"#..................########.............",
    b"########################################",
];

pub struct Map;

impl Map {
    pub fn demo() -> Self {
        Map
    }

    pub fn width(&self) -> usize {
        W
    }

    pub fn height(&self) -> usize {
        H
    }

    pub fn tile(&self, x: usize, y: usize) -> u8 {
        if y >= H || x >= W {
            return b' ';
        }
        TILES[y][x]
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= W || y >= H {
            return false;
        }
        TILES[y][x] == b'.'
    }

    /// Default spawn position (guaranteed to be on floor).
    pub fn spawn(&self) -> (usize, usize) {
        (2, 1)
    }
}
