use super::tiled::{DynamicTile, RegularMap, TileSetting, VRamManager};

use alloc::{vec, vec::Vec};

pub struct FontLetter {
    width: u8,
    height: u8,
    data: &'static [u8],
    xmin: i8,
    ymin: i8,
    advance_width: u8,
}

impl FontLetter {
    pub const fn new(
        width: u8,
        height: u8,
        data: &'static [u8],
        xmin: i8,
        ymin: i8,
        advance_width: u8,
    ) -> Self {
        Self {
            width,
            height,
            data,
            xmin,
            ymin,
            advance_width,
        }
    }
}

pub struct Font {
    letters: &'static [FontLetter],
}

impl Font {
    pub const fn new(letters: &'static [FontLetter]) -> Self {
        Self { letters }
    }

    fn letter(&self, letter: char) -> &'static FontLetter {
        &self.letters[letter as usize]
    }
}

impl Font {
    pub fn render_text(
        &self,
        tile_x: u16,
        tile_y: u16,
        text: &str,
        foreground_colour: u8,
        background_colour: u8,
        _max_width: i32,
        bg: &mut RegularMap,
        vram_manager: &mut VRamManager,
    ) -> i32 {
        let mut tiles: Vec<Vec<DynamicTile>> = vec![];

        let mut render_pixel = |x: u16, y: u16| {
            let tile_x = (x / 8) as usize;
            let tile_y = (y / 8) as usize;
            let inner_x = x % 8;
            let inner_y = y % 8;

            if tiles.len() <= tile_x {
                tiles.resize_with(tile_x + 1, || vec![]);
            }

            let x_dynamic_tiles = &mut tiles[tile_x];
            if x_dynamic_tiles.len() <= tile_y {
                x_dynamic_tiles.resize_with(tile_y + 1, || {
                    vram_manager.new_dynamic_tile().fill_with(background_colour)
                });
            }

            let colour = foreground_colour as u32;

            let index = (inner_x + inner_y * 8) as usize;
            tiles[tile_x][tile_y].tile_data[index / 8] |= colour << ((index % 8) * 4);
        };

        let mut current_x_pos = 0i32;
        let current_y_pos = 0i32;

        for c in text.chars() {
            let letter = self.letter(c);

            for letter_y in 0..(letter.height as i32) {
                for letter_x in 0..(letter.width as i32) {
                    let x = current_x_pos + letter_x;
                    let y = current_y_pos + letter_y;

                    let px = letter.data[(letter_x + letter_y * letter.width as i32) as usize];

                    if px > 100 {
                        render_pixel(x as u16, y as u16);
                    }
                }
            }

            current_x_pos += letter.advance_width as i32;
        }

        for (x, x_tiles) in tiles.into_iter().enumerate() {
            for (y, tile) in x_tiles.into_iter().enumerate() {
                bg.set_tile(
                    vram_manager,
                    (tile_x + x as u16, tile_y + y as u16).into(),
                    &tile.tile_set(),
                    TileSetting::from_raw(tile.tile_index()),
                );
                vram_manager.remove_dynamic_tile(tile);
            }
        }

        1
    }
}
