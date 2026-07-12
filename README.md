# Lavtype

Lavtype is a small, tray-only push-to-talk dictation utility for macOS and Linux. Hold one configurable shortcut, speak, release, and Lavtype types one transcript into the application that was focused when recognition finishes. It has no dashboard, history, clipboard fallback, cloud recognizer, or background service.

## What is included

- One press-and-hold shortcut (there is no default shortcut).
- Bounded microphone capture (16 kHz mono recognition input, at most 55 seconds per hold).
- Local English **Parakeet** recognition through sherpa-onnx, downloaded only after an explicit menu action.
- On macOS, optional on-device **Apple Speech**. Lavtype forces on-device recognition and never falls back to Apple’s network recognizer.
- Optional Unicode lowercase transformation, applied once to the complete trimmed transcript.
- A single native menu-bar/status-tray icon and native About metadata.

The Linux release supports **X11/Xorg only**. A Wayland or XWayland session is rejected intentionally; Lavtype does not use portals, `ydotool`, an input group, or root privileges.

## Install a release

Download the artifact for your architecture from the GitHub release:

- macOS 13 or newer: separate unsigned/ad-hoc-signed `arm64` and `x86_64` DMGs.
- Linux: one x86_64 GNU AppImage.

Every release includes `SHA256SUMS`. Verify it before opening an artifact:

```sh
shasum -a 256 -c SHA256SUMS
```

### macOS first launch

The initial DMGs are not notarized. Drag **Lavtype.app** to Applications, then Control-click (or right-click) it and choose **Open** once. Confirm the dialog to allow the unsigned application. Lavtype is a background app and intentionally does not create a Dock icon.

In **Permissions…**, grant Microphone and, when using Apple Speech, Speech Recognition. Grant Accessibility in System Settings > Privacy & Security > Accessibility so Lavtype can type into the focused application. Denied permissions leave the tray usable and report an actionable error; they never enable a network recognizer. Lavtype does not request Input Monitoring.

The app uses these exact macOS usage explanations:

- `Lavtype records while you hold the dictation shortcut.`
- `Lavtype uses Apple Speech to turn your recording into text.`

### Linux host requirements

The AppImage bundles its application/runtime libraries, but the host must provide a working desktop session and audio/input stack:

- Ubuntu 22.04 or a compatible x86_64 GNU Linux system.
- **Xorg/X11** with `DISPLAY` set. `XDG_SESSION_TYPE=wayland`, `WAYLAND_DISPLAY`, or a missing `DISPLAY` is refused, including XWayland sessions.
- An AppIndicator/status-notifier host (for example, a GNOME AppIndicator extension). Without a panel host there is no supported permanent window or tooltip fallback.
- A working ALSA/PulseAudio/PipeWire ALSA input device.
- FUSE 2 (`libfuse2`) for normal AppImage mounting. If FUSE cannot be installed, run the AppImage with `--appimage-extract-and-run`.

The Linux menu has no Apple Speech choice. It shows only **Parakeet (local, English)**. Install the model from the tray before the first dictation; selecting Parakeet or pressing the shortcut never silently downloads it.

## Use

1. Launch Lavtype. The status row says **Set a shortcut to start** until a binding exists.
2. Choose **Set Push-to-Talk Shortcut…**, then press a non-media key with Control, Alt, Shift, or Meta; standalone F1–F12 are also accepted. Escape cancels. A replacement is transactional: if registration fails, the old shortcut and settings remain active.
3. On macOS choose **Apple Speech (on-device)** or **Parakeet (local, English)**. Linux has only Parakeet.
4. For Parakeet, choose **Download model (460 MiB)**. The download is approximately 460 MiB (`482,468,385` bytes), requires 1.2 GiB free, and is SHA-256 verified before an atomic install. Interrupted, invalid, or stale partial data is removed and the same menu action retries.
5. Focus the destination application, hold the shortcut while speaking, and release. Clips shorter than 100 ms type nothing. Lavtype trims outer whitespace, optionally lowercases the complete Unicode string, and types it directly with accessibility/keyboard APIs. It never copies to the clipboard and never types a partial result.
6. Use **Lowercase output** to make all letters lowercase while retaining punctuation. **Quit** exits the tray application.

The Parakeet model is NVIDIA `nvidia/parakeet-tdt-0.6b-v2`, English with punctuation/capitalization, distributed as `sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2`. It is licensed CC-BY-4.0. The model is stored under the platform data directory in `models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8`; recognition is offline after installation.

## Build from source

Rust 1.92.0 is pinned by `rust-toolchain.toml`. Install a C/C++ toolchain, `pkg-config`, and CMake, then install the platform development headers.

### macOS

Use macOS 13 or newer with Xcode Command Line Tools. Build and package one architecture on its native runner:

```sh
cargo install cargo-packager --version 0.11.8 --locked
rustup target add aarch64-apple-darwin # or x86_64-apple-darwin
./scripts/release.sh macos arm64   # or: ./scripts/release.sh macos x86_64
```

### Linux (release target)

Build on Ubuntu 22.04 x86_64. Install GTK3, Ayatana AppIndicator, ALSA, XKB, FUSE, `pkg-config`, and CMake development packages (the workflow installs the exact CI package set), then run:

```sh
cargo install cargo-packager --version 0.11.8 --locked
rustup target add x86_64-unknown-linux-gnu
./scripts/release.sh linux x86_64
```

The script prefetches the pinned sherpa-onnx 1.13.4 native archive, verifies its SHA-256, exports `SHERPA_ONNX_ARCHIVE_DIR`, builds the release binary, packages an AppImage, and writes `SHA256SUMS`. It does not download the Parakeet model; that is an explicit in-app action.

## Release engineering

GitHub Actions builds on macOS arm64 and x86_64 and Ubuntu 22.04 x86_64. The native sherpa archives are fetched from the v1.13.4 GitHub release and checked before Cargo runs:

| target | archive | SHA-256 |
| --- | --- | --- |
| Linux x86_64 | `sherpa-onnx-v1.13.4-linux-x64-static-lib.tar.bz2` | `98b0e31996426f6e78244dbce1955548f2c64e8f01c4be75b85af7cdaa2e8d5c` |
| macOS arm64 | `sherpa-onnx-v1.13.4-osx-arm64-static-lib.tar.bz2` | `57801db2bbb786a5d343f515a38ff210b401842338bdc804fa075312d1cd2404` |
| macOS x86_64 | `sherpa-onnx-v1.13.4-osx-x64-static-lib.tar.bz2` | `2bda2c10b31a1cfc45d9f9e14bd4983743ec3779d309e42d99a6c8fa1689043f` |

Normal releases are unsigned DMGs/AppImage artifacts plus source and checksums. If Apple signing credentials are supplied as repository secrets, the same macOS job signs with Hardened Runtime, notarizes, staples, and verifies an additional DMG; no Apple credentials are needed for ordinary local or CI builds. A signed release is not published unless `codesign`, `spctl`, and `stapler validate` pass.

## License

Lavtype source is MIT licensed. Third-party notices and the Parakeet model license are in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md). sherpa-onnx’s native runtime archive remains under its upstream license; it is downloaded and verified only as a build dependency.
