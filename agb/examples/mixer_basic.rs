#![no_std]
#![no_main]

use agb::fixnum::Num;
use agb::input::{Button, ButtonController, Tri};
use agb::sound::mixer::SoundChannel;
use agb::{include_wav, Gba};

// Music - "Dead Code" by Josh Woodward, free download at http://joshwoodward.com
const DEAD_CODE: &[u8] = include_wav!("examples/JoshWoodward-DeadCode.wav");

#[agb::entry]
fn main(mut gba: Gba) -> ! {
    let mut input = ButtonController::new();
    let vblank_provider = agb::interrupt::VBlank::get();

    let mut timers = gba.timers.timers();
    let mut mixer = gba.mixer.mixer(&mut timers.timer0);
    mixer.enable();

    let channel = SoundChannel::new(DEAD_CODE);
    let channel_id = mixer.play_sound(channel).unwrap();

    loop {
        input.update();

        {
            if let Some(channel) = mixer.channel(&channel_id) {
                let half: Num<i16, 4> = Num::new(1) / 2;
                let half_usize: Num<usize, 8> = Num::new(1) / 2;
                match input.x_tri() {
                    Tri::Negative => channel.panning(-half),
                    Tri::Zero => channel.panning(0.into()),
                    Tri::Positive => channel.panning(half),
                };

                match input.y_tri() {
                    Tri::Negative => channel.playback(half_usize.change_base() + 1),
                    Tri::Zero => channel.playback(1.into()),
                    Tri::Positive => channel.playback(half_usize),
                };

                if input.is_pressed(Button::L) {
                    channel.volume(half);
                } else if input.is_pressed(Button::R) {
                    channel.volume(20.into()); // intentionally introduce clipping
                } else {
                    channel.volume(1.into());
                }
            }
        }

        mixer.frame();
        vblank_provider.wait_for_vblank();
        mixer.after_vblank();
    }
}
