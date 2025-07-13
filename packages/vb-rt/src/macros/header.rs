#[macro_export]
macro_rules! rom_header {
    ($game_title:expr, $maker_code:expr, $game_code:expr) => {
        #[unsafe(link_section = ".rom_header")]
        #[used]
        pub static ROM_HEADER: [u8; 32] =
            vb_rt::macros::header::rom_header($game_title, $maker_code, $game_code);
    };
}

pub const fn rom_header(game_title: &str, maker_code: &str, game_code: &str) -> [u8; 32] {
    let mut result = [0; 32];

    // This should be shift-jis
    let game_title_bytes = game_title.as_bytes();
    assert!(
        game_title_bytes.len() <= 20,
        "Game title must be <= 20 bytes"
    );
    let mut idx = 0;
    while idx < game_title_bytes.len() {
        result[idx] = game_title_bytes[idx];
        idx += 1;
    }

    let maker_code_bytes = maker_code.as_bytes();
    assert!(maker_code_bytes.len() == 2, "Maker code must be 2 bytes");
    let mut idx = 0;
    while idx < maker_code_bytes.len() {
        result[idx + 0x19] = maker_code_bytes[idx];
        idx += 1;
    }

    let game_code_bytes = game_code.as_bytes();
    assert!(game_code_bytes.len() <= 4, "Game code must be <= 4 bytes");
    let mut idx = 0;
    while idx < game_code_bytes.len() {
        result[idx + 0x1b] = game_code_bytes[idx];
        idx += 1;
    }

    result[0x1f] = 1;
    result
}
