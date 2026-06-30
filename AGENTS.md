# AGENTS.md

This file gives coding agents implementation instructions for the ModelHub Windows repository.

The project is a Windows desktop app with tray support for managing local AI models across Hugging Face cache, LM Studio, Ollama, and custom folders.

Primary stack:

- Tauri v2
- React
- TypeScript
- Rust
- Windows 10/11

The main goal is to build a working end-to-end MVP suitable for a serious product demo and potential contribution/hiring conversation.

---

## 1. Product Boundary

Build a local model manager, not a chat app.

### In scope

- Windows desktop app.
- System tray support.
- Local model scanning.
- Hugging Face cache scanning.
- LM Studio folder scanning.
- Ollama local API scanning.
- Custom folder scanning.
- Hugging Face search.
- Model details and file selection.
- Download manager.
- Hugging Face-compatible cache writer.
- Runtime status page.
- Settings page.
- Safe delete through Recycle Bin.

### Out of scope for MVP

Do not implement these unless explicitly requested:

- Chat UI.
- Inference runtime.
- Fine-tuning.
- Quantization.
- Model conversion.
- RAG.
- Cloud sync.
- Accounts/teams.
- Telemetry.
- Plugin system.
- Benchmarking.

If asked to add one of these, first confirm it is a post-MVP feature.

---

## 2. Core Product Principle

The MVP must work end to end:

1. Launch Windows app.
2. Show tray icon.
3. Scan local models.
4. Search Hugging Face.
5. Download selected model files.
6. Write files into Hugging Face-compatible cache layout.
7. Refresh local models and show the downloaded model.
8. Detect LM Studio/Ollama status.

Prefer completing this loop over adding extra features.

---

## 3. Repository Structure

Use this structure unless there is a strong reason to change it:

```text
modelhub-windows/
  README.md
  AGENTS.md
  package.json
  pnpm-lock.yaml
  src/
    App.tsx
    main.tsx
    api/
      tauri.ts
      types.ts
    components/
      AppShell.tsx
      Sidebar.tsx
      ModelCard.tsx
      DownloadProgress.tsx
      EmptyState.tsx
      ErrorBanner.tsx
    pages/
      LocalPage.tsx
      ExplorePage.tsx
      DownloadsPage.tsx
      RuntimesPage.tsx
      SettingsPage.tsx
    state/
      useModelsStore.ts
      useDownloadsStore.ts
      useSettingsStore.ts
    styles/
      globals.css
  src-tauri/
    Cargo.toml
    tauri.conf.json
    src/
      main.rs
      commands.rs
      models.rs
      errors.rs
      scanner/
        mod.rs
        huggingface.rs
        lmstudio.rs
        ollama.rs
        custom.rs
      hf/
        mod.rs
        api.rs
        cache_writer.rs
        download.rs
      runtimes/
        mod.rs
        lmstudio.rs
        ollama.rs
      settings.rs
      paths.rs
      diagnostics.rs
  tests/
    fixtures/
      hf_cache_sample/
      lmstudio_sample/
```

Keep domain logic in Rust services, not React components.

---

## 4. Development Commands

Use `pnpm` for frontend/package tasks.

Common commands:

```bash
pnpm install
pnpm dev
pnpm tauri dev
pnpm build
pnpm tauri build
```

Rust commands:

```bash
cd src-tauri
cargo fmt
cargo clippy -- -D warnings
cargo test
```

Before completing a task, run the most relevant checks:

```bash
pnpm build
cd src-tauri && cargo fmt && cargo clippy -- -D warnings && cargo test
```

If a command cannot run in the environment, clearly mention it in the final response.

---

## 5. Coding Standards

### TypeScript / React

- Use TypeScript for all frontend code.
- Avoid `any` unless absolutely necessary.
- Keep components small and readable.
- Use explicit prop types.
- Keep Tauri calls in `src/api/tauri.ts`.
- Keep shared app types in `src/api/types.ts`.
- UI components should not know Rust implementation details.
- Handle loading, empty, and error states for every async view.

