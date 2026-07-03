# ModelHub Windows

ModelHub Windows is a Windows desktop prototype inspired by ModelHub. It is a local AI model manager for developers who keep models spread across Hugging Face cache, LM Studio, Ollama, and manually managed folders.

The goal is not to build another chat app. The goal is to make local model storage visible, searchable, and safer to manage on Windows.

## What This Project Is Trying To Do

ModelHub Windows aims to provide one calm desktop utility for the full local-model loop:

1. Scan local model locations on Windows.
2. Show models from Hugging Face cache, LM Studio, and Ollama in one place.
3. Search public Hugging Face models.
4. Inspect model files before downloading.
5. Download selected files with progress tracking.
6. Install completed downloads into a Hugging Face-compatible cache layout.
7. Refresh the local library so the downloaded model appears immediately.
8. Check local runtime status without requiring runtimes to be installed.

This repository is an early Windows MVP built with Tauri, React, TypeScript, and Rust.

## Current Status

Version `0.1.1` is the first installable Windows preview. It is suitable for local development, demo review, and trusted early users, but it is signed with a self-signed certificate rather than a publicly trusted code-signing certificate.

Implemented so far:

- Tauri v2 Windows desktop shell with system tray support.
- React/TypeScript app with Local, Explore, Downloads, Runtimes, and Settings pages.
- Hugging Face cache path resolution using settings, `HF_HUB_CACHE`, `HF_HOME`, and the default Windows cache path.
- Hugging Face cache scanning with cache folder decoding, snapshot reading, model metadata extraction, and shared-blob-aware deletion boundaries.
- LM Studio folder scanning with GGUF and model metadata support.
- Ollama local API scanning and runtime status for `http://localhost:11434`.
- Hugging Face public search and model details lookup.
- Staged Hugging Face downloads with live progress events, cancellation, persisted jobs, and restart-safe failure marking.
- Install completed downloads into Hugging Face-compatible cache folders under `models--org--repo/blobs`, `refs`, and `snapshots`.
- Symlink-first snapshot creation with copy fallback when Windows symlinks are unavailable.
- Open model folders in Explorer.
- Conservative delete flow that moves eligible local models to the Recycle Bin.
- Settings UI for model paths, tray behavior, scanning, symlink attempts, and safe deletion defaults.
- Local system information collection for CPU, memory, GPU, and cache disk context.

Known gaps before a broader public end-user release:

- The installer is self-signed. Windows SmartScreen or browser warnings may still appear.
- LM Studio server runtime check is still a placeholder; folder scanning works.
- Hugging Face token storage is not enabled yet, so private/gated models are reported clearly but cannot be downloaded.
- Pause/resume controls are intentionally disabled until HTTP range resume is implemented.
- Custom folder scanning is represented in settings and types, but the scanner still needs full implementation.
- UI polish and broader Windows hardware testing are still in progress.

## Install Preview Build

Download the latest Windows installer from the GitHub Releases page:

```text
https://github.com/anantha99/ModelHub/releases
```

The `0.1.1` preview installer targets Windows 10/11 x64 and installs for the current user, so it should not require administrator access.

Because this preview uses a self-signed certificate, Windows may show SmartScreen or Unknown Publisher warnings. The checksum and certificate thumbprint published with the GitHub release help detect corruption or accidental asset mismatches, but they are not an independent trust anchor by themselves. For stronger trust, verify the certificate thumbprint through a separate channel before running the installer.

The installer uses Tauri's WebView2 download bootstrapper. If Microsoft Edge WebView2 Runtime is missing, the installer may request network access and show Microsoft's WebView2 installer prompt.

After launch, closing the main window hides it to the system tray by default. Use the tray menu and choose Quit to fully exit the app.

## Demo Flow

The intended MVP demo is:

1. Launch ModelHub Windows.
2. Confirm the tray icon appears.
3. Open Local and scan discovered models.
4. Open Runtimes and check Ollama status.
5. Open Explore and search Hugging Face.
6. Select files from a model details view.
7. Start a download and watch progress on Downloads.
8. Install the completed download into the Hugging Face cache.
9. Return to Local and confirm the installed model appears.
10. Open the model folder in Explorer.

## Tech Stack

- Tauri v2
- Rust
- React 19
- TypeScript
- Vite
- pnpm
- Windows 10/11 target platform

## Repository Layout

```text
src/
  api/              Tauri command wrappers and shared frontend types
  components/       Shared React UI components
  pages/            Local, Explore, Downloads, Runtimes, Settings
  styles/           Global app styling
  utils/            Formatting helpers
src-tauri/
  src/              Rust commands, scanners, downloads, cache writer, tray
  capabilities/     Tauri capability configuration
tests/fixtures/     Scanner fixtures for local unit tests
```

## Development

Install dependencies:

```bash
pnpm install
```

Run the frontend dev server:

```bash
pnpm dev
```

Run the Tauri app in development:

```bash
pnpm tauri dev
```

Build the frontend:

```bash
pnpm build
```

Run Rust checks:

```bash
cd src-tauri
cargo fmt
cargo clippy -- -D warnings
cargo test
```

Build the Tauri app and NSIS installer:

```bash
pnpm release:windows
```

Installer artifacts are written under `src-tauri/target/release/bundle/nsis/`.

The release script builds the NSIS installer, signs packaged executables through Tauri's Windows `signCommand`, signs the final installer, exports the public certificate, and writes release upload notes.

To regenerate only the signing/checksum notes after a successful build:

```bash
pnpm release:windows:sign
```

The signing script creates or reuses a `CurrentUser\My` code-signing certificate, exports the public `.cer` file beside the installer, and writes checksum/thumbprint notes for the GitHub Release. It does not commit private key material.

## Windows Paths

Default Hugging Face cache resolution order:

1. User setting in ModelHub Windows.
2. `HF_HUB_CACHE`.
3. `HF_HOME` plus `hub`.
4. `%USERPROFILE%\.cache\huggingface\hub`.

Default LM Studio model path:

```text
%USERPROFILE%\.lmstudio\models
```

Ollama is checked through its local API:

```text
http://localhost:11434
```

LM Studio server checks target this endpoint once runtime checks are fully connected:

```text
http://localhost:1234/v1
```

## Privacy And Safety

- No telemetry is included in the MVP.
- Local paths and model metadata are not uploaded to external services.
- Hugging Face public search uses the Hugging Face API.
- Hugging Face tokens are not stored in plaintext; token support is deferred until OS credential storage is wired in.
- Delete actions are conservative and use the Recycle Bin.
- Hugging Face cache deletion is limited to snapshot folders, not shared blobs or full cache roots.

## Attribution

This is an independent Windows prototype inspired by ModelHub. It does not claim official affiliation with Conscious Engines or the original ModelHub project.

## Release Notes

### 0.1.1

First installable Windows preview release with a current-user NSIS installer, release metadata, MIT licensing, and a repeatable self-signed Authenticode signing script for preview artifacts.

### 0.1.0

Initial Windows MVP source release with desktop shell, tray support, local model scanning, Hugging Face search/details, staged downloads, Hugging Face cache installation, settings, safe deletion, and Ollama runtime checks.
