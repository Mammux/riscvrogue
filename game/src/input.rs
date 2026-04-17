pub enum GameKey {
    Move { dx: i32, dy: i32 },
    Options,
    Char(u8),
    Quit,
    Unknown,
}

pub fn decode_key(byte: u8) -> GameKey {
    match byte {
        b'h' | b'4' => GameKey::Move { dx: -1, dy: 0 },
        b'l' | b'6' => GameKey::Move { dx: 1, dy: 0 },
        b'k' | b'8' => GameKey::Move { dx: 0, dy: -1 },
        b'j' | b'2' => GameKey::Move { dx: 0, dy: 1 },
        b'y' | b'7' => GameKey::Move { dx: -1, dy: -1 },
        b'u' | b'9' => GameKey::Move { dx: 1, dy: -1 },
        b'b' | b'1' => GameKey::Move { dx: -1, dy: 1 },
        b'n' | b'3' => GameKey::Move { dx: 1, dy: 1 },
        b'O' | b'o' => GameKey::Options,
        b'q' | 0x03 | 0x04 => GameKey::Quit,
        b' '..=b'~' => GameKey::Char(byte),
        _ => GameKey::Unknown,
    }
}
