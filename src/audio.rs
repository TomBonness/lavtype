//! Microphone capture and sample-rate normalization.
//!
//! The callback path is deliberately small and allocation-free: it only converts
//! samples, downmixes complete frames, and appends to a preallocated bounded
//! buffer. All stream setup, resampling, and ownership transitions happen on the
//! event-loop/worker thread.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use rubato::audioadapter_buffers::owned::InterleavedOwned;
use rubato::{Fft, FixedSync, Resampler};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use thiserror::Error;

/// The source-rate mono samples captured for one push-to-talk hold.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioClip {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

impl AudioClip {
    pub fn new(sample_rate: u32, samples: Vec<f32>) -> Self {
        Self {
            sample_rate,
            samples,
        }
    }

    /// Return this clip as mono 16 kHz samples, using rubato's synchronous FFT
    /// resampler off the audio callback thread.
    pub fn resample_to_16khz(&self) -> Result<Vec<f32>, CaptureError> {
        resample_to_16khz(&self.samples, self.sample_rate)
    }
}

/// Errors surfaced by input-device setup, capture, and normalization.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CaptureError {
    #[error("no default input device is available")]
    NoInputDevice,
    #[error("could not query the default input configuration: {0}")]
    DefaultConfig(String),
    #[error("unsupported input sample format: {0}")]
    UnsupportedSampleFormat(String),
    #[error("could not build the input stream: {0}")]
    BuildStream(String),
    #[error("could not start the input stream: {0}")]
    StartStream(String),
    #[error("the input stream reported an error")]
    StreamError,
    #[error("a recording is already in progress")]
    AlreadyRecording,
    #[error("there is no recording to stop")]
    NotRecording,
    #[error("the input configuration has no channels")]
    InvalidChannels,
    #[error("the input sample rate is zero")]
    InvalidSampleRate,
    #[error("could not resample audio: {0}")]
    Resample(String),
}

/// The hard upper bound required by the product contract.
pub const MAX_RECORDING_SECONDS: u32 = 55;

/// Convert one F32 sample to normalized PCM.
#[inline]
pub fn f32_to_normalized(sample: f32) -> f32 {
    sample
}

/// Convert one signed 16-bit sample to normalized PCM.
#[inline]
pub fn i16_to_normalized(sample: i16) -> f32 {
    sample as f32 / 32_768.0
}

/// Convert one unsigned 16-bit sample (whose midpoint is silence) to normalized PCM.
#[inline]
pub fn u16_to_normalized(sample: u16) -> f32 {
    (sample as f32 - 32_768.0) / 32_768.0
}

/// Convert interleaved source samples to a bounded mono buffer.
///
/// Incomplete trailing frames are ignored rather than padded with silence.
/// This function does not allocate and is suitable for use from a cpal callback.
fn append_interleaved<T, F>(
    data: &[T],
    channels: usize,
    destination: &mut Vec<f32>,
    capacity: usize,
    mut convert: F,
) where
    F: FnMut(T) -> f32,
    T: Copy,
{
    if channels == 0 {
        return;
    }
    for frame in data.chunks_exact(channels) {
        if destination.len() >= capacity {
            break;
        }
        let mut sum = 0.0f32;
        for &sample in frame {
            sum += convert(sample);
        }
        destination.push(sum / channels as f32);
    }
}

/// Convert an interleaved F32 callback payload to mono.
pub fn downmix_f32(data: &[f32], channels: usize, capacity: usize) -> Vec<f32> {
    let mut output = Vec::with_capacity(data.len().div_ceil(channels.max(1)).min(capacity));
    append_interleaved(data, channels, &mut output, capacity, f32_to_normalized);
    output
}

/// Convert an interleaved I16 callback payload to mono.
pub fn downmix_i16(data: &[i16], channels: usize, capacity: usize) -> Vec<f32> {
    let mut output = Vec::with_capacity(data.len().div_ceil(channels.max(1)).min(capacity));
    append_interleaved(data, channels, &mut output, capacity, i16_to_normalized);
    output
}

/// Convert an interleaved U16 callback payload to mono.
pub fn downmix_u16(data: &[u16], channels: usize, capacity: usize) -> Vec<f32> {
    let mut output = Vec::with_capacity(data.len().div_ceil(channels.max(1)).min(capacity));
    append_interleaved(data, channels, &mut output, capacity, u16_to_normalized);
    output
}

