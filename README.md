# ModelHub Windows

ModelHub Windows is a desktop model inventory app for Windows developers working with local AI models.

It scans common local model locations, shows what is installed, helps you understand where models live on disk, and gives you safer tools for opening, downloading, and managing model files without digging through cache folders by hand.

[Download Windows Preview](https://github.com/anantha99/ModelHub/releases/latest)

## Why ModelHub Windows

Local AI models rarely live in one obvious place.

Hugging Face uses cache folders. LM Studio has its own model directory. Ollama exposes models through a local runtime. Some files are manually downloaded, renamed, or moved over time.

ModelHub Windows gives developers one calm Windows desktop app for seeing local models, understanding their source, checking runtime status, and taking safe file actions from one place.

It is not a chat app, inference runtime, or model conversion tool. It is focused on local model visibility and management.

## What It Does

- Scans Hugging Face cache models on Windows.
- Scans LM Studio model folders.
- Reads locally available Ollama models through the Ollama API.
- Shows model metadata, file formats, paths, sizes, and source.
- Searches public Hugging Face models.
- Lets you inspect model files before downloading.
- Tracks staged downloads with progress and cancellation.
- Installs completed downloads into a Hugging Face-compatible cache layout.
- Opens model folders in Windows Explorer.
- Uses conservative delete behavior with Recycle Bin support where eligible.
- Runs as a Windows desktop app with system tray support.

## Windows Preview

ModelHub Windows is currently available as a Windows Preview for Windows 10/11 x64.

Download the latest installer from:

```text
https://github.com/anantha99/ModelHub/releases/latest
```

The preview installer is self-signed. Windows SmartScreen or browser warnings may appear because the certificate is not from a public code-signing authority yet. Release checksums and certificate details are published with the release so trusted early users can verify the downloaded installer.

## Quick Start

1. Install and launch ModelHub Windows.
2. Open the Local page.
3. Scan local model locations.
4. Review discovered models, formats, sizes, and paths.
5. Open model folders directly in Explorer when needed.
6. Use Explore to search Hugging Face models.
7. Select only the files you want to download.
8. Track downloads from the Downloads page.
9. Check Ollama and local runtime status from Runtimes.
10. Adjust cache paths and app behavior in Settings.

## Privacy And Safety

ModelHub Windows is local-first.

- No telemetry is included in the preview.
- Local model paths are not uploaded.
- Local model metadata is not sent to external services.
- Hugging Face search uses the public Hugging Face API.
- Delete actions are conservative and prefer the Recycle Bin.
- Hugging Face cache deletion avoids shared blob cleanup by default.
- Hugging Face token storage is deferred until OS credential storage is wired in.

## Preview Limitations

The current Windows Preview is intended for local development, demo review, and trusted early users.

Known limitations:

- The installer is self-signed, so Windows trust warnings may appear.
- Hugging Face private and gated model downloads are not enabled yet.
- Pause and resume controls are disabled until HTTP range resume is implemented.
- LM Studio folder scanning works, but full LM Studio server runtime checks are still being completed.
- Custom folder scanning is represented in settings, but full scanner support is still in progress.
- Broader Windows hardware testing and UI polish are ongoing.

## Tech Stack

- Tauri v2
- Rust
- React
- TypeScript
- Vite
- pnpm
- Windows 10/11

## Development

Install dependencies:

```bash
pnpm install
```

Run the frontend dev server:

```bash
pnpm dev
```

Run the desktop app in development:

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

Build the Windows preview installer:

```bash
pnpm release:windows
```

Installer artifacts are generated under:

```text
src-tauri/target/release/bundle/nsis/
```

## Project Status

ModelHub Windows is focused on completing the local model management loop:

1. Scan local models.
2. Show models from Hugging Face cache, LM Studio, and Ollama.
3. Search Hugging Face.
4. Inspect model files.
5. Download selected files.
6. Install downloads into a Hugging Face-compatible cache.
7. Refresh the local library.
8. Open or safely manage model folders from Windows.

See `CHANGELOG.md` for release history.

## Attribution

ModelHub Windows is an independent Windows project inspired by ModelHub. It is not officially affiliated with Conscious Engines or the original ModelHub project.

## License

MIT
