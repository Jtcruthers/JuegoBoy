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

/// Enum for the various channels in the APU, since they all have differences
enum ChannelType {
    One {
        nr10: u8,
        sweep_timer: u8,
        envelope_timer: u8,
    },
    Two {
        envelope_timer: u8,
    },
    Three {
        nr30: u8,
        volume: u8,
    },
    Four {
        envelope_timer: u8,
    },
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
    nrx1: u8,
    nrx2: u8,
    nrx3: u8,
    nrx4: u8,

    period_counter: u16,
    length_timer: u8,
    envelope_timer: u8,

    waveform_generator: WaveformGenerator,
    channel_type: ChannelType,
}

impl Channel {
    fn new_ch1() -> Self {
        Channel {
            nrx1: 0,
            nrx2: 0,
            nrx3: 0,
            nrx4: 0,

            period_counter: 0,
            length_timer: 0,
            envelope_timer: 0,

            waveform_generator: WaveformGenerator::Pulse,

            channel_type: ChannelType::One {
                nr10: 0,
                sweep_timer: 0,
                envelope_timer: 0,
            },
        }
    }

    fn new_ch2() -> Self {
        Channel {
            nrx1: 0,
            nrx2: 0,
            nrx3: 0,
            nrx4: 0,

            period_counter: 0,
            length_timer: 0,
            envelope_timer: 0,

            waveform_generator: WaveformGenerator::Pulse,

            channel_type: ChannelType::Two { envelope_timer: 0 },
        }
    }

    fn new_ch3() -> Self {
        Channel {
            nrx1: 0,
            nrx2: 0,
            nrx3: 0,
            nrx4: 0,

            period_counter: 0,
            length_timer: 0,
            envelope_timer: 0,

            waveform_generator: WaveformGenerator::Wave,

            channel_type: ChannelType::Three { nr30: 0, volume: 0 },
        }
    }

    fn new_ch4() -> Self {
        Channel {
            nrx1: 0,
            nrx2: 0,
            nrx3: 0,
            nrx4: 0,

            period_counter: 0,
            length_timer: 0,
            envelope_timer: 0,

            waveform_generator: WaveformGenerator::LFSR,

            channel_type: ChannelType::Four { envelope_timer: 0 },
        }
    }

    /// Write the byte to the register at the address. The APU is responsible for giving the Channel
    /// the offset, so if you want to write to nrx2 for any channel, it would call write_byte(2, value)
    /// on the specific channel itself
    fn write_byte(&mut self, address_offset: u16, value: u8) {
        match address_offset {
            0 => match &mut self.channel_type {
                ChannelType::One { nr10, .. } => *nr10 = value,
                ChannelType::Three { nr30, .. } => *nr30 = value,
                _ => (),
            },
            1 => self.nrx1 = value,
            2 => self.nrx2 = value,
            3 => self.nrx3 = value,
            4 => self.nrx4 = value,
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
                ChannelType::One { nr10, .. } => nr10 | 0b1000_0000,
                ChannelType::Three { nr30, .. } => nr30 | 0b0111_1111,
                _ => 0xFF, // Doesn't exist, so read 0xFF
            },
            1 => self.nrx1 | 0b0011_1111,
            2 => match self.channel_type {
                ChannelType::Three { .. } => self.nrx2 | 0b1001_1111,
                _ => self.nrx2,
            },
            3 => match self.channel_type {
                ChannelType::Four { .. } => self.nrx3,
                _ => 0xFF,
            },
            4 => self.nrx4 | 0b1011_1111, // Bits 7 and 2-0 are write-only, and 5-3 are empty, so set them
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
}

impl APU {
    pub fn new() -> Self {
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
        }
    }