/// Resample mono source-rate samples to 16 kHz with rubato's FFT resampler.
pub fn resample_to_16khz(samples: &[f32], source_rate: u32) -> Result<Vec<f32>, CaptureError> {
    if source_rate == 0 {
        return Err(CaptureError::InvalidSampleRate);
    }
    if samples.is_empty() || source_rate == 16_000 {
        return Ok(samples.to_vec());
    }

    let input = InterleavedOwned::new_from(samples.to_vec(), 1, samples.len())
        .map_err(|error| CaptureError::Resample(error.to_string()))?;
    let mut resampler = Fft::<f32>::new(source_rate as usize, 16_000, 1_024, 1, FixedSync::Both)
        .map_err(|error| CaptureError::Resample(error.to_string()))?;
    let output = resampler
        .process_all(&input, samples.len(), None)
        .map_err(|error| CaptureError::Resample(error.to_string()))?;
    Ok(output.take_data())
}

/// Default cpal input recorder.
pub struct CpalAudioRecorder {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    stream_error: Arc<AtomicBool>,
    at_capacity: Arc<AtomicBool>,
    sample_rate: u32,
    capacity: usize,
    recording: bool,
}

/// Name used by callers that want the platform-default recorder.
pub type DefaultAudioRecorder = CpalAudioRecorder;

impl Default for CpalAudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl CpalAudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            stream_error: Arc::new(AtomicBool::new(false)),
            at_capacity: Arc::new(AtomicBool::new(false)),
            sample_rate: 0,
            capacity: 0,
            recording: false,
        }
    }

    /// True after the callback has filled the 55-second bound. The coordinator
    /// should stop the recorder immediately and treat this like a release.
    pub fn reached_capacity(&self) -> bool {
        self.at_capacity.load(Ordering::Acquire)
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    fn reset_buffer(&mut self, capacity: usize) -> Result<(), CaptureError> {
        let mut buffer = self.buffer.lock().map_err(|_| CaptureError::StreamError)?;
        if buffer.capacity() < capacity {
            let additional = capacity - buffer.capacity();
            buffer.reserve(additional);
        }
        Ok(())
    }

    fn callback_parts(&self) -> (Arc<Mutex<Vec<f32>>>, Arc<AtomicBool>, Arc<AtomicBool>) {
        (
            Arc::clone(&self.buffer),
            Arc::clone(&self.stream_error),
            Arc::clone(&self.at_capacity),
        )
    }
}

