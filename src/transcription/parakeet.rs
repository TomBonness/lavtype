use std::path::{Path, PathBuf};

use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};

use super::{Transcriber, TranscriptionError, model_files_present};

/// Cached, single-model Parakeet recognizer.
///
/// Construction is deliberately explicit: selecting Parakeet never downloads a
/// model and callers must pass the already-installed model directory.
pub struct ParakeetTranscriber {
    recognizer: OfflineRecognizer,
    model_dir: PathBuf,
}

impl std::fmt::Debug for ParakeetTranscriber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParakeetTranscriber")
            .field("model_dir", &self.model_dir)
            .finish_non_exhaustive()
    }
}

impl ParakeetTranscriber {
    /// Create and cache one recognizer from a verified model directory.
    pub fn from_model_dir(path: impl AsRef<Path>) -> Result<Self, TranscriptionError> {
        let model_dir = path.as_ref().to_path_buf();
        if !model_files_present(&model_dir) {
            return Err(TranscriptionError::ModelNotInstalled);
        }

        let mut config = OfflineRecognizerConfig::default();
        config.model_config.transducer = OfflineTransducerModelConfig {
            encoder: Some(
                model_dir
                    .join("encoder.int8.onnx")
                    .to_string_lossy()
                    .into_owned(),
            ),
            decoder: Some(
                model_dir
                    .join("decoder.int8.onnx")
                    .to_string_lossy()
                    .into_owned(),
            ),
            joiner: Some(
                model_dir
                    .join("joiner.int8.onnx")
                    .to_string_lossy()
                    .into_owned(),
            ),
        };
        config.model_config.tokens =
            Some(model_dir.join("tokens.txt").to_string_lossy().into_owned());
        config.model_config.model_type = Some("nemo_transducer".to_owned());
        config.model_config.provider = Some("cpu".to_owned());
        config.model_config.num_threads = 2;
        config.decoding_method = Some("greedy_search".to_owned());

        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
            TranscriptionError::ModelLoad(format!(
                "failed to load model from {}",
                model_dir.display()
            ))
        })?;
        Ok(Self {
            recognizer,
            model_dir,
        })
    }

    /// Alias retained as the conventional constructor for callers.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, TranscriptionError> {
        Self::from_model_dir(path)
    }

    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    fn transcribe_inner(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<String, TranscriptionError> {
        if sample_rate != 16_000 {
            return Err(TranscriptionError::InvalidSampleRate(sample_rate));
        }
        let stream = self.recognizer.create_stream();
        stream.accept_waveform(16_000, samples);
        self.recognizer.decode(&stream);
        stream
            .get_result()
            .map(|result| result.text)
            .ok_or(TranscriptionError::MissingResult)
    }
}

impl Transcriber for ParakeetTranscriber {
    fn transcribe(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<String, TranscriptionError> {
        self.transcribe_inner(samples, sample_rate)
    }
}

impl crate::app::SpeechTranscriber for ParakeetTranscriber {
    fn transcribe(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<String, TranscriptionError> {
        self.transcribe_inner(samples, sample_rate)
    }
}
