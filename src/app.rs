use std::time::{Duration, Instant};

use crate::audio::{AudioClip, CaptureError};
use crate::output::OutputError;
use crate::transcription::TranscriptionError;

pub trait AudioRecorder {
    fn start(&mut self) -> Result<(), CaptureError>;
    fn stop(&mut self) -> Result<AudioClip, CaptureError>;
    fn cancel(&mut self);
}

pub trait SpeechTranscriber {
    fn transcribe(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<String, TranscriptionError>;
}

pub trait TextInjector {
    fn type_text(&mut self, text: &str) -> Result<(), OutputError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationState {
    Idle,
    Recording,
    Transcribing,
    Downloading,
    CapturingShortcut,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
}

pub const MAX_RECORDING: Duration = Duration::from_secs(55);

/// The synchronous, testable part of the application coordinator.  The tao
/// event loop below is deliberately a thin adapter around this authority.
pub struct CoordinatorCore<A, S, I> {
    pub recorder: A,
    pub transcriber: S,
    pub injector: I,
    lowercase: bool,
    pub state: OperationState,
    pub last_error: Option<String>,
    started_at: Option<Instant>,
    pressed: bool,
}

impl<A, S, I> CoordinatorCore<A, S, I>
where
    A: AudioRecorder,
    S: SpeechTranscriber,
    I: TextInjector,
{
    pub fn new(recorder: A, transcriber: S, injector: I) -> Self {
        Self {
            recorder,
            transcriber,
            injector,
            state: OperationState::Idle,
            last_error: None,
            lowercase: false,
            started_at: None,
            pressed: false,
        }
    }

    pub fn set_lowercase(&mut self, lowercase: bool) {
        self.lowercase = lowercase;
    }

    /// A duplicate press, an unrelated release, and every event while busy is
    /// ignored.  Only the first press in Idle starts the recorder.
    pub fn pressed(&mut self, now: Instant) {
        if self.state != OperationState::Idle || self.pressed {
            return;
        }
        self.pressed = true;
        match self.recorder.start() {
            Ok(()) => {
                self.state = OperationState::Recording;
                self.started_at = Some(now);
                self.last_error = None;
            }
            Err(error) => self.fail(error.to_string()),
        }
    }

    pub fn released(&mut self) {
        if self.state != OperationState::Recording || !self.pressed {
            return;
        }
        self.pressed = false;
        self.finish_recording();
    }

    pub fn tick(&mut self, now: Instant) {
        if self.state == OperationState::Recording
            && self
                .started_at
                .is_some_and(|start| now.duration_since(start) >= MAX_RECORDING)
        {
            self.pressed = false;
            self.finish_recording();
        }
    }

    fn finish_recording(&mut self) {
        let clip = match self.recorder.stop() {
            Ok(clip) => clip,
            Err(error) => {
                self.fail(error.to_string());
                return;
            }
        };
        self.started_at = None;
        self.state = OperationState::Transcribing;
        let samples = if clip.sample_rate == 16_000 {
            clip.samples.clone()
        } else {
            match crate::audio::resample_to_16khz(&clip.samples, clip.sample_rate) {
                Ok(samples) => samples,
                Err(error) => {
                    self.fail(error.to_string());
                    return;
                }
            }
        };
        if samples.len() < 1_600 {
            self.state = OperationState::Idle;
            return;
        }
        let result = self.transcriber.transcribe(&samples, 16_000);
        match result {
            Ok(text) => {
                if let Some(text) = apply_output_policy(&text, self.lowercase)
                    && let Err(error) = self.injector.type_text(&text)
                {
                    self.fail(error.to_string());
                    return;
                }
                self.state = OperationState::Idle;
                self.last_error = None;
            }
            Err(error) => self.fail(error.to_string()),
        }
    }

    pub fn cancel(&mut self) {
        if self.state == OperationState::Recording {
            self.recorder.cancel();
        }
        self.pressed = false;
        self.started_at = None;
        self.state = OperationState::Idle;
    }

    fn fail(&mut self, error: String) {
        self.recorder.cancel();
        self.pressed = false;
        self.started_at = None;
        self.last_error = Some(error);
        self.state = OperationState::Idle;
    }
}

pub fn apply_output_policy(text: &str, lowercase: bool) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(if lowercase {
        trimmed.to_lowercase()
    } else {
        trimmed.to_owned()
    })
}

/// Application bootstrap and tao event-loop owner.  All mutable UI state is
/// kept inside the event-loop closure; worker threads must communicate through
/// channels and never touch tray/menu objects.
enum WorkerInput {
    Clip(AudioClip, crate::settings::Engine),
}

enum WorkerEvent {
    Transcribed(Result<String, TranscriptionError>),
}

enum WorkerTranscriber {
    Parakeet(crate::transcription::ParakeetTranscriber),
    #[cfg(target_os = "macos")]
    Apple(crate::transcription::AppleSpeechTranscriber),
}

impl SpeechTranscriber for WorkerTranscriber {
    fn transcribe(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<String, TranscriptionError> {
        match self {
            Self::Parakeet(transcriber) => transcriber.transcribe(samples, sample_rate),
            #[cfg(target_os = "macos")]
            Self::Apple(transcriber) => transcriber.transcribe(samples, sample_rate),
        }
    }
}

fn run_worker(
    receiver: crossbeam_channel::Receiver<WorkerInput>,
    sender: crossbeam_channel::Sender<WorkerEvent>,
    model_dir: Option<std::path::PathBuf>,
) {
    let mut cached: Option<(crate::settings::Engine, WorkerTranscriber)> = None;
    while let Ok(WorkerInput::Clip(clip, engine)) = receiver.recv() {
        if cached
            .as_ref()
            .is_none_or(|(current, _)| *current != engine)
        {
            let transcriber = match engine {
                crate::settings::Engine::Parakeet => {
                    let Some(path) = model_dir.as_deref() else {
                        let _ = sender.send(WorkerEvent::Transcribed(Err(
                            TranscriptionError::ModelNotInstalled,
                        )));
                        continue;
                    };
                    match crate::transcription::ParakeetTranscriber::from_model_dir(path) {
                        Ok(value) => WorkerTranscriber::Parakeet(value),
                        Err(error) => {
                            let _ = sender.send(WorkerEvent::Transcribed(Err(error)));
                            continue;
                        }
                    }
                }
                #[cfg(target_os = "macos")]
                crate::settings::Engine::AppleSpeech => {
                    match crate::transcription::AppleSpeechTranscriber::new() {
                        Ok(value) => WorkerTranscriber::Apple(value),
                        Err(error) => {
                            let _ = sender.send(WorkerEvent::Transcribed(Err(error)));
                            continue;
                        }
                    }
                }
                #[cfg(not(target_os = "macos"))]
                crate::settings::Engine::AppleSpeech => {
                    let _ = sender.send(WorkerEvent::Transcribed(Err(
                        TranscriptionError::Recognition(
                            "Apple Speech is not supported on Linux".into(),
                        ),
                    )));
                    continue;
                }
            };
            cached = Some((engine, transcriber));
        }
        let samples = match clip.resample_to_16khz() {
            Ok(samples) => samples,
            Err(error) => {
                let _ = sender.send(WorkerEvent::Transcribed(Err(
                    TranscriptionError::Recognition(error.to_string()),
                )));
                continue;
            }
        };
        if samples.len() < 1_600 {
            let _ = sender.send(WorkerEvent::Transcribed(Ok(String::new())));
            continue;
        }
        let result = cached
            .as_mut()
            .expect("worker cache initialized")
            .1
            .transcribe(&samples, 16_000);
        let _ = sender.send(WorkerEvent::Transcribed(result));
    }
}

pub struct Coordinator;

impl Coordinator {
    pub fn run() -> Result<(), AppError> {
        #[cfg(target_os = "linux")]
        crate::platform::require_x11_session()
            .map_err(|error| AppError::Message(error.to_owned()))?;

        use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
        use tao::{
            dpi::LogicalSize,
            event::{Event, WindowEvent},
            event_loop::{ControlFlow, EventLoopBuilder},
            window::WindowBuilder,
        };

        let event_loop = EventLoopBuilder::<UiEvent>::with_user_event().build();
        let proxy = event_loop.create_proxy();
        tray_icon::menu::MenuEvent::set_event_handler(Some(move |event| {
            let _ = proxy.send_event(UiEvent::Menu(event));
        }));
        let proxy = event_loop.create_proxy();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
            let _ = proxy.send_event(UiEvent::Tray(event));
        }));
        let hotkeys = GlobalHotKeyManager::new().map_err(|e| AppError::Message(e.to_string()))?;
        let loaded = crate::settings::load().map_err(|e| AppError::Message(e.to_string()))?;
        let mut settings = loaded.settings;
        let mut last_error: Option<String> = loaded.error.map(|e| e.to_string());
        let mut registered = settings.shortcut.and_then(|shortcut| {
            crate::hotkey::RegisteredShortcut::register(&hotkeys, shortcut).ok()
        });
        if settings.shortcut.is_some() && registered.is_none() {
            last_error = Some("could not register saved shortcut".into());
        }
        let mut has_shortcut = registered.is_some();
        let engine_apple = matches!(settings.engine, crate::settings::Engine::AppleSpeech);
        let mut installer = crate::models::ParakeetInstaller::new().ok();
        let mut parakeet_ready = installer.as_ref().is_some_and(|value| value.is_ready());
        let tray = crate::tray::Tray::new(engine_apple, settings.lowercase, parakeet_ready)
            .map_err(AppError::Message)?;
        let mut recorder = crate::audio::CpalAudioRecorder::new();
        let (worker_tx, worker_rx) = crossbeam_channel::unbounded::<WorkerInput>();
        let (result_tx, result_rx) = crossbeam_channel::unbounded::<WorkerEvent>();
        let model_dir = installer
            .as_ref()
            .map(|installer| installer.model_dir().to_path_buf());
        std::thread::spawn(move || run_worker(worker_rx, result_tx, model_dir));
        let mut state = OperationState::Idle;
        let mut lowercase = settings.lowercase;
        let mut started_at: Option<Instant> = None;
        let mut pressed = false;
        let mut shortcut_window: Option<tao::window::Window> = None;
        let mut shortcut_recorder = crate::hotkey::ShortcutRecorder::new();
        let mut download_progress: Option<crate::models::DownloadProgress> = None;
        let download_proxy = event_loop.create_proxy();
        event_loop.run(move |event, target, control_flow| {
            *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(30));
            match event {
                Event::MainEventsCleared => {
                    for result in result_rx.try_iter() {
                        if state != OperationState::Transcribing {
                            continue;
                        }
                        match result {
                            WorkerEvent::Transcribed(Ok(text)) => {
                                if let Some(text) = apply_output_policy(&text, lowercase) {
                                    match crate::output::EnigoTextInjector::new()
                                        .and_then(|mut injector| injector.type_text(&text))
                                    {
                                        Ok(()) => last_error = None,
                                        Err(error) => last_error = Some(error.to_string()),
                                    }
                                } else {
                                    last_error = None;
                                }
                            }
                            WorkerEvent::Transcribed(Err(error)) => {
                                last_error = Some(error.to_string())
                            }
                        }
                        state = OperationState::Idle;
                    }
                    let native_right_control_id = registered.as_ref().and_then(|binding| {
                        (binding.shortcut().key == crate::hotkey::KeyName::ControlRight)
                            .then_some(binding.id())
                    });
                    for hotkey in GlobalHotKeyEvent::receiver().try_iter().chain(
                        crate::hotkey::right_control_receiver()
                            .try_iter()
                            .filter_map(move |state| {
                                native_right_control_id.map(|id| GlobalHotKeyEvent { id, state })
                            }),
                    ) {
                        let is_expected = registered
                            .as_ref()
                            .is_some_and(|binding| binding.id() == hotkey.id);
                        match hotkey.state {
                            HotKeyState::Pressed
                                if is_expected && !pressed && state == OperationState::Idle =>
                            {
                                if settings.engine == crate::settings::Engine::Parakeet
                                    && !parakeet_ready
                                {
                                    last_error = Some(
                                        "Parakeet is not installed; choose Download model".into(),
                                    );
                                } else {
                                    match AudioRecorder::start(&mut recorder) {
                                        Ok(()) => {
                                            pressed = true;
                                            started_at = Some(Instant::now());
                                            state = OperationState::Recording;
                                            last_error = None;
                                        }
                                        Err(error) => last_error = Some(error.to_string()),
                                    }
                                }
                            }
                            HotKeyState::Released
                                if is_expected && pressed && state == OperationState::Recording =>
                            {
                                pressed = false;
                                started_at = None;
                                match AudioRecorder::stop(&mut recorder) {
                                    Ok(clip) if clip.samples.len() < 1_600 => {
                                        state = OperationState::Idle
                                    }
                                    Ok(clip) => {
                                        state = OperationState::Transcribing;
                                        if worker_tx
                                            .send(WorkerInput::Clip(clip, settings.engine))
                                            .is_err()
                                        {
                                            state = OperationState::Idle;
                                            last_error = Some("recognition worker stopped".into());
                                        }
                                    }
                                    Err(error) => {
                                        state = OperationState::Idle;
                                        last_error = Some(error.to_string());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    if state == OperationState::Recording
                        && started_at.is_some_and(|started| started.elapsed() >= MAX_RECORDING)
                    {
                        pressed = false;
                        started_at = None;
                        match AudioRecorder::stop(&mut recorder) {
                            Ok(clip) if clip.samples.len() < 1_600 => state = OperationState::Idle,
                            Ok(clip) => {
                                state = OperationState::Transcribing;
                                if worker_tx
                                    .send(WorkerInput::Clip(clip, settings.engine))
                                    .is_err()
                                {
                                    state = OperationState::Idle;
                                    last_error = Some("recognition worker stopped".into());
                                }
                            }
                            Err(error) => {
                                state = OperationState::Idle;
                                last_error = Some(error.to_string());
                            }
                        }
                    }
                    tray.update(
                        state,
                        last_error.as_deref(),
                        parakeet_ready,
                        has_shortcut,
                        download_progress,
                    );
                }
                Event::UserEvent(UiEvent::DownloadProgress { downloaded, total }) => {
                    if state == OperationState::Downloading {
                        download_progress =
                            Some(crate::models::DownloadProgress::new(downloaded, total));
                    }
                }
                Event::UserEvent(UiEvent::DownloadFinished {
                    installer: completed,
                    result,
                }) => {
                    installer = Some(completed);
                    state = OperationState::Idle;
                    download_progress = None;
                    match result {
                        Ok(()) => {
                            parakeet_ready = true;
                            last_error = None;
                        }
                        Err(error) => {
                            parakeet_ready = false;
                            last_error = Some(error);
                        }
                    }
                }
                Event::UserEvent(UiEvent::Menu(event)) => {
                    let id = event.id;
                    if id == crate::tray::ID_SHORTCUT {
                        if state == OperationState::Idle {
                            match WindowBuilder::new()
                                .with_title(
                                    "Hold a shortcut: Control/Option/Shift/Command + key, or Right Control (Esc cancels)",
                                )
                                .with_inner_size(LogicalSize::new(360.0, 100.0))
                                .with_resizable(false)
                                .with_always_on_top(true)
                                .build(target)
                            {
                                Ok(window) => {
                                    shortcut_window = Some(window);
                                    shortcut_recorder = crate::hotkey::ShortcutRecorder::new();
                                    state = OperationState::CapturingShortcut;
                                }
                                Err(error) => last_error = Some(error.to_string()),
                            }
                        }
                    } else if id == crate::tray::ID_LOWERCASE {
                        if state == OperationState::Idle {
                            lowercase = !lowercase;
                            settings.lowercase = lowercase;
                            if let Err(error) = crate::settings::save(&settings) {
                                last_error = Some(error.to_string());
                            }
                        }
                    } else if id == crate::tray::ID_ENGINE_APPLE {
                        #[cfg(target_os = "macos")]
                        if state == OperationState::Idle {
                            settings.engine = crate::settings::Engine::AppleSpeech;
                            if let Err(error) = crate::settings::save(&settings) {
                                last_error = Some(error.to_string());
                            }
                        }
                    } else if id == crate::tray::ID_ENGINE_PARAKEET {
                        if state == OperationState::Idle {
                            settings.engine = crate::settings::Engine::Parakeet;
                            if let Err(error) = crate::settings::save(&settings) {
                                last_error = Some(error.to_string());
                            }
                        }
                    } else if id == crate::tray::ID_PERMISSIONS && state == OperationState::Idle {
                        #[cfg(target_os = "macos")]
                        {
                            let (microphone, speech, accessibility) =
                                crate::platform::permission_summary();
                            if !microphone.is_authorized() {
                                let _ = crate::platform::request_microphone();
                            }
                            if matches!(settings.engine, crate::settings::Engine::AppleSpeech)
                                && !speech.is_authorized()
                            {
                                let _ = crate::platform::request_speech();
                            }
                            if !accessibility {
                                let _ = crate::platform::request_accessibility();
                            }
                            let (microphone, speech, accessibility) =
                                crate::platform::permission_summary();
                            if !microphone.is_authorized() {
                                last_error =
                                    Some("Microphone permission is required to record.".into());
                            } else if matches!(
                                settings.engine,
                                crate::settings::Engine::AppleSpeech
                            ) && !speech.is_authorized()
                            {
                                last_error = Some(
                                    "Speech Recognition permission is required for Apple Speech."
                                        .into(),
                                );
                            } else if !accessibility {
                                last_error =
                                    Some(crate::platform::ACCESSIBILITY_PERMISSION_ERROR.into());
                            } else {
                                last_error = None;
                            }
                        }
                    } else if id == crate::tray::ID_DOWNLOAD && state == OperationState::Idle {
                        state = OperationState::Downloading;
                        download_progress = Some(crate::models::DownloadProgress::new(
                            0,
                            crate::models::PARAKEET_MANIFEST.length,
                        ));
                        if let Some(mut installer_value) = installer.take() {
                            let proxy = download_proxy.clone();
                            let progress_proxy = proxy.clone();
                            std::thread::spawn(move || {
                                let mut last_report = Instant::now() - Duration::from_secs(1);
                                let result = installer_value
                                    .download_with_progress(|downloaded, total| {
                                        if downloaded == total
                                            || last_report.elapsed() >= Duration::from_millis(100)
                                        {
                                            let _ = progress_proxy.send_event(
                                                UiEvent::DownloadProgress { downloaded, total },
                                            );
                                            last_report = Instant::now();
                                        }
                                    })
                                    .map_err(|error| error.to_string());
                                let _ = proxy.send_event(UiEvent::DownloadFinished {
                                    installer: installer_value,
                                    result,
                                });
                            });
                        } else {
                            state = OperationState::Idle;
                            download_progress = None;
                            last_error = Some("Lavtype data directory is unavailable".into());
                        }
                    } else if id == crate::tray::ID_QUIT {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                Event::WindowEvent {
                    event, window_id, ..
                } => {
                    if shortcut_window
                        .as_ref()
                        .is_some_and(|window| window.id() == window_id)
                    {
                        match shortcut_recorder.handle_window_event(&event) {
                            crate::hotkey::RecorderAction::Completed(shortcut) => {
                                let registration = if let Some(binding) = registered.as_mut() {
                                    binding.replace(&hotkeys, shortcut)
                                } else {
                                    crate::hotkey::RegisteredShortcut::register(&hotkeys, shortcut)
                                        .map(|binding| {
                                            registered = Some(binding);
                                        })
                                };
                                match registration {
                                    Ok(()) => {
                                        settings.shortcut = Some(shortcut);
                                        has_shortcut = true;
                                        if let Err(error) = crate::settings::save(&settings) {
                                            last_error = Some(error.to_string());
                                        } else {
                                            last_error = None;
                                        }
                                    }
                                    Err(error) => last_error = Some(error.to_string()),
                                }
                                shortcut_window = None;
                                state = OperationState::Idle;
                            }
                            crate::hotkey::RecorderAction::Cancelled => {
                                shortcut_window = None;
                                state = OperationState::Idle;
                            }
                            _ => {}
                        }
                    }
                }
                Event::LoopDestroyed => {
                    tray_icon::menu::MenuEvent::set_event_handler(
                        None::<fn(tray_icon::menu::MenuEvent)>,
                    );
                    tray_icon::TrayIconEvent::set_event_handler(
                        None::<fn(tray_icon::TrayIconEvent)>,
                    );
                }
                _ => {}
            }
        })
    }
}

enum UiEvent {
    Menu(tray_icon::menu::MenuEvent),
    Tray(tray_icon::TrayIconEvent),
    DownloadFinished {
        installer: crate::models::ParakeetInstaller,
        result: Result<(), String>,
    },
    DownloadProgress {
        downloaded: u64,
        total: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    struct FakeRecorder {
        starts: usize,
        stops: usize,
        clip: AudioClip,
    }
    impl AudioRecorder for FakeRecorder {
        fn start(&mut self) -> Result<(), CaptureError> {
            self.starts += 1;
            Ok(())
        }
        fn stop(&mut self) -> Result<AudioClip, CaptureError> {
            self.stops += 1;
            Ok(self.clip.clone())
        }
        fn cancel(&mut self) {}
    }
    struct FakeSpeech {
        calls: usize,
        text: String,
        fails: bool,
    }
    impl SpeechTranscriber for FakeSpeech {
        fn transcribe(&mut self, _: &[f32], _: u32) -> Result<String, TranscriptionError> {
            self.calls += 1;
            if self.fails {
                Err(TranscriptionError::Recognition("fake failure".into()))
            } else {
                Ok(self.text.clone())
            }
        }
    }
    struct FakeInjector {
        calls: usize,
        output: String,
    }
    impl TextInjector for FakeInjector {
        fn type_text(&mut self, text: &str) -> Result<(), OutputError> {
            self.calls += 1;
            self.output = text.into();
            Ok(())
        }
    }
    fn core(text: &str) -> CoordinatorCore<FakeRecorder, FakeSpeech, FakeInjector> {
        CoordinatorCore::new(
            FakeRecorder {
                starts: 0,
                stops: 0,
                clip: AudioClip {
                    sample_rate: 16_000,
                    samples: vec![0.0; 1_600],
                },
            },
            FakeSpeech {
                calls: 0,
                text: text.into(),
                fails: false,
            },
            FakeInjector {
                calls: 0,
                output: String::new(),
            },
        )
    }
    #[test]
    fn press_release_transcribes_once() {
        let now = Instant::now();
        let mut c = core(" hello ");
        c.pressed(now);
        c.pressed(now);
        c.released();
        c.released();
        assert_eq!(c.recorder.starts, 1);
        assert_eq!(c.recorder.stops, 1);
        assert_eq!(c.transcriber.calls, 1);
        assert_eq!(c.injector.calls, 1);
        assert_eq!(c.injector.output, "hello");
    }
    #[test]
    fn transcription_failure_recovers_to_idle() {
        let mut c = core("ignored");
        c.transcriber.fails = true;
        c.pressed(Instant::now());
        c.released();
        assert_eq!(c.state, OperationState::Idle);
        assert!(c.last_error.is_some());
        assert_eq!(c.injector.calls, 0);
    }
    #[test]
    fn cap_auto_stops_and_ignores_late_release() {
        let now = Instant::now();
        let mut c = core(" capped ");
        c.pressed(now);
        c.tick(now + MAX_RECORDING);
        c.released();
        assert_eq!(c.recorder.starts, 1);
        assert_eq!(c.recorder.stops, 1);
        assert_eq!(c.transcriber.calls, 1);
        assert_eq!(c.injector.calls, 1);
    }
    #[test]
    fn lowercase_output_is_typed() {
        let mut c = core(" Hello, LAVTYPE! ");
        c.set_lowercase(true);
        c.pressed(Instant::now());
        c.released();
        assert_eq!(c.injector.output, "hello, lavtype!");
        assert_eq!(
            apply_output_policy(" Hello, LAVTYPE! ", false).as_deref(),
            Some("Hello, LAVTYPE!")
        );
        assert_eq!(apply_output_policy("  \n", true), None);
    }
}