impl crate::app::AudioRecorder for CpalAudioRecorder {
    fn start(&mut self) -> Result<(), CaptureError> {
        if self.recording {
            return Err(CaptureError::AlreadyRecording);
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(CaptureError::NoInputDevice)?;
        let supported = device
            .default_input_config()
            .map_err(|error: cpal::Error| CaptureError::DefaultConfig(error.to_string()))?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let channels = usize::from(config.channels);
        if channels == 0 {
            return Err(CaptureError::InvalidChannels);
        }
        let sample_rate = config.sample_rate;
        if sample_rate == 0 {
            return Err(CaptureError::InvalidSampleRate);
        }
        let capacity = (sample_rate as usize)
            .checked_mul(MAX_RECORDING_SECONDS as usize)
            .ok_or(CaptureError::InvalidSampleRate)?;
        self.reset_buffer(capacity)?;
        self.sample_rate = sample_rate;
        self.capacity = capacity;
        let (buffer, stream_error, at_capacity) = self.callback_parts();
        let stream = match sample_format {
            SampleFormat::F32 => {
                let buffer = Arc::clone(&buffer);
                let full = Arc::clone(&at_capacity);
                let error_flag = Arc::clone(&stream_error);
                device.build_input_stream(
                    config,
                    move |data: &[f32], _| {
                        if let Ok(mut destination) = buffer.lock() {
                            append_interleaved(
                                data,
                                channels,
                                &mut destination,
                                capacity,
                                f32_to_normalized,
                            );
                            if destination.len() >= capacity {
                                full.store(true, Ordering::Release);
                            }
                        }
                    },
                    move |_error: cpal::Error| {
                        error_flag.store(true, Ordering::Release);
                    },
                    None,
                )
            }
            SampleFormat::I16 => {
                let buffer = Arc::clone(&buffer);
                let full = Arc::clone(&at_capacity);
                let error_flag = Arc::clone(&stream_error);
                device.build_input_stream(
                    config,
                    move |data: &[i16], _| {
                        if let Ok(mut destination) = buffer.lock() {
                            append_interleaved(
                                data,
                                channels,
                                &mut destination,
                                capacity,
                                i16_to_normalized,
                            );
                            if destination.len() >= capacity {
                                full.store(true, Ordering::Release);
                            }
                        }
                    },
                    move |_error: cpal::Error| {
                        error_flag.store(true, Ordering::Release);
                    },
                    None,
                )
            }
            SampleFormat::U16 => {
                let buffer = Arc::clone(&buffer);
                let full = Arc::clone(&at_capacity);
                let error_flag = Arc::clone(&stream_error);
                device.build_input_stream(
                    config,
                    move |data: &[u16], _| {
                        if let Ok(mut destination) = buffer.lock() {
                            append_interleaved(
                                data,
                                channels,
                                &mut destination,
                                capacity,
                                u16_to_normalized,
                            );
                            if destination.len() >= capacity {
                                full.store(true, Ordering::Release);
                            }
                        }
                    },
                    move |_error: cpal::Error| {
                        error_flag.store(true, Ordering::Release);
                    },
                    None,
                )
            }
            _ => Err(cpal::Error::from(cpal::ErrorKind::UnsupportedConfig)),
        }
        .map_err(|error: cpal::Error| match sample_format {
            SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16 => {
                CaptureError::BuildStream(error.to_string())
            }
            _ => CaptureError::UnsupportedSampleFormat(other_format_name(sample_format)),
        })?;

        stream
            .play()
            .map_err(|error| CaptureError::StartStream(error.to_string()))?;
        self.stream = Some(stream);
        self.recording = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<AudioClip, CaptureError> {
        if !self.recording {
            return Err(CaptureError::NotRecording);
        }
        // Dropping the stream first guarantees no callback can race buffer extraction.
        drop(self.stream.take());
        self.recording = false;
        if self.stream_error.swap(false, Ordering::AcqRel) {
            self.at_capacity.store(false, Ordering::Release);
            return Err(CaptureError::StreamError);
        }
        let mut buffer = self.buffer.lock().map_err(|_| CaptureError::StreamError)?;
        let samples = buffer.clone();
        buffer.clear();
        let sample_rate = self.sample_rate;
        self.at_capacity.store(false, Ordering::Release);
        Ok(AudioClip::new(sample_rate, samples))
    }

    fn cancel(&mut self) {
        drop(self.stream.take());
        self.recording = false;
        self.stream_error.store(false, Ordering::Release);
        self.at_capacity.store(false, Ordering::Release);
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.clear();
        }
    }
}

fn other_format_name(format: SampleFormat) -> String {
    format!("{format:?}")
}

impl Drop for CpalAudioRecorder {
    fn drop(&mut self) {
        <Self as crate::app::AudioRecorder>::cancel(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmixes_interleaved_samples_without_padding() {
        assert_eq!(
            downmix_f32(&[1.0, -1.0, 0.5, 0.5, 0.25], 2, 8),
            vec![0.0, 0.5]
        );
        assert_eq!(downmix_i16(&[0, 0], 2, 8), vec![0.0]);
        assert_eq!(
            downmix_u16(&[32_768, 32_768, 65_535, 0], 2, 8),
            vec![0.0, -1.5258789e-5]
        );
    }

    #[test]
    fn conversion_is_normalized() {
        assert_eq!(i16_to_normalized(-32_768), -1.0);
        assert_eq!(u16_to_normalized(32_768), 0.0);
        assert!((u16_to_normalized(65_535) - 0.9999695).abs() < 1e-6);
    }

    #[test]
    fn resamples_to_target_rate() {
        let source = vec![0.0f32; 48_000];
        let result = resample_to_16khz(&source, 48_000).unwrap();
        assert!((result.len() as isize - 16_000).abs() <= 2);
    }

    #[test]
    fn rejects_zero_sample_rate() {
        assert_eq!(
            resample_to_16khz(&[1.0], 0),
            Err(CaptureError::InvalidSampleRate)
        );
    }
}