### Rust

- Use strong typed structs for command inputs/outputs.
- Derive `Serialize` and `Deserialize` for all Tauri payload types.
- Use `thiserror` for domain errors where useful.
- Avoid `unwrap()` and `expect()` in production code.
- Return user-friendly errors through command boundaries.
- Keep scanning/downloading non-blocking.
- Write unit tests for parsers, cache paths, and scanner edge cases.

### Formatting

- Run `cargo fmt` for Rust.
- Use the project formatter for frontend once configured.
- Keep names clear and boring.

---

## 6. Tauri Command Contracts

Expose these commands from Rust to React:

```ts
scan_models(): Promise<ScanResult>
search_hf_models(input: HfSearchInput): Promise<HfSearchResult>
get_hf_model_details(repoId: string, revision?: string): Promise<HfModelDetails>
start_download(input: StartDownloadInput): Promise<DownloadJob>
pause_download(jobId: string): Promise<void>
resume_download(jobId: string): Promise<void>
cancel_download(jobId: string): Promise<void>
list_downloads(): Promise<DownloadJob[]>
get_runtime_status(): Promise<RuntimeStatus>
get_settings(): Promise<AppSettings>
update_settings(patch: Partial<AppSettings>): Promise<AppSettings>
open_path(path: string): Promise<void>
delete_model(input: DeleteModelInput): Promise<DeleteResult>
store_hf_token(token: string): Promise<void>
clear_hf_token(): Promise<void>
```

Emit events for long-running work:

```ts
download:updated
download:completed
download:failed
scan:completed
runtime:status_changed
diagnostics:warning
```

Do not make the frontend poll aggressively if events can be used.

---

## 7. Data Model Rules

Shared frontend/backend models should stay aligned.

Important model source values:

```ts
"huggingface" | "lmstudio" | "ollama" | "custom"
```

Important model format values:

```ts
"gguf" | "safetensors" | "onnx" | "mlx" | "unknown"
```

Important download statuses:

```ts
"queued" | "downloading" | "paused" | "completed" | "failed" | "cancelled"
```

When adding fields, update:

1. Rust structs.
2. TypeScript types.
3. UI rendering.
4. Tests/fixtures where relevant.

---

## 8. Filesystem and Cache Rules

### Hugging Face cache

The app must understand and write this structure:

```text
models--org--repo/
  blobs/
  refs/
    main
  snapshots/
    <commit-sha>/
      file.gguf
```

Rules:

- Encode repo IDs as `models--org--repo`.
- Decode cache folders back to `org/repo`.
- Do not recursively scan huge model directories when shallow scanning is enough.
- Do not double-count symlinked snapshot files.
- Prefer blob size for disk usage.
- Use actual commit SHA for snapshots when available.
- Do not invent fake commit SHA values for production cache writing.

### Windows symlink behavior

When writing snapshot files:

1. Try symlink if setting allows it.
2. If symlink fails, copy the file.
3. Record a warning.
4. Do not fail the whole download solely because symlink is unavailable.

### Delete behavior

- Prefer Recycle Bin over permanent delete.
- Always require confirmation in the UI.
- Be conservative with Hugging Face cache deletion because blobs can be shared.
- Never delete paths outside known model roots unless the user explicitly selected a custom folder model.

---

## 9. Path Rules

Default paths:

```text
%USERPROFILE%\.cache\huggingface\hub
%USERPROFILE%\.lmstudio\models
```

Environment overrides:

```text
HF_HOME
HF_HUB_CACHE
```

Resolution order for Hugging Face cache:

1. User setting `hfCachePath`.
2. `HF_HUB_CACHE`.
3. `HF_HOME` + `/hub`.
4. `%USERPROFILE%\.cache\huggingface\hub`.

Do not hardcode the current developer’s username.

---

## 10. Runtime Integration Rules

### Ollama

- Base URL: `http://localhost:11434`.
- Listing models should use local API.
- Timeout quickly if not running.
- Treat “not running” as normal status, not an exception.

