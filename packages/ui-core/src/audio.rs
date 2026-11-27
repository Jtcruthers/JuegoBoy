/*!
* This module holds the core audio functionality.
* The idea is the emulator pushes samples into a shared queue, and cpal's callback pulls
* them out whenever the device needs more.
*
* Since the gameboy uses an analog signal, it doesn't have a sample rate. So we we will
* just request at the device's preferred sample rate.
*/
use log::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct Audio {
    stream: cpal::Stream,
    queue: Arc<Mutex<VecDeque<f32>>>,
    pub sample_rate: u32,
}

impl Audio {
    /// opens the default output device. Returns None if there isn't one (or it can't be opened), so
    /// the emulator can keep running without sound.
    pub fn new() -> Option<Self> {
        let device = cpal::default_host().default_output_device()?;
        let default_config = match device.default_output_config() {
            Ok(config) => config,

            Err(err) => {
                log::warn!("No usable audio output config: {err}");
                return None;
            }
        };

        let config: cpal::StreamConfig = default_config.into();
        let sample_rate = config.sample_rate;
        let channels = config.channels as usize;

        let queue = Arc::new(Mutex::new(VecDeque::<f32>::new()));
        let callback_queue = queue.clone();
        let stream = device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut queue = callback_queue.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    // Add 0 if there is nothing to unwrap so that it is silence
                    let left = queue.pop_front().unwrap_or(0.0);
                    let right = queue.pop_front().unwrap_or(0.0);
                    frame[0] = left;
                    if channels > 1 {
                        frame[1] = right;
                    }
                    for sample in frame.iter_mut().skip(2) {
                        *sample = 0.0;
                    }
                }
            },
            |err| match err.kind() {
                cpal::ErrorKind::Xrun => debug!("Audio stream: {err}"),
                _ => error!("Audio stream error: {err}"),
            },
            None,
        );
        let stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                warn!("Couldn't open audio stream: {err}");
                return None;
            }
        };

        let audio = Audio {
            stream,
            queue,
            sample_rate,
        };
        audio.play();
        Some(audio)
    }

    /// pushes a set of samples to be played onto the queue.
    /// The queue is capped around 100ms to stop emulator from getting too out of sync
    pub fn push(&self, samples: &[f32]) {
        let mut queue = self.queue.lock().unwrap();
        queue.extend(samples);

        // Keep it a multiple of 2 so left and right never swap
        let max = (self.sample_rate as usize / 10) * 2;
        if queue.len() > max {
            let excess = (queue.len() - max + 1) & !1;
            queue.drain(..excess);
        }
    }

    pub fn play(&self) {
        if let Err(err) = self.stream.play() {
            warn!("Couldn't start audio: {err}");
        }
    }

    pub fn pause(&self) {
        if let Err(err) = self.stream.pause() {
            warn!("Couldn't pause audio: {err}");
        }
    }
}
