/*!
* This module emulates the gameboy's APU. It contains 4 channels, each of which has it's own
* Digital-to-Analog Converter (DAC). The output of the 4 DACs go into a mixer, which adds them into
* two analog outputs based on NR51, one for left and one for right. Each of the two stereo outputs
* from the Mixer have their volume scaled based twice, once from NR50 and once from the voumne knob
* on the gameboy. Each of the two outputs then pass through a High-pass filter, and then sent to the
* speaker. The real APU is driven off of T-Cycles, however my emulator is based on M-cycles. I will
* need to see how that affects the implementation.
*
* The docs say that the APU is a bunch of counters. There is one main counter for the APU as a
* whole, the DIV-APU. This increases whenever the timer's DIV's bit 4 goes from high to low, to keep
* a 512 Hz pace. But each of the channels have their own timers.
*
* Each channel has a set of components that control different parts of the sound generation. Each
* channel has a period counter and a length timer. Channels 1 2 and 4 contain an envelope. Only
* channel 1 has a sweep timer. The length, sweep, and envelope timers all fire when the div-apu hits
* a certain number of ticks. The components are differentiated as either a counter if it is tied to
* the system clock, or a timer if it is driven off of DIV-APU events.
*
* As the DIV-APU increments at 512 hz, the timers are triggered at either 256, 128, or 64 hz.
* All of the components are timers except for each channel's Period Counter.
*/

use std::{
    hash::{BuildHasher, Hasher, RandomState},
    ops::Div,
};

/// Enum for the various channels in the APU, since they all have differences
enum ChannelType {
    One { sweep_timer: u8 },
    Two,
    Three { wave_index: u8 },
    Four,
}

/// enum for the various types of waveform generators.
/// Could possibly combine with ChannelType
enum WaveformGenerator {
    /// Pulse waveform, also called Duty in pandocs
    Pulse,
    Wave,
    /// LFSR waveform, Also called Noise in pandocs
    LFSR,
}

/// Channel is a representation of one of the actual channels in a gameboy APU.
/// It contains 4-5 registers, a period counter, a length timer, and one or more of it's own timers
/// based on the type of channel. Since the waveform is the biggest difference, it's an Enum that I
/// can match on
struct Channel {
    /// Only enabled by triggering the channel, not writing to NR52
    /// Channel will turn off if length timer expires while enabled, or if ch1 period sweep
    /// overflows, or the DAC is turned off
    enabled: bool,
    volume: u8,

    nrx0: u8,
    nrx1: u8,
    nrx2: u8,
    nrx3: u8,
    nrx4: u8,

    period_counter: u16,
    length_timer: u8,
    envelope_timer: u8,

    channel_type: ChannelType,
}

impl Channel {
    fn new_ch1() -> Self {
        Channel {
            enabled: true,
            volume: 0,

            nrx0: 0,
            nrx1: 0,
            nrx2: 0,
            nrx3: 0,
            nrx4: 0,

            period_counter: 0,
            length_timer: 0,
            envelope_timer: 0,

            channel_type: ChannelType::One { sweep_timer: 0 },
        }
    }

    fn new_ch2() -> Self {
        Channel {
            enabled: true,
            volume: 0,

            nrx0: 0,
            nrx1: 0,
            nrx2: 0,
            nrx3: 0,
            nrx4: 0,

            period_counter: 0,
            length_timer: 0,
            envelope_timer: 0,

            channel_type: ChannelType::Two,
        }
    }

    fn new_ch3() -> Self {
        Channel {
            enabled: true,
            volume: 0,

            nrx0: 0,
            nrx1: 0,
            nrx2: 0,
            nrx3: 0,
            nrx4: 0,

            period_counter: 0,
            length_timer: 0,
            envelope_timer: 0,

            channel_type: ChannelType::Three { wave_index: 0 },
        }
    }

    fn new_ch4() -> Self {
        Channel {
            enabled: true,
            volume: 0,

            nrx0: 0,
            nrx1: 0,
            nrx2: 0,
            nrx3: 0,
            nrx4: 0,

            period_counter: 0,
            length_timer: 0,
            envelope_timer: 0,

            channel_type: ChannelType::Four,
        }
    }

