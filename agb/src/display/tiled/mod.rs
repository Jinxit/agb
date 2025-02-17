mod infinite_scrolled_map;
mod map;
mod tiled0;
mod vram_manager;

use agb_fixnum::Vector2D;
pub use infinite_scrolled_map::{InfiniteScrolledMap, PartialUpdateStatus};
pub use map::{MapLoan, RegularMap};
pub use tiled0::Tiled0;
pub use vram_manager::{DynamicTile, TileFormat, TileIndex, TileSet, VRamManager};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegularBackgroundSize {
    Background32x32,
    Background64x32,
    Background32x64,
    Background64x64,
}

impl RegularBackgroundSize {
    pub fn width(&self) -> u32 {
        match self {
            RegularBackgroundSize::Background32x32 => 32,
            RegularBackgroundSize::Background64x32 => 64,
            RegularBackgroundSize::Background32x64 => 32,
            RegularBackgroundSize::Background64x64 => 64,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            RegularBackgroundSize::Background32x32 => 32,
            RegularBackgroundSize::Background64x32 => 32,
            RegularBackgroundSize::Background32x64 => 64,
            RegularBackgroundSize::Background64x64 => 64,
        }
    }

    pub(crate) fn size_flag(&self) -> u16 {
        match self {
            RegularBackgroundSize::Background32x32 => 0,
            RegularBackgroundSize::Background64x32 => 1,
            RegularBackgroundSize::Background32x64 => 2,
            RegularBackgroundSize::Background64x64 => 3,
        }
    }

    pub(crate) fn num_tiles(&self) -> usize {
        (self.width() * self.height()) as usize
    }

    pub(crate) fn num_screen_blocks(&self) -> usize {
        self.num_tiles() / (32 * 32)
    }

    // This is hilariously complicated due to how the GBA stores the background screenblocks.
    // See https://www.coranac.com/tonc/text/regbg.htm#sec-map for an explanation
    pub(crate) fn gba_offset(&self, pos: Vector2D<u16>) -> usize {
        let x_mod = pos.x & (self.width() as u16 - 1);
        let y_mod = pos.y & (self.height() as u16 - 1);

        let screenblock = (x_mod / 32) + (y_mod / 32) * (self.width() as u16 / 32);

        let pos = screenblock * 32 * 32 + (x_mod % 32 + 32 * (y_mod % 32));

        pos as usize
    }

    pub(crate) fn tile_pos_x(&self, x: i32) -> u16 {
        ((x as u32) & (self.width() - 1)) as u16
    }

    pub(crate) fn tile_pos_y(&self, y: i32) -> u16 {
        ((y as u32) & (self.height() - 1)) as u16
    }

    pub(crate) fn px_offset_x(&self, x: i32) -> u16 {
        ((x as u32) & (self.width() * 8 - 1)) as u16
    }

    pub(crate) fn px_offset_y(&self, y: i32) -> u16 {
        ((y as u32) & (self.height() * 8 - 1)) as u16
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
struct Tile(u16);

impl Tile {
    fn new(idx: TileIndex, setting: TileSetting) -> Self {
        Self(idx.index() | setting.setting())
    }

    fn tile_index(self) -> TileIndex {
        TileIndex::new(self.0 as usize & ((1 << 10) - 1))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TileSetting(u16);

impl TileSetting {
    pub const fn new(tile_id: u16, hflip: bool, vflip: bool, palette_id: u8) -> Self {
        Self(
            (tile_id & ((1 << 10) - 1))
                | ((hflip as u16) << 10)
                | ((vflip as u16) << 11)
                | ((palette_id as u16) << 12),
        )
    }

    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    fn index(self) -> u16 {
        self.0 & ((1 << 10) - 1)
    }

    fn setting(self) -> u16 {
        self.0 & !((1 << 10) - 1)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test_case]
    fn rem_euclid_width_works(_gba: &mut crate::Gba) {
        use RegularBackgroundSize::*;

        let sizes = [
            Background32x32,
            Background32x64,
            Background64x32,
            Background64x64,
        ];

        for size in sizes.iter() {
            let width = size.width() as i32;

            assert_eq!(size.tile_pos_x(8), 8);
            assert_eq!(size.tile_pos_x(3 + width), 3);
            assert_eq!(size.tile_pos_x(7 + width * 9), 7);

            assert_eq!(size.tile_pos_x(-8), (size.width() - 8) as u16);
            assert_eq!(size.tile_pos_x(-17 - width * 8), (size.width() - 17) as u16);
        }
    }
}