    pub fn tick(&mut self, timer_div: u8) {
        // increment div_apu when bit 4 of timer_div goes from 1 to 0
        // I think since the timer is kind of the driver, the div_apu increment should be handled
        // first before the rest of the tick.
        if self.timer_div_last & 0b0001_0000 > 0 && timer_div & 0b0001_0000 == 0 {
            self.div_apu += 1;
        }

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
        match address {
            0xFF26 => self.nr52 = value & 0b1111_0000, // Only bit 7 can be set
            0xFF25 => self.nr51 = value,
            0xFF24 => self.nr50 = value,
            0xFF10..=0xFF14 => self.channel1.write_byte(address - 0xFF10, value),
            0xFF16..=0xFF19 => self.channel2.write_byte(address - 0xFF16, value),
            0xFF1A..=0xFF1E => self.channel3.write_byte(address - 0xFF1A, value),
            0xFF20..=0xFF23 => self.channel4.write_byte(address - 0xFF20, value),
            0xFF30..=0xFF3F => self.wave_ram[(address - 0xFF30) as usize] = value,
            _ => panic!("Invalid APU write address: {:X}", address),
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            0xFF26 => self.nr52 | 0b0111_0000, // Bits 6-4 are empty, so set them
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_nr10() {
        let mut apu = APU::new();

        match &mut apu.channel1.channel_type {
            ChannelType::One { nr10, .. } => *nr10 = 0,
            _ => assert!(false, "Channel 1 is not of type One"),
        }
        assert_eq!(apu.read_byte(0xFF10), 0b1000_0000);

        match &mut apu.channel1.channel_type {
            ChannelType::One { nr10, .. } => *nr10 = 0xFF,
            _ => assert!(false, "Channel 1 is not of type One"),
        }
        assert_eq!(apu.read_byte(0xFF10), 0xFF);
    }

    #[test]
    fn test_read_nr11() {
        let mut apu = APU::new();

        match &mut apu.channel1.channel_type {
            ChannelType::One { nr10, .. } => *nr10 = 0xFF,
            _ => assert!(false, "Channel 1 is not of type One"),
        }

        apu.channel1.nrx1 = 0;
        assert_eq!(apu.read_byte(0xFF11), 0b0011_1111);

        apu.channel1.nrx1 = 0xFF;
        assert_eq!(apu.read_byte(0xFF11), 0xFF);
    }

    #[test]
    fn test_read_nr12() {
        let mut apu = APU::new();

        apu.channel1.nrx2 = 0;
        assert_eq!(apu.read_byte(0xFF12), 0);

        apu.channel1.nrx2 = 0xFF;
        assert_eq!(apu.read_byte(0xFF12), 0b1111_1111);
    }

    #[test]
    fn test_read_nr13() {
        let mut apu = APU::new();

        apu.channel1.nrx3 = 0;
        assert_eq!(apu.read_byte(0xFF13), 0xFF);

        apu.channel1.nrx3 = 0xFF;
        assert_eq!(apu.read_byte(0xFF13), 0xFF);
    }

    #[test]
    fn test_read_n1x4() {
        let mut apu = APU::new();

        apu.channel1.nrx4 = 0;
        assert_eq!(apu.read_byte(0xFF14), 0b1011_1111);

        apu.channel1.nrx4 = 0xFF;
        assert_eq!(apu.read_byte(0xFF14), 0xFF);
    }

    #[test]
    fn test_read_nr21() {
        let mut apu = APU::new();

        apu.channel2.nrx1 = 0;
        assert_eq!(apu.read_byte(0xFF16), 0b0011_1111);

        apu.channel2.nrx1 = 0xFF;
        assert_eq!(apu.read_byte(0xFF16), 0xFF);
    }

    #[test]
    fn test_read_nr22() {
        let mut apu = APU::new();

        apu.channel2.nrx2 = 0;
        assert_eq!(apu.read_byte(0xFF17), 0);

        apu.channel2.nrx2 = 0xFF;
        assert_eq!(apu.read_byte(0xFF17), 0b1111_1111);
    }

    #[test]
    fn test_read_nr23() {
        let mut apu = APU::new();

        apu.channel2.nrx3 = 0;
        assert_eq!(apu.read_byte(0xFF18), 0xFF);

        apu.channel2.nrx3 = 0xFF;
        assert_eq!(apu.read_byte(0xFF13), 0xFF);
    }

    #[test]
    fn test_read_nr24() {
        let mut apu = APU::new();

        apu.channel2.nrx4 = 0;
        assert_eq!(apu.read_byte(0xFF19), 0b1011_1111);

        apu.channel2.nrx4 = 0xFF;
        assert_eq!(apu.read_byte(0xFF19), 0xFF);
    }

    #[test]
    fn test_read_nr30() {
        let mut apu = APU::new();

        match &mut apu.channel3.channel_type {
            ChannelType::Three { nr30, .. } => *nr30 = 0,
            _ => assert!(false, "Channel 3 is not of type Three"),
        }
        assert_eq!(apu.read_byte(0xFF1A), 0b0111_1111);

        match &mut apu.channel3.channel_type {
            ChannelType::Three { nr30, .. } => *nr30 = 0xFF,
            _ => assert!(false, "Channel 3 is not of type Three"),
        }
        assert_eq!(apu.read_byte(0xFF1A), 0xFF);
    }

    #[test]
    fn test_read_nr31() {
        let mut apu = APU::new();

        apu.channel3.nrx1 = 0;
        assert_eq!(apu.read_byte(0xFF1B), 0xFF);

        apu.channel3.nrx1 = 0xFF;
        assert_eq!(apu.read_byte(0xFF1B), 0xFF);
    }

    #[test]
    fn test_read_nr32() {
        let mut apu = APU::new();

        apu.channel3.nrx2 = 0;
        assert_eq!(apu.read_byte(0xFF1C), 0b1001_1111);

        apu.channel3.nrx2 = 0xFF;
        assert_eq!(apu.read_byte(0xFF1C), 0xFF);
    }

    #[test]
    fn test_read_nr33() {
        let mut apu = APU::new();

        apu.channel3.nrx3 = 0;
        assert_eq!(apu.read_byte(0xFF1D), 0xFF);

        apu.channel3.nrx3 = 0xFF;
        assert_eq!(apu.read_byte(0xFF1D), 0xFF);
    }

    #[test]
    fn test_read_nr34() {
        let mut apu = APU::new();

        apu.channel3.nrx4 = 0;
        assert_eq!(apu.read_byte(0xFF1E), 0b1011_1111);

        apu.channel3.nrx4 = 0xFF;
        assert_eq!(apu.read_byte(0xFF1E), 0xFF);
    }

    #[test]
    fn test_read_nr41() {
        let mut apu = APU::new();

        apu.channel4.nrx1 = 0;
        assert_eq!(apu.read_byte(0xFF20), 0xFF);

        apu.channel4.nrx1 = 0xFF;
        assert_eq!(apu.read_byte(0xFF20), 0xFF);
    }

    #[test]
    fn test_read_nr42() {
        let mut apu = APU::new();

        apu.channel4.nrx2 = 0;
        assert_eq!(apu.read_byte(0xFF21), 0);

        apu.channel4.nrx2 = 0xFF;
        assert_eq!(apu.read_byte(0xFF21), 0b1111_1111);
    }

    #[test]
    fn test_read_nr43() {
        let mut apu = APU::new();

        apu.channel4.nrx3 = 0;
        assert_eq!(apu.read_byte(0xFF22), 0);

        apu.channel4.nrx3 = 0xFF;
        assert_eq!(apu.read_byte(0xFF22), 0xFF);
    }

    #[test]
    fn test_read_nr44() {
        let mut apu = APU::new();

        apu.channel4.nrx4 = 0;
        assert_eq!(apu.read_byte(0xFF23), 0b1011_1111);

        apu.channel4.nrx4 = 0xFF;
        assert_eq!(apu.read_byte(0xFF23), 0xFF);
    }
}
