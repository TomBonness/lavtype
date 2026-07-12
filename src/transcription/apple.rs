use std::time::Duration;

use block2::RcBlock;
use crossbeam_channel::{Receiver, bounded};
use hound::{SampleFormat, WavSpec, WavWriter};
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_foundation::{NSError, NSString, NSURL};
use objc2_speech::{
    SFSpeechRecognitionResult, SFSpeechRecognitionTask, SFSpeechRecognitionTaskHint,
    SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus, SFSpeechURLRecognitionRequest,
};
use tempfile::NamedTempFile;

use super::{Transcriber, TranscriptionError};

const ON_DEVICE_UNAVAILABLE: &str = "On-device Apple Speech is unavailable for the current system language; choose Parakeet or install macOS dictation support.";

#[derive(Debug)]
enum CallbackResult {
    Final(String),
    Error,
}
type RecognitionCallback = RcBlock<dyn Fn(*mut SFSpeechRecognitionResult, *mut NSError)>;
type RequestLifetime = (
    Receiver<CallbackResult>,
    Retained<SFSpeechURLRecognitionRequest>,
    RecognitionCallback,
    Retained<SFSpeechRecognitionTask>,
);

/// On-device Apple Speech transcriber.
///
/// The recognizer is retained by this object; each request additionally retains
/// its URL request, callback block, task, and temporary WAV until completion.
pub struct AppleSpeechTranscriber {
    recognizer: Retained<SFSpeechRecognizer>,
}

impl std::fmt::Debug for AppleSpeechTranscriber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppleSpeechTranscriber")
            .finish_non_exhaustive()
    }
}

impl AppleSpeechTranscriber {
    /// Construct a recognizer for the system's current dictation language.
    pub fn new() -> Result<Self, TranscriptionError> {
        let status = unsafe { SFSpeechRecognizer::authorizationStatus() };
        if status != SFSpeechRecognizerAuthorizationStatus::Authorized {
            return Err(TranscriptionError::ApplePermissionDenied);
        }
        let recognizer = unsafe { SFSpeechRecognizer::init(SFSpeechRecognizer::alloc()) }
            .ok_or(TranscriptionError::AppleUnavailable)?;
        let available = unsafe { recognizer.isAvailable() };
        let on_device = unsafe { recognizer.supportsOnDeviceRecognition() };
        if !available || !on_device {
            return Err(TranscriptionError::AppleUnavailable);
        }
        Ok(Self { recognizer })
    }

    fn write_wav(samples: &[f32]) -> Result<NamedTempFile, TranscriptionError> {
        let mut file = NamedTempFile::new()
            .map_err(|error| TranscriptionError::AudioFile(error.to_string()))?;
        let spec = WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::new(file.as_file_mut(), spec)
            .map_err(|error| TranscriptionError::AudioFile(error.to_string()))?;
        for &sample in samples {
            let sample = sample.clamp(-1.0, 1.0);
            let pcm = (sample * i16::MAX as f32).round() as i16;
            writer
                .write_sample(pcm)
                .map_err(|error| TranscriptionError::AudioFile(error.to_string()))?;
        }
        writer
            .finalize()
            .map_err(|error| TranscriptionError::AudioFile(error.to_string()))?;
        Ok(file)
    }

