use crate::memory_mapped::MemoryMapped;

#[non_exhaustive]
pub struct MixerController {}

impl MixerController {
    pub(crate) const fn new() -> Self {
        MixerController {}
    }

    pub fn mixer(&mut self) -> Mixer {
        Mixer::new()
    }
}

pub struct Mixer {
    buffer: MixerBuffer,
    channels: [Option<SoundChannel>; 16],
}

impl Mixer {
    fn new() -> Self {
        Mixer {
            buffer: MixerBuffer::new(),
            channels: Default::default(),
        }
    }

    pub fn enable(&self) {
        set_timer_counter_for_frequency_and_enable(SOUND_FREQUENCY);
        set_sound_control_register_for_mixer();
    }

    pub fn vblank(&mut self) {
        self.buffer.swap();
        self.buffer.clear();

        for channel in self.channels.iter_mut() {
            let mut has_finished = false;

            if let Some(some_channel) = channel {
                self.buffer.write_channel(&some_channel);
                some_channel.pos += SOUND_BUFFER_SIZE;

                if some_channel.pos >= some_channel.data.len() {
                    has_finished = true;
                }
            }

            if has_finished {
                channel.take();
            }
        }
    }

    pub fn play_sound(&mut self, new_channel: SoundChannel) {
        for channel in self.channels.iter_mut() {
            if channel.is_some() {
                continue;
            }

            channel.replace(new_channel);
            return;
        }

        panic!("Cannot play more than 16 sounds at once");
    }
}

pub struct SoundChannel {
    data: &'static [u8],
    pos: usize,
}

impl SoundChannel {
    pub fn new(data: &'static [u8]) -> Self {
        SoundChannel { data, pos: 0 }
    }
}

// I've picked one frequency that works nicely. But there are others that work nicely
// which we may want to consider in the future: https://web.archive.org/web/20070608011909/http://deku.gbadev.org/program/sound1.html
const SOUND_FREQUENCY: i32 = 10512;
const SOUND_BUFFER_SIZE: usize = 176;

struct MixerBuffer {
    buffer1: [i8; SOUND_BUFFER_SIZE],
    buffer2: [i8; SOUND_BUFFER_SIZE],

    buffer_1_active: bool,
}

impl MixerBuffer {
    fn new() -> Self {
        MixerBuffer {
            buffer1: [0; SOUND_BUFFER_SIZE],
            buffer2: [0; SOUND_BUFFER_SIZE],

            buffer_1_active: true,
        }
    }

    fn swap(&mut self) {
        self.buffer_1_active = !self.buffer_1_active;

        if self.buffer_1_active {
            enable_dma1_for_sound(&self.buffer1);
        } else {
            enable_dma1_for_sound(&self.buffer2);
        }
    }

    fn clear(&mut self) {
        if self.buffer_1_active {
            self.buffer2.fill(0);
        } else {
            self.buffer1.fill(0);
        }
    }

    fn write_channel(&mut self, channel: &SoundChannel) {
        let data_to_copy = &channel.data[channel.pos..(channel.pos + SOUND_BUFFER_SIZE)];

        if self.buffer_1_active {
            for (i, v) in data_to_copy.iter().enumerate() {
                let v = *v as i8;
                self.buffer2[i] = self.buffer2[i].saturating_add(v);
            }
        } else {
            for (i, v) in data_to_copy.iter().enumerate() {
                let v = *v as i8;
                self.buffer1[i] = self.buffer1[i].saturating_add(v);
            }
        }
    }
}

// Once we have proper DMA support, we should use that rather than hard coding these here too
const DMA1_SOURCE_ADDR: MemoryMapped<u32> = unsafe { MemoryMapped::new(0x0400_00bc) };
const DMA1_DEST_ADDR: MemoryMapped<u32> = unsafe { MemoryMapped::new(0x0400_00c0) };
const _DMA1_WORD_COUNT: MemoryMapped<u16> = unsafe { MemoryMapped::new(0x0400_00c4) }; // sound ignores this for some reason
const DMA1_CONTROL: MemoryMapped<u16> = unsafe { MemoryMapped::new(0x0400_00c6) };

const FIFOA_DEST_ADDR: u32 = 0x0400_00a0;

// Similarly for proper timer support
const TIMER0_COUNTER: MemoryMapped<u16> = unsafe { MemoryMapped::new(0x0400_0100) };
const TIMER0_CONTROL: MemoryMapped<u16> = unsafe { MemoryMapped::new(0x0400_0102) };

const SOUND_CONTROL: MemoryMapped<u16> = unsafe { MemoryMapped::new(0x0400_0082) };
const SOUND_CONTROL_X: MemoryMapped<u16> = unsafe { MemoryMapped::new(0x0400_0084) };

fn enable_dma1_for_sound(sound_memory: &[i8]) {
    let dest_fixed: u16 = 2 << 5; // dest addr control = fixed
    let repeat: u16 = 1 << 9;
    let transfer_type: u16 = 1 << 10; // transfer in words
    let dma_start_timing: u16 = 3 << 12; // sound fifo timing
    let enable: u16 = 1 << 15; // enable

    DMA1_CONTROL.set(0);
    DMA1_SOURCE_ADDR.set(sound_memory.as_ptr() as u32);
    DMA1_DEST_ADDR.set(FIFOA_DEST_ADDR);
    DMA1_CONTROL.set(dest_fixed | repeat | transfer_type | dma_start_timing | enable);
}

fn set_sound_control_register_for_mixer() {
    let sound_a_volume_100: u16 = 1 << 2;
    let sound_a_rout: u16 = 1 << 8;
    let sound_a_lout: u16 = 1 << 9;
    let sound_a_fifo_reset: u16 = 1 << 11;

    SOUND_CONTROL.set(sound_a_volume_100 | sound_a_rout | sound_a_lout | sound_a_fifo_reset);

    // master sound enable
    SOUND_CONTROL_X.set(1 << 7);
}

fn set_timer_counter_for_frequency_and_enable(frequency: i32) {
    let counter = 65536 - (16777216 / frequency);
    TIMER0_COUNTER.set(counter as u16);

    TIMER0_CONTROL.set(1 << 7); // enable the timer
}
