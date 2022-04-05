#![no_std]
#![no_main]

use agb::{
    display::{palette16::Palette16, tiled::TileSetting, Font, Priority},
    include_font,
};

const FONT: Font = include_font!("examples/RobotoCondensed-Regular.ttf", 14);

#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    let (gfx, mut vram) = gba.display.video.tiled0();
    let vblank = agb::interrupt::VBlank::get();

    vram.set_background_palettes(&[Palette16::new([
        0x0000, 0x0ff0, 0x00ff, 0xf00f, 0xf0f0, 0x0f0f, 0xaaaa, 0x5555, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ])]);

    let background_tile = vram.new_dynamic_tile().fill_with(0);

    let mut bg = gfx.background(Priority::P0);

    for y in 0..20u16 {
        for x in 0..30u16 {
            bg.set_tile(
                &mut vram,
                (x, y).into(),
                &background_tile.tile_set(),
                TileSetting::from_raw(background_tile.tile_index()),
            );
        }
    }

    vram.remove_dynamic_tile(background_tile);

    FONT.render_text(3, 3, "Hello, World!", 1, 2, 100, &mut bg, &mut vram);

    bg.commit();
    bg.show();

    loop {
        vblank.wait_for_vblank();
    }
}
