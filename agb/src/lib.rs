#![no_std]
// This appears to be needed for testing to work
#![cfg_attr(test, no_main)]
#![deny(clippy::all)]
#![feature(custom_test_frameworks)]
#![feature(alloc_error_handler)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
//! # agb
//! `agb` is a library for making games on the Game Boy Advance using the Rust
//! programming language. It attempts to be a high level abstraction over the
//! internal workings of the Game Boy Advance whilst still being high
//! performance and memory efficient.
//!
//! To get started with agb, you should clone the [template repo](https://github.com/agbrs/template) and work from there.

/// This macro is used to convert a png or bmp into a format usable by the Game Boy Advance.
///
/// The macro expects to be linked to a `toml` file which contains a metadata about the image
/// and a link to the png or bmp itself. See the examples below for a full definition of the format.
///
/// # The manifest file format
///
/// The following is an example of the toml file you would need to create. Generally you will
/// find this in the `gfx` folder in the same level as the `src` folder (see the examples).
///
/// Suppose that the following is in `gfx/sprites.toml`.
///
/// ```toml
/// version = "1.0" # version included for compatibility reasons
///
/// [images.objects]
/// filename = "sprites.png"
/// tile_size = "16x16"
/// transparent_colour = "ff0044"
/// ```
///
/// You then import this using:
/// ```rust,ignore
/// agb::include_gfx!("gfx/sprites.toml");
/// ```
///
/// This will generate something along the lines of the following:
///
/// ```
/// // module name comes from the name of the toml file, so `sprites` in this case because it is
/// // called `sprites.toml`
/// mod sprites {
///     const objects = /* ... */;
/// }
/// ```
///
/// And objects will be an instance of [`TileData`][crate::display::tile_data::TileData]
///
/// # Examples
///
/// ## Loading sprites:
///
/// In `gfx/sprites.toml`:
/// ```toml
/// version = "1.0"
///
/// [image.sprites]
/// filename = "sprites.png"
/// tile_size = "16x16"
/// transparent_colour = "ff0044"
/// ```
///
/// In `src/main.rs`
/// ```
/// mod gfx {
///     use agb::display::object::ObjectControl;
///
///     // Import the sprites into this module. This will create a `sprites` module
///     // and within that will be a constant called `sprites` which houses all the
///     // palette and tile data.
///     agb::include_gfx!("gfx/sprites.toml");
///
///     // Loads the sprites tile data and palette data into VRAM
///     pub fn load_sprite_data(object: &mut ObjectControl) {
///         object.set_sprite_palettes(sprites::sprites.palettes);
///         object.set_sprite_tilemap(sprites::sprites.tiles);
///     }
/// }
/// ```
///
/// ## Loading tiles:
///
/// In `gfx/tiles.toml`:
/// ```toml
/// version = "1.0"
///
/// [image.background]
/// filename = "tile_sheet.png"
/// tile_size = "8x8"
/// transparent_colour = "2ce8f4"
/// ```
///
/// In `src/main.rs`:
/// ```
/// mod gfx {
///     use agb::display::background::BackgroundDistributor;
///
///     agb::include_gfx!("gfx/tile_sheet.toml");
///
///     pub fn load_tile_sheet(tiled: &mut BackgroundDistributor) {
///         tiled.set_background_palettes(tile_sheet::background.palettes);
///         tiled.set_background_tilemap(tile_sheet::background.tiles);
///     }
/// }
/// ```
pub use agb_image_converter::include_gfx;

/// This macro declares the entry point to your game written using `agb`.
///
/// It is already included in the template, but your `main` function must be annotated with `#[agb::entry]`, take no arguments and never return.
/// Doing this will ensure that `agb` can correctly set up the environment to call your rust function on start up.
///
/// # Examples
/// ```
/// #![no_std]
/// #![no_main]
///
/// use agb::Gba;
///
/// #[agb::entry]
/// fn main() -> ! {
///     let mut gba = Gba::new();
///
///     loop {}
/// }
/// ```
pub use agb_macros::entry;

pub use agb_sound_converter::include_wav;
pub use agb_sound_converter::include_xm;

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
mod agb_alloc;

