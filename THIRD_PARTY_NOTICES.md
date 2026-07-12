# Third-party notices

Lavtype is distributed under the MIT license in [`LICENSE`](LICENSE). The release contains or builds against the following third-party software. License texts are available from the linked upstream projects and are included in the Cargo dependency source where applicable.

## Rust dependencies

The versions are pinned in `Cargo.toml`/`Cargo.lock`; Cargo may select the compatible MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0, or Unicode-DFS-2016 licensed transitive components listed by `cargo license` for a particular build.

- tao 0.35.3 — MIT.
- tray-icon 0.24.1 — MIT.
- global-hotkey 0.8.0 — MIT.
- cpal 0.18.1 — Apache-2.0/MIT.
- rubato 4.0.0 and audioadapter-buffers 4.0.0 — MIT.
- enigo 0.6.1 — MIT.
- sherpa-onnx 1.13.4 and its native runtime archives — Apache-2.0.
- directories 6.0.0, serde 1.x, toml 0.9.x, thiserror 2.x, crossbeam-channel 0.5.x, tempfile 3.27.x, hound 3.5.1, reqwest 0.13.4, sha2 0.11.0, tar 0.4.46, bzip2 0.6.1, and fs2 0.4.x — their upstream license files and notices are authoritative.
- macOS-only objc2 0.6, block2 0.6, objc2-speech 0.3.2, objc2-foundation 0.3.2, objc2-av-foundation 0.3.2, and objc2-application-services 0.3.2 — MIT/Apache-2.0 as indicated by each upstream package.

The complete dependency graph is reproducible with the committed lockfile. Do not remove transitive notices when redistributing a source or binary artifact.

## Parakeet model

The optional downloaded model is **NVIDIA Parakeet TDT 0.6B v2** (`nvidia/parakeet-tdt-0.6b-v2`), an English model with punctuation and capitalization. It is distributed under **CC-BY-4.0**. The Lavtype model installer pins and verifies this exact release asset:

- Asset: `sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2`
- Upstream: <https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2>
- Size: `482468385` bytes
- SHA-256: `157c157bc51155e03e37d2466522a3a737dd9c72bb25f36eb18912964161e1ad`
- License reference: <https://creativecommons.org/licenses/by/4.0/>
- Model card: <https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2>

Lavtype does not redistribute the model in its DMGs/AppImage. Users opt in to the download from the tray, and the model remains in the user data directory.

## Build tools and packaging

- cargo-packager 0.11.8 — MIT/Apache-2.0; used only by the release/build workflow.
- linuxdeploy and the GTK linuxdeploy plugin — MIT; the GTK plugin is pinned in `Packager.toml` to commit `7a3fbc31a9e5075073ff8790f26effbac5f84453`.
- GitHub Actions and runner-provided system libraries (GTK3, Ayatana AppIndicator, ALSA, XKB, FUSE) are build/runtime infrastructure, not shipped as Lavtype source.
