use std::path::{Path, PathBuf};

use lavtype::models::ParakeetInstaller;
use lavtype::transcription::ParakeetTranscriber;
use tempfile::tempdir;

const EXPECTED: &str = "Well, I don't wish to see it any more, observed Phebe, turning away her eyes. It is certainly very like the old portrait.";

fn find_test_wav(root: &Path) -> Option<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let entries = std::fs::read_dir(path).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().and_then(|name| name.to_str()) == Some("0.wav")
                && path
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    == Some("test_wavs")
            {
                return Some(path);
            }
            if path.is_dir() {
                pending.push(path);
            }
        }
    }
    None
}

#[test]
#[ignore = "downloads the pinned 460 MiB model; run with LAVTYPE_MODEL_E2E=1"]
fn parakeet_production_model_is_offline_and_exact() {
    if std::env::var("LAVTYPE_MODEL_E2E").as_deref() != Ok("1") {
        eprintln!("set LAVTYPE_MODEL_E2E=1 to run the real-model contract");
        return;
    }
    let data = tempdir().expect("temporary model directory");
    let mut installer = ParakeetInstaller::from_data_dir(data.path()).expect("model installer");
    installer
        .download()
        .expect("pinned model download and verification");
    let wav_path = find_test_wav(data.path()).expect("test_wavs/0.wav in downloaded model archive");
    let mut reader = hound::WavReader::open(wav_path).expect("test wav");
    let spec = reader.spec();
    assert_eq!(spec.channels, 1);
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|sample| sample.expect("valid PCM sample") as f32 / 32_768.0)
        .collect();
    let mut transcriber =
        ParakeetTranscriber::from_model_dir(installer.model_dir()).expect("load installed model");
    let result =
        lavtype::app::SpeechTranscriber::transcribe(&mut transcriber, &samples, spec.sample_rate)
            .expect("first offline recognition");
    assert_eq!(result, EXPECTED);
    let repeated =
        lavtype::app::SpeechTranscriber::transcribe(&mut transcriber, &samples, spec.sample_rate)
            .expect("repeat offline recognition with network disabled");
    assert_eq!(repeated, EXPECTED);
}