mod arena;
mod bitarray;
/// Implements everything relating to things that are displayed on screen.
pub mod display;
/// Button inputs to the system.
pub mod input;
#[doc(hidden)] // hide for now as the implementation in here is unsound
pub mod interrupt;
mod memory_mapped;
/// Implements logging to the mgba emulator.
pub mod mgba;
/// Implementation of fixnums for working with non-integer values.
pub mod number;
mod single;
/// Implements sound output.
pub mod sound;
/// System BIOS calls / syscalls.
pub mod syscall;
/// Interactions with the internal timers
pub mod timer;

#[cfg(not(test))]
use core::fmt::Write;

#[cfg(not(test))]
#[panic_handler]
#[allow(unused_must_use)]
fn panic_implementation(info: &core::panic::PanicInfo) -> ! {
    if let Some(mut mgba) = mgba::Mgba::new() {
        write!(mgba, "{}", info);
        mgba.set_level(mgba::DebugLevel::Fatal);
    }

    loop {}
}

static mut GBASINGLE: single::Singleton<Gba> = single::Singleton::new(unsafe { Gba::single_new() });

/// The Gba struct is used to control access to the Game Boy Advance's hardware in a way which makes it the
/// borrow checker's responsibility to ensure no clashes of global resources.
///
/// This is typically created once at the start of the main function and then the various fields are used
/// to ensure mutually exclusive use of the various hardware registers. It provides a gateway into the main
/// usage of `agb` library.
///
/// # Panics
///
/// Calling this twice will panic.
///
/// # Examples
///
/// ```
/// #![no_std]
/// #![no_main]
///
/// use agb::Gba;
///
/// #[agb::entry]
/// fn main() -> ! {
///     let mut gba = Gba::new();
///
///     // Do whatever you need to do with gba
///
///     loop {}
/// }
/// ```
#[non_exhaustive]
pub struct Gba {
    /// Manages access to the Game Boy Advance's display hardware
    pub display: display::Display,
    /// Manages access to the Game Boy Advance's beeps and boops sound hardware as part of the
    /// original Game Boy's sound chip (the DMG).
    pub sound: sound::dmg::Sound,
    /// Manages access to the Game Boy Advance's direct sound mixer for playing raw wav files.
    pub mixer: sound::mixer::MixerController,
    /// Manages access to the Game Boy Advance's 4 timers.
    pub timers: timer::TimerController,
}

impl Gba {
    /// Creates a new instance of the Gba struct.
    ///
    /// Note that you can only create 1 instance, and trying to create a second will panic.
    ///
    /// # Panics
    ///
    /// Panics if you try to create the second instance.
    pub fn new() -> Self {
        unsafe { GBASINGLE.take() }
    }

    const unsafe fn single_new() -> Self {
        Self {
            display: display::Display::new(),
            sound: sound::dmg::Sound::new(),
            mixer: sound::mixer::MixerController::new(),
            timers: timer::TimerController::new(),
        }
    }
}

impl Default for Gba {
    fn default() -> Self {
        Self::new()
    }
}

#[doc(hidden)]
pub trait Testable {
    fn run(&self, gba: &mut Gba);
}

impl<T> Testable for T
where
    T: Fn(&mut Gba),
{
    fn run(&self, gba: &mut Gba) {
        let mut mgba = mgba::Mgba::new().unwrap();
        mgba.print(
            format_args!("{}...", core::any::type_name::<T>()),
            mgba::DebugLevel::Info,
        )
        .unwrap();
        mgba::number_of_cycles_tagged(785);
        self(gba);
        mgba::number_of_cycles_tagged(785);
        mgba.print(format_args!("[ok]"), mgba::DebugLevel::Info)
            .unwrap();
    }
}

#[panic_handler]
#[cfg(test)]
fn panic_implementation(info: &core::panic::PanicInfo) -> ! {
    if let Some(mut mgba) = mgba::Mgba::new() {
        mgba.print(format_args!("[failed]"), mgba::DebugLevel::Error)
            .unwrap();
        mgba.print(format_args!("Error: {}", info), mgba::DebugLevel::Fatal)
            .unwrap();
    }

    loop {}
}