    /// Enables the channel, resets volume to initial value, and does certain things per channel:
    /// * ch1, ch2, ch3 - set period divider to AUDxLOW and AUDxHIGH
    /// * ch1, ch2, ch4 - resets envelope timer
    /// * ch1 - Sweetp does stuff for ch1
    /// * ch3 - waveram is reset but not refilled
    /// * ch4 - lsfr bits are reset
    fn trigger(&mut self) {
        self.enabled = true;

        self.volume = (self.nrx2 & 0b1111_0000) >> 4;

        if self.length_timer == 0 {
            let length_mask = match self.channel_type {
                ChannelType::Three { .. } => 0xFF, // Only channel3 uses the entire register for length
                _ => 0b0011_1111,
            };
            self.length_timer = self.nrx1 & length_mask;
        }

        // Set period
        match self.channel_type {
            ChannelType::Four => (),
            _ => {
                let high = self.nrx4 as u16 & 0b111;
                let low = self.nrx3 as u16;
                let period = (high << 8) | low;
                self.period_counter = period;
            }
        }

        // Reset envelope
        match self.channel_type {
            ChannelType::Three { .. } => (),
            _ => {
                self.envelope_timer = self.nrx2 & 0b111;
            }
        }

        match self.channel_type {
            ChannelType::One {
                ref mut sweep_timer,
            } => {
                // TODO - actually do something here
                *sweep_timer = 4;
            }
            ChannelType::Three { ref mut wave_index } => {
                *wave_index = 0;
            }
            ChannelType::Four => {
                // lsfr
            }
            _ => (),
        }
    }

    // fn get_generator(&self) -> WaveformGenerator {
    //     match self.channel_type {
    //         ChannelType::One { .. } | ChannelType::Two => WaveformGenerator::Pulse,
    //         ChannelType::Three => WaveformGenerator::Wave,
    //         ChannelType::Four => WaveformGenerator::LFSR,
    //     }
    // }

    /// Write the byte to the register at the address. The APU is responsible for giving the Channel
    /// the offset, so if you want to write to nrx2 for any channel, it would call write_byte(2, value)
    /// on the specific channel itself
    fn write_byte(&mut self, address_offset: u16, value: u8) {
        match address_offset {
            0 => match self.channel_type {
                ChannelType::One { .. } | ChannelType::Three { .. } => self.nrx0 = value,
                _ => (),
            },
            1 => self.nrx1 = value,
            2 => {
                self.nrx2 = value;
                if value & 0b1111_1000 == 0 {
                    // disable dac and channel
                    self.enabled = false;
                }
            }
            3 => self.nrx3 = value,
            4 => {
                self.nrx4 = value;
                // if Triggered
                if value & 0b1000_0000 != 0 {
                    self.trigger();
                }
                // If length enable bit set
                if value & 0b0100_0000 != 0 {
                    // enable length for ch1, ch2, ch3
                }
            }
            _ => panic!("Invalid channel write address: {:X}", address_offset),
        }
    }

    /// Reads the byte to the register at the address. The APU is responsible for giving the Channel
    /// the offset, so if you want to read nrx2 for any channel, it would call read_byte(2)
    /// on the specific channel itself
    fn read_byte(&self, address_offset: u16) -> u8 {
        // This is kinda complex with the masks. There is a better way
        match address_offset {
            0 => match self.channel_type {
                ChannelType::One { .. } => self.nrx0 | 0b1000_0000,
                ChannelType::Three { .. } => self.nrx0 | 0b0111_1111,
                _ => 0xFF, // Doesn't exist, so read 0xFF
            },
            1 => self.nrx1 | 0b0011_1111,
            2 => match self.channel_type {
                ChannelType::Three { .. } => self.nrx2 | 0b1001_1111,
                _ => self.nrx2,
            },
            3 => match self.channel_type {
                ChannelType::Four => self.nrx3,
                _ => 0xFF,
            },
            4 => self.nrx4 | 0b1011_1111,
            _ => panic!("Invalid channel write address: {:X}", address_offset),
        }
    }

    fn reset(&mut self) {
        self.nrx1 = 0;
        self.nrx2 = 0;
        self.nrx3 = 0;
        self.nrx4 = 0;

        self.period_counter = 0;
        self.length_timer = 0;
        self.envelope_timer = 0;

        // TODO - reset the channel_type specific data
    }
}

pub struct APU {
    nr52: u8,
    nr51: u8,
    nr50: u8,
    channel1: Channel,
    channel2: Channel,
    channel3: Channel,
    channel4: Channel,
    wave_ram: [u8; 16],

    div_apu: u32,
    timer_div_last: u8,