### LM Studio

- Base URL: `http://localhost:1234/v1` for OpenAI-compatible endpoint.
- Loaded models may be checked through `/models` when server is running.
- Treat “not running” as normal status.

Do not require either runtime to be installed for the app to work.

---

## 11. Hugging Face API Rules

- Public search must work without a token.
- Gated/private models should show a clear message.
- Token support should use OS credential storage, not plaintext config.
- Never log tokens.
- Network errors should be friendly.
- Large downloads must show size warnings.

Download behavior:

- Use `.part` files during active download.
- Move/rename only after completion.
- Verify expected size when available.
- Avoid overwriting existing valid blobs.
- Support cancellation.
- Pause/resume should only be marked supported when range requests are actually implemented.

---

## 12. UI Rules

Every page must support:

- Loading state.
- Empty state.
- Error state.
- Refresh action where relevant.

Pages:

- Local
- Explore
- Downloads
- Runtimes
- Settings

The UI should feel like a Windows developer tool:

- Calm.
- Clear.
- Fast.
- Not flashy.
- No fake AI magic.

Do not block core functionality on pixel-perfect styling.

---

## 13. Security and Privacy Rules

- No telemetry in MVP.
- Do not send local file paths to external services.
- Do not upload model metadata anywhere.
- Do not store Hugging Face tokens in plaintext.
- Do not print tokens in logs.
- Keep diagnostics local.
- Confirm destructive actions.
- Avoid admin-only requirements.

---

## 14. Testing Requirements

Add tests for:

- Hugging Face cache folder encode/decode.
- Hugging Face cache scanning fixtures.
- LM Studio folder scanning fixtures.
- Quantization parsing.
- Size formatting.
- Cache writer path creation.
- Missing folders.
- Broken symlink handling where testable.
- User-friendly error conversion.

Do not require network access for normal unit tests.

Network/API tests should be manual or explicitly marked ignored.

---

## 15. Implementation Order

Follow this order unless explicitly told otherwise:

1. Create Tauri + React shell.
2. Add tray support.
3. Add settings storage.
4. Implement path resolution.
5. Implement HF cache scanner.
6. Implement LM Studio scanner.
7. Implement Local page.
8. Implement Ollama/LM Studio runtime checks.
9. Implement Explore search.
10. Implement model details file list.
11. Implement download manager.
12. Implement HF cache writer.
13. Connect completed downloads to Local refresh.
14. Add safe delete/open folder/copy actions.
15. Polish error states.
16. Add README/demo notes.

Do not start with complex styling or post-MVP features.

---

## 16. Definition of Done for Any Task

A task is done only when:

- Code compiles.
- Relevant tests pass or the failure is clearly documented.
- UI handles loading/error/empty states if UI was changed.
- No token/path privacy regression was introduced.
- No unrelated refactor was performed.
- The final response summarizes changed files and commands run.

---

## 17. Demo Requirements

The final app should support this demo:

1. Launch app.
2. Tray icon appears.
3. Local page scans models.
4. Runtimes page shows Ollama/LM Studio status.
5. Explore page searches Hugging Face.
6. User selects a model file.
7. Download progress appears.
8. Download completes into HF cache.
9. Local page shows downloaded model.
10. User opens model folder in Explorer.

Prioritize bugs blocking this demo above all other issues.

---

## 18. Attribution and Naming

This project may be inspired by Conscious Engines ModelHub.

Rules:

- Do not claim official affiliation unless explicitly approved.
- Preserve license notices for copied MIT code.
- Prefer original implementation in Rust/TypeScript.
- Use a neutral README phrase such as “Windows prototype inspired by ModelHub” until permission is granted.
- Do not use company logos or trademarks without permission.

---

## 19. Communication Style for Agent Outputs

When reporting progress, be direct:

- What changed.
- Why it changed.
- What commands were run.
- What still needs work.
- Any known risks.

Do not over-explain obvious code.
Do not hide failed checks.
Do not say a feature is complete unless it works end to end.