#[doc(hidden)]
pub fn test_runner(tests: &[&dyn Testable]) {
    let mut mgba = mgba::Mgba::new().unwrap();
    mgba.print(
        format_args!("Running {} tests", tests.len()),
        mgba::DebugLevel::Info,
    )
    .unwrap();

    let mut gba = Gba::new();

    for test in tests {
        test.run(&mut gba);
    }

    mgba.print(
        format_args!("Tests finished successfully"),
        mgba::DebugLevel::Info,
    )
    .unwrap();
}

#[cfg(test)]
#[entry]
fn agb_test_main() -> ! {
    test_main();
    loop {}
}

#[cfg(test)]
fn assert_image_output(image: &str) {
    display::busy_wait_for_vblank();
    display::busy_wait_for_vblank();
    let mut mgba = crate::mgba::Mgba::new().unwrap();
    mgba.print(
        format_args!("image:{}", image),
        crate::mgba::DebugLevel::Info,
    )
    .unwrap();
    display::busy_wait_for_vblank();
}

#[cfg(test)]
mod test {
    use super::Gba;

    #[test_case]
    fn trivial_test(_gba: &mut Gba) {
        assert_eq!(1, 1);
    }

    #[test_case]
    fn gba_struct_is_zero_sized(_gba: &mut Gba) {
        use core::mem;
        assert_eq!(mem::size_of::<Gba>(), 0);
    }

    #[test_case]
    fn wait_30_frames(_gba: &mut Gba) {
        let vblank = crate::interrupt::VBlank::get();
        let mut counter = 0;
        loop {
            if counter > 30 {
                break;
            }
            vblank.wait_for_vblank();
            counter += 1
        }
    }

    #[link_section = ".ewram"]
    static mut EWRAM_TEST: u32 = 5;
    #[test_case]
    fn ewram_static_test(_gba: &mut Gba) {
        unsafe {
            let ewram_ptr = &mut EWRAM_TEST as *mut u32;
            let content = ewram_ptr.read_volatile();
            assert_eq!(content, 5, "expected data in ewram to be 5");
            ewram_ptr.write_volatile(content + 1);
            let content = ewram_ptr.read_volatile();
            assert_eq!(content, 6, "expected data to have increased by one");
            let address = ewram_ptr as usize;
            assert!(
                (0x0200_0000..0x0204_0000).contains(&address),
                "ewram is located between 0x0200_0000 and 0x0204_0000, address was actually found to be {:#010X}",
                address
            );
        }
    }

    #[link_section = ".iwram"]
    static mut IWRAM_EXPLICIT: u32 = 9;
    #[test_case]
    fn iwram_explicit_test(_gba: &mut Gba) {
        unsafe {
            let iwram_ptr = &mut IWRAM_EXPLICIT as *mut u32;
            let address = iwram_ptr as usize;
            assert!(
                (0x0300_0000..0x0300_8000).contains(&address),
                "iwram is located beween 0x0300_0000 and 0x0300_8000, but was actually found to be at {:#010X}",
                address
            );
            let c = iwram_ptr.read_volatile();
            assert_eq!(c, 9, "exctected content to be 9");
            iwram_ptr.write_volatile(u32::MAX);
            let c = iwram_ptr.read_volatile();
            assert_eq!(c, u32::MAX, "expected content to be {}", u32::MAX);
        }
    }

    static mut IMPLICIT_STORAGE: u32 = 9;
    #[test_case]
    fn implicit_data_test(_gba: &mut Gba) {
        unsafe {
            let iwram_ptr = &mut IMPLICIT_STORAGE as *mut u32;
            let address = iwram_ptr as usize;
            assert!(
                (0x0200_0000..0x0204_0000).contains(&address),
                "implicit data storage is expected to be in ewram, which is between 0x0300_0000 and 0x0300_8000, but was actually found to be at {:#010X}",
                address
            );
            let c = iwram_ptr.read_volatile();
            assert_eq!(c, 9, "exctected content to be 9");
            iwram_ptr.write_volatile(u32::MAX);
            let c = iwram_ptr.read_volatile();
            assert_eq!(c, u32::MAX, "expected content to be {}", u32::MAX);
        }
    }
}
