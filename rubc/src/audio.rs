//! cpal audio output: pumps the APU's stereo f32 sample stream to the host's
//! default output device.
//!
//! Design constraints (from the GB audio pipeline):
//! - The APU produces interleaved stereo f32 (L,R,L,R...) in [-1.0, 1.0]. We ask
//!   it to produce AT the device's native sample rate (`set_sample_rate`) so no
//!   resampling is ever needed here.
//! - The cpal audio callback runs on a realtime thread: it must never block,
//!   allocate heavily, or panic. On underrun (ring empty) it writes silence.
//! - Frontend -> device hand-off is a `Mutex<VecDeque<f32>>` ring. The callback
//!   holds the lock only long enough to copy out a buffer's worth of samples;
//!   the producer (`push_samples`) bounds the buffer to keep latency in check.
//!
//! cpal itself is pure-Rust FFI (objc2 / windows-rs / alsa) — no C compilation,
//! matching the existing wgpu graphics stack. rubc-core stays dependency-free.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Cap the ring at ~4096 stereo frames (8192 f32) to bound output latency. At
/// 48 kHz that is ~85 ms; if the producer outruns the device we drop the oldest
/// samples rather than grow without bound.
const MAX_BUFFERED_FRAMES: usize = 4096;

/// A live audio output. Holds the cpal `Stream` (dropping it stops audio) plus
/// the shared ring the stream's callback drains.
pub struct AudioOutput {
    /// Kept alive for the lifetime of output; dropping stops the stream.
    _stream: cpal::Stream,
    /// Shared producer->callback ring of interleaved stereo f32 samples.
    ring: Arc<Mutex<VecDeque<f32>>>,
    /// Device-native sample rate the APU must target (no resampling).
    sample_rate: u32,
}

impl AudioOutput {
    /// Open the default output device and start a stream. Returns an error
    /// (rather than panicking) when no device/config is available so the caller
    /// can continue silently (headless CI, no soundcard).
    pub fn new() -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("no default audio output device"))?;
        let supported = device
            .default_output_config()
            .map_err(|e| anyhow::anyhow!("no default output config: {e}"))?;

        let sample_rate = supported.sample_rate();
        let channels = supported.channels() as usize;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.config();

        log::info!("audio: rate={sample_rate}Hz channels={channels} format={sample_format:?}");

        let ring: Arc<Mutex<VecDeque<f32>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(MAX_BUFFERED_FRAMES * 2)));

        let err_fn = |err| log::error!("audio: stream error: {err}");

        // The callback reads GB stereo (L,R) frames from the ring and maps them
        // onto the device's channel layout. Build a typed stream for whatever
        // sample format the device exposes (commonly F32, sometimes I16/U16).
        let stream = match sample_format {
            cpal::SampleFormat::F32 => {
                build_stream::<f32>(&device, &config, Arc::clone(&ring), channels, err_fn)?
            }
            cpal::SampleFormat::I16 => {
                build_stream::<i16>(&device, &config, Arc::clone(&ring), channels, err_fn)?
            }
            cpal::SampleFormat::U16 => {
                build_stream::<u16>(&device, &config, Arc::clone(&ring), channels, err_fn)?
            }
            other => {
                return Err(anyhow::anyhow!("unsupported sample format: {other:?}"));
            }
        };
        stream
            .play()
            .map_err(|e| anyhow::anyhow!("failed to start audio stream: {e}"))?;

        Ok(Self {
            _stream: stream,
            ring,
            sample_rate,
        })
    }

    /// The device-native sample rate. Feed this to `Apu::set_sample_rate` so the
    /// APU emits samples at exactly the rate the device consumes them.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Push interleaved stereo f32 samples (L,R,L,R...) from the APU into the
    /// ring. Bounds the buffer to `MAX_BUFFERED_FRAMES`, dropping the oldest
    /// (whole frames) on overflow to keep latency low. Never blocks the caller
    /// for long: a poisoned lock is recovered rather than propagated.
    pub fn push_samples(&self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        let mut ring = self.ring.lock().unwrap_or_else(|e| e.into_inner());
        ring.extend(samples.iter().copied());
        // Buffer stays even-length (we only ever push/pop whole stereo frames),
        // so draining an even count preserves L/R interleave parity.
        let cap = MAX_BUFFERED_FRAMES * 2;
        if ring.len() > cap {
            let drop = (ring.len() - cap) & !1;
            ring.drain(0..drop);
        }
    }

    /// Stereo frames currently queued for the device (diagnostic only).
    pub fn buffered_frames(&self) -> usize {
        let ring = self.ring.lock().unwrap_or_else(|e| e.into_inner());
        ring.len() / 2
    }
}

/// Build a typed output stream whose callback drains GB stereo frames from
/// `ring` and writes them to the device, converting f32 -> `T` and mapping the
/// 2-channel GB output onto `channels` device channels.
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    ring: Arc<Mutex<VecDeque<f32>>>,
    channels: usize,
    err_fn: impl FnMut(cpal::Error) + Send + 'static,
) -> anyhow::Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let stream = device
        .build_output_stream(
            *config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                // Lock briefly, copy out, release. No allocation, no panics:
                // recover a poisoned lock and emit silence on underrun.
                let mut ring = ring.lock().unwrap_or_else(|e| e.into_inner());
                let silence = T::from_sample(0.0f32);
                for frame in data.chunks_mut(channels) {
                    // Pull one GB stereo (L,R) pair, or silence on underrun.
                    let (l, r) = if ring.len() >= 2 {
                        let l = ring.pop_front().unwrap_or(0.0);
                        let r = ring.pop_front().unwrap_or(0.0);
                        (l, r)
                    } else {
                        (0.0, 0.0)
                    };
                    match channels {
                        // Mono device: downmix L+R.
                        1 => {
                            if let Some(s) = frame.get_mut(0) {
                                *s = T::from_sample((l + r) * 0.5);
                            }
                        }
                        // Stereo (or more): L,R on the first two, zero the rest.
                        _ => {
                            if let Some(s) = frame.get_mut(0) {
                                *s = T::from_sample(l);
                            }
                            if let Some(s) = frame.get_mut(1) {
                                *s = T::from_sample(r);
                            }
                            for s in frame.iter_mut().skip(2) {
                                *s = silence;
                            }
                        }
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| anyhow::anyhow!("failed to build output stream: {e}"))?;
    Ok(stream)
}