    fn start_request(&self, wav: &NamedTempFile) -> Result<RequestLifetime, TranscriptionError> {
        let path = wav.path().to_string_lossy();
        let path_string = NSString::from_str(&path);
        let url = NSURL::fileURLWithPath(&path_string);
        let request = unsafe {
            SFSpeechURLRecognitionRequest::initWithURL(SFSpeechURLRecognitionRequest::alloc(), &url)
        };
        unsafe {
            request.setRequiresOnDeviceRecognition(true);
            request.setShouldReportPartialResults(false);
            request.setTaskHint(SFSpeechRecognitionTaskHint::Dictation);
        }
        let (sender, receiver) = bounded(1);
        let callback: RcBlock<dyn Fn(*mut SFSpeechRecognitionResult, *mut NSError)> =
            RcBlock::<dyn Fn(*mut SFSpeechRecognitionResult, *mut NSError)>::new(
                move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                    if !error.is_null() {
                        let _ = sender.send(CallbackResult::Error);
                        return;
                    }
                    // Callback pointers are valid for the duration of the callback.
                    let Some(result) = (unsafe { result.as_ref() }) else {
                        let _ = sender.send(CallbackResult::Error);
                        return;
                    };
                    if !unsafe { result.isFinal() } {
                        return;
                    }
                    let transcription = unsafe { result.bestTranscription() };
                    let text = unsafe { transcription.formattedString() }.to_string();
                    let _ = sender.send(CallbackResult::Final(text));
                },
            );
        let task = unsafe {
            self.recognizer
                .recognitionTaskWithRequest_resultHandler(&request, &callback)
        };
        Ok((receiver, request, callback, task))
    }

    fn transcribe_inner(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<String, TranscriptionError> {
        if sample_rate != 16_000 {
            return Err(TranscriptionError::InvalidSampleRate(sample_rate));
        }
        let wav = Self::write_wav(samples)?;
        let (receiver, _request, _callback, task) = self.start_request(&wav)?;
        await_result(receiver, Duration::from_secs(60), || unsafe {
            task.cancel()
        })
    }

    /// The exact user-facing availability message used by the tray.
    pub const fn unavailable_message() -> &'static str {
        ON_DEVICE_UNAVAILABLE
    }
}

fn await_result(
    receiver: Receiver<CallbackResult>,
    timeout: Duration,
    mut cancel: impl FnMut(),
) -> Result<String, TranscriptionError> {
    match receiver.recv_timeout(timeout) {
        Ok(CallbackResult::Final(text)) => Ok(text),
        Ok(CallbackResult::Error) => {
            cancel();
            Err(TranscriptionError::AppleRecognition(
                "the on-device recognizer reported an error".to_owned(),
            ))
        }
        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
            cancel();
            Err(TranscriptionError::AppleTimeout)
        }
        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
            cancel();
            Err(TranscriptionError::AppleRecognition(
                "recognition callback disconnected".to_owned(),
            ))
        }
    }
}
impl Transcriber for AppleSpeechTranscriber {
    fn transcribe(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<String, TranscriptionError> {
        self.transcribe_inner(samples, sample_rate)
    }
}

impl crate::app::SpeechTranscriber for AppleSpeechTranscriber {
    fn transcribe(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<String, TranscriptionError> {
        self.transcribe_inner(samples, sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_result_is_returned_without_cancellation() {
        let (sender, receiver) = bounded(1);
        sender
            .send(CallbackResult::Final("hello".to_owned()))
            .unwrap();
        let canceled = std::sync::atomic::AtomicBool::new(false);
        let result = await_result(receiver, Duration::from_millis(10), || {
            canceled.store(true, std::sync::atomic::Ordering::SeqCst)
        });
        assert_eq!(result.unwrap(), "hello");
        assert!(!canceled.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn error_and_timeout_cancel_and_release_lifetime() {
        let (sender, receiver) = bounded(1);
        sender.send(CallbackResult::Error).unwrap();
        let canceled = std::sync::atomic::AtomicBool::new(false);
        let result = await_result(receiver, Duration::from_millis(10), || {
            canceled.store(true, std::sync::atomic::Ordering::SeqCst)
        });
        assert!(matches!(
            result,
            Err(TranscriptionError::AppleRecognition(_))
        ));
        assert!(canceled.load(std::sync::atomic::Ordering::SeqCst));

        let (_sender, receiver) = bounded(1);
        let canceled = std::sync::atomic::AtomicBool::new(false);
        let result = await_result(receiver, Duration::from_millis(0), || {
            canceled.store(true, std::sync::atomic::Ordering::SeqCst)
        });
        assert!(matches!(result, Err(TranscriptionError::AppleTimeout)));
        assert!(canceled.load(std::sync::atomic::Ordering::SeqCst));
    }
}