    /// The host's playback rate in hz. The APU's output changes every M cycle, which is 1+ million
    /// times per second. So we just use whatever sample rate is asked for since that is too many
    /// to actually use
    cycles_per_sample: f32,
    sample_timer: u32,
    pub samples: Vec<f32>,
}

const M_CYCLES_PER_SECOND: f32 = 1_048_576.0;

impl APU {
    pub fn new(sample_rate: u32) -> Self {
        APU {
            channel1: Channel::new_ch1(),
            channel2: Channel::new_ch2(),
            channel3: Channel::new_ch3(),
            channel4: Channel::new_ch4(),
            nr52: 0x00,
            nr51: 0x00,
            nr50: 0x00,
            wave_ram: [0; 16],

            div_apu: 0,
            timer_div_last: 0,

            cycles_per_sample: M_CYCLES_PER_SECOND.div(sample_rate as f32),
            sample_timer: 0,
            samples: Vec::new(),
        }
    }

    pub fn tick(&mut self, timer_div: u8) {
        // We need to take a sample every M_CYCLES_PER_SECOND / sample_rate to resample for
        // the actual device's rate
        self.sample_timer += 1;
        if self.sample_timer as f32 >= self.cycles_per_sample {
            self.sample_timer = 0;
            // TODO - figure out the actual samples
            let num = RandomState::new().build_hasher().finish() as f32;
            let unit = num / (u32::MAX as f32); // 0.0 to 1.0
            let sample = unit * 2.0 - 1.0; // -1.0 to 1.0
            self.push_samples(sample, sample);
        }

        // increment div_apu when bit 4 of timer_div goes from 1 to 0
        // I think since the timer is kind of the driver, the div_apu increment should be handled
        // first before the rest of the tick.
        let timer_div_last = self.timer_div_last;
        self.timer_div_last = timer_div;
        if timer_div_last & 0b0001_0000 > 0 && timer_div & 0b0001_0000 == 0 {
            self.div_apu += 1;
        } else {
            // work here is done, we only actually do the stuff when div_apu ticks
            return;
        }

        // Has to be after the tick, as being disabled doesn't affect div-apu
        if self.nr52 & 0b1000_0000 == 0 {
            // APU is disabled, reset channels, but keep wave ram. Timer is uneffected
            self.channel1.reset();
            self.channel2.reset();
            self.channel3.reset();
            self.channel4.reset();

            return;
        }

        if self.div_apu % 8 == 0 {
            // envelope sweep
        }

        if self.div_apu % 2 == 0 {
            // sound length
        }

        if self.div_apu % 4 == 0 {
            // channel 1 sweep
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        if self.nr52 & 0b1000_0000 == 0 && address < 0xFF24 {
            // If audio is off, writes here are ignored, but wave_ram is still accissible
            return;
        }
        match address {
            0xFF26 => self.nr52 = value & 0b1000_0000, // Only bit 7 can be set
            0xFF25 => self.nr51 = value,
            0xFF24 => self.nr50 = value,
            0xFF10..=0xFF14 => self.channel1.write_byte(address - 0xFF10, value),
            0xFF15..=0xFF19 => self.channel2.write_byte(address - 0xFF15, value),
            0xFF1A..=0xFF1E => self.channel3.write_byte(address - 0xFF1A, value),
            0xFF1F..=0xFF23 => self.channel4.write_byte(address - 0xFF1F, value),
            0xFF30..=0xFF3F => self.wave_ram[(address - 0xFF30) as usize] = value,
            _ => panic!("Invalid APU write address: {:X}", address),
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            0xFF26 => {
                let mut nr52 = self.nr52 | 0b0111_0000;
                if self.channel1.enabled {
                    nr52 |= 0b1
                }
                if self.channel2.enabled {
                    nr52 |= 0b10
                }
                if self.channel3.enabled {
                    nr52 |= 0b100
                }
                if self.channel4.enabled {
                    nr52 |= 0b1000
                }

                nr52
            }
            0xFF25 => self.nr51,
            0xFF24 => self.nr50,
            0xFF1B => 0xFF, // nr31 is write-only, return 0xFF
            0xFF20 => 0xFF, // nr41 is write-only, return 0xFF
            0xFF10..=0xFF14 => self.channel1.read_byte(address - 0xFF10),
            0xFF15..=0xFF19 => self.channel2.read_byte(address - 0xFF15),
            0xFF1A..=0xFF1E => self.channel3.read_byte(address - 0xFF1A),
            0xFF1F..=0xFF23 => self.channel4.read_byte(address - 0xFF1F),
            0xFF30..=0xFF3F => self.wave_ram[(address - 0xFF30) as usize],
            _ => panic!("Invalid APU read address: {:X}", address),
        }
    }

    /// Push two samples onto the queue, one for the left channel and one for the right channel
    fn push_samples(&mut self, left_sample: f32, right_sample: f32) {
        self.samples.push(left_sample);
        self.samples.push(right_sample);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_nr10() {
        let mut apu = APU::new(48_000);

        apu.channel1.nrx0 = 0;
        assert_eq!(apu.read_byte(0xFF10), 0b1000_0000);

        apu.channel1.nrx0 = 0xFF;
        assert_eq!(apu.read_byte(0xFF10), 0xFF);
    }

    #[test]
    fn test_read_nr11() {
        let mut apu = APU::new(48_000);

        apu.channel1.nrx1 = 0;
        assert_eq!(apu.read_byte(0xFF11), 0b0011_1111);

        apu.channel1.nrx1 = 0xFF;
        assert_eq!(apu.read_byte(0xFF11), 0xFF);
    }

    #[test]
    fn test_read_nr12() {
        let mut apu = APU::new(48_000);

        apu.channel1.nrx2 = 0;
        assert_eq!(apu.read_byte(0xFF12), 0);

        apu.channel1.nrx2 = 0xFF;
        assert_eq!(apu.read_byte(0xFF12), 0b1111_1111);
    }

    #[test]
    fn test_read_nr13() {
        let mut apu = APU::new(48_000);

        apu.channel1.nrx3 = 0;
        assert_eq!(apu.read_byte(0xFF13), 0xFF);

        apu.channel1.nrx3 = 0xFF;
        assert_eq!(apu.read_byte(0xFF13), 0xFF);
    }

    #[test]
    fn test_read_n1x4() {
        let mut apu = APU::new(48_000);

        apu.channel1.nrx4 = 0;
        assert_eq!(apu.read_byte(0xFF14), 0b1011_1111);

        apu.channel1.nrx4 = 0xFF;
        assert_eq!(apu.read_byte(0xFF14), 0xFF);
    }

    #[test]
    fn test_read_nr21() {
        let mut apu = APU::new(48_000);

        apu.channel2.nrx1 = 0;
        assert_eq!(apu.read_byte(0xFF16), 0b0011_1111);

        apu.channel2.nrx1 = 0xFF;
        assert_eq!(apu.read_byte(0xFF16), 0xFF);
    }

    #[test]
    fn test_read_nr22() {
        let mut apu = APU::new(48_000);

        apu.channel2.nrx2 = 0;
        assert_eq!(apu.read_byte(0xFF17), 0);

        apu.channel2.nrx2 = 0xFF;
        assert_eq!(apu.read_byte(0xFF17), 0b1111_1111);
    }

    #[test]
    fn test_read_nr23() {
        let mut apu = APU::new(48_000);

        apu.channel2.nrx3 = 0;
        assert_eq!(apu.read_byte(0xFF18), 0xFF);

        apu.channel2.nrx3 = 0xFF;
        assert_eq!(apu.read_byte(0xFF13), 0xFF);
    }

    #[test]
    fn test_read_nr24() {
        let mut apu = APU::new(48_000);

        apu.channel2.nrx4 = 0;
        assert_eq!(apu.read_byte(0xFF19), 0b1011_1111);

        apu.channel2.nrx4 = 0xFF;
        assert_eq!(apu.read_byte(0xFF19), 0xFF);
    }

    #[test]
    fn test_read_nr30() {
        let mut apu = APU::new(48_000);

        apu.channel3.nrx0 = 0;
        assert_eq!(apu.read_byte(0xFF1A), 0b0111_1111);

        apu.channel3.nrx0 = 0xFF;
        assert_eq!(apu.read_byte(0xFF1A), 0xFF);
    }

    #[test]
    fn test_read_nr31() {
        let mut apu = APU::new(48_000);

        apu.channel3.nrx1 = 0;
        assert_eq!(apu.read_byte(0xFF1B), 0xFF);

        apu.channel3.nrx1 = 0xFF;
        assert_eq!(apu.read_byte(0xFF1B), 0xFF);
    }

    #[test]
    fn test_read_nr32() {
        let mut apu = APU::new(48_000);

        apu.channel3.nrx2 = 0;
        assert_eq!(apu.read_byte(0xFF1C), 0b1001_1111);

        apu.channel3.nrx2 = 0xFF;
        assert_eq!(apu.read_byte(0xFF1C), 0xFF);
    }

    #[test]
    fn test_read_nr33() {
        let mut apu = APU::new(48_000);

        apu.channel3.nrx3 = 0;
        assert_eq!(apu.read_byte(0xFF1D), 0xFF);

        apu.channel3.nrx3 = 0xFF;
        assert_eq!(apu.read_byte(0xFF1D), 0xFF);
    }

    #[test]
    fn test_read_nr34() {
        let mut apu = APU::new(48_000);

        apu.channel3.nrx4 = 0;
        assert_eq!(apu.read_byte(0xFF1E), 0b1011_1111);

        apu.channel3.nrx4 = 0xFF;
        assert_eq!(apu.read_byte(0xFF1E), 0xFF);
    }

    #[test]
    fn test_read_nr41() {
        let mut apu = APU::new(48_000);

        apu.channel4.nrx1 = 0;
        assert_eq!(apu.read_byte(0xFF20), 0xFF);

        apu.channel4.nrx1 = 0xFF;
        assert_eq!(apu.read_byte(0xFF20), 0xFF);
    }

    #[test]
    fn test_read_nr42() {
        let mut apu = APU::new(48_000);

        apu.channel4.nrx2 = 0;
        assert_eq!(apu.read_byte(0xFF21), 0);

        apu.channel4.nrx2 = 0xFF;
        assert_eq!(apu.read_byte(0xFF21), 0b1111_1111);
    }

    #[test]
    fn test_read_nr43() {
        let mut apu = APU::new(48_000);

        apu.channel4.nrx3 = 0;
        assert_eq!(apu.read_byte(0xFF22), 0);

        apu.channel4.nrx3 = 0xFF;
        assert_eq!(apu.read_byte(0xFF22), 0xFF);
    }

    #[test]
    fn test_read_nr44() {
        let mut apu = APU::new(48_000);

        apu.channel4.nrx4 = 0;
        assert_eq!(apu.read_byte(0xFF23), 0b1011_1111);

        apu.channel4.nrx4 = 0xFF;
        assert_eq!(apu.read_byte(0xFF23), 0xFF);
    }

    #[test]
    fn test_read_nr52_channel_enabled() {
        let mut apu = APU::new(48_000);

        apu.write_byte(0xFF26, 0);

        assert_eq!(apu.read_byte(0xFF26), 0b0111_1111);

        apu.channel1.enabled = false;
        assert_eq!(apu.read_byte(0xFF26), 0b0111_1110);

        apu.channel2.enabled = false;
        assert_eq!(apu.read_byte(0xFF26), 0b0111_1100);

        apu.channel3.enabled = false;
        assert_eq!(apu.read_byte(0xFF26), 0b0111_1000);

        apu.channel4.enabled = false;
        assert_eq!(apu.read_byte(0xFF26), 0b0111_0000);
    }

    #[test]
    fn test_nr51() {
        let mut apu = APU::new(48_000);

        apu.write_byte(0xFF25, 0);
        assert_eq!(apu.read_byte(0xFF25), 0);

        apu.write_byte(0xFF25, 0xAA);
        assert_eq!(apu.read_byte(0xFF25), 0xAA);

        apu.write_byte(0xFF25, 0xFF);
        assert_eq!(apu.read_byte(0xFF25), 0xFF);
    }

    #[test]
    fn test_nr50() {
        let mut apu = APU::new(48_000);

        apu.write_byte(0xFF24, 0);
        assert_eq!(apu.read_byte(0xFF24), 0);

        apu.write_byte(0xFF24, 0xAA);
        assert_eq!(apu.read_byte(0xFF24), 0xAA);

        apu.write_byte(0xFF24, 0xFF);
        assert_eq!(apu.read_byte(0xFF24), 0xFF);
    }

    #[test]
    fn test_trigger_resets_wave_index() {
        let mut c = Channel::new_ch3();
        match c.channel_type {
            ChannelType::Three { ref mut wave_index } => {
                *wave_index = 4;
            }
            _ => assert!(false),
        }

        c.trigger();

        match c.channel_type {
            ChannelType::Three { wave_index } => {
                assert_eq!(wave_index, 0)
            }
            _ => assert!(false),
        }
    }
}
