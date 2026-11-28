/*!
* This module handles running the enumlator for frontends, so all the frontend packages are just
* the minimum glue meant for their specific platforms. This will handle the frame timing, audio,
* and anything else that can be shared between platforms that isn't directly related to the gameboy
* itself.
*/

use std::time::Duration;

use crate::audio::Audio;
use gameboy_core::Gameboy;

/// How long one gameboy frame lasts. The gameby has 70224 T Cycles running at 4194304 Hz,
/// so about 16.74 ms. That gives you about 59.73 fps
const FRAME_TIME_U64: u64 = 70_224 * 1_000_000_000 / 4_194_304;
const FRAME_TIME: Duration = Duration::from_nanos(FRAME_TIME_U64);

/// The longest duration that we will try to advance to. Certain things can cause the emulator to
/// hang, like being on an inactive tab. We don't want to fast forward 1000s of frames, so just
/// drop any time past 5 frames
const MAX_TIME_DELTA: Duration = Duration::from_nanos(FRAME_TIME_U64 * 5);

/// The Gameboy wrapped in a frontend-agnostic player that handles the system's audio, timing, and what not
pub struct Player {
    pub gameboy: Gameboy,
    audio: Option<Audio>,
    time_delta: Duration,
}

impl Player {
    pub fn new(rom: Vec<u8>) -> Self {
        let audio = Audio::new();

        // Without an audio device we still need a real rate, since the APU divides by it. The
        // samples it makes are just dropped in advance.
        let sample_rate = match &audio {
            Some(a) => a.sample_rate,
            None => 48_000,
        };

        Player {
            gameboy: Gameboy::new(rom, sample_rate),
            audio,
            time_delta: Duration::ZERO,
        }
    }

    /// Runs however many frames are due now that `elapsed` more real time has passed.
    /// Returns true if at least one frame ran, so the frontend knows there's a new frame to draw.
    ///
    /// This is required to keep the emulator running at the exact hz of the gameboy, otherwise it
    /// will be off by some amount of ms per frame. It also allows us to get audio samples between
    /// frames, otherwise we only get the samples 59 times per second.
    pub fn advance(&mut self, elapsed: Duration) -> bool {
        self.time_delta = (self.time_delta + elapsed).min(MAX_TIME_DELTA);

        let mut stepped = false;
        while self.time_delta >= FRAME_TIME {
            self.gameboy.step_frame();
            let samples = self.gameboy.take_samples();
            if let Some(audio) = &self.audio {
                audio.push(&samples);
            }
            self.time_delta -= FRAME_TIME;
            stepped = true;
        }
        stepped
    }

    pub fn play_audio(&self) {
        if let Some(audio) = &self.audio {
            audio.play();
        }
    }

    pub fn pause_audio(&self) {
        if let Some(audio) = &self.audio {
            audio.pause();
        }
    }
}
