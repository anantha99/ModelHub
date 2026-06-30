# PRD: ModelHub Windows

**Working name:** ModelHub Windows
**Product type:** Windows desktop app with system tray companion
**Primary platform:** Windows 10/11 x64
**Primary stack:** Tauri v2 + React + TypeScript + Rust
**Document status:** Implementation-ready MVP PRD
**Owner:** Anantha Kattani
**Goal:** Build a working end-to-end Windows model manager that can be used as a serious hiring/contribution demo for Conscious Engines or similar AI developer tooling companies.

---

## 1. Product Summary

ModelHub Windows is a desktop utility for managing local AI models on Windows. It gives users one clean place to discover, inspect, download, and manage models across Hugging Face cache, LM Studio, Ollama, and custom folders.

The product should not try to become a full chat app or inference runtime in the MVP. The MVP should focus on doing model management extremely well:

1. Find local models.
2. Search Hugging Face.
3. Download selected model files.
4. Write downloaded files into a Hugging Face-compatible cache layout.
5. Detect LM Studio and Ollama runtime status.
6. Provide a Windows-native desktop experience with tray support.

The hiring/demo story should be:

> “The original ModelHub is a macOS menu-bar app. I built a Windows desktop/tray version that preserves Hugging Face cache compatibility, adds Windows-specific storage handling, and integrates with LM Studio and Ollama.”

---

## 2. Why This Product Should Exist

Local LLM users on Windows often have models scattered across tools:

- Hugging Face cache
- LM Studio model directory
- Ollama internal model store
- Manually downloaded GGUF folders
- Random experiment folders on external drives

This causes common problems:

- Users do not know where disk space went.
- The same model may be downloaded multiple times.
- Users cannot easily identify model format, quantization, size, or source.
- Hugging Face downloads are not always compatible across tools.
- LM Studio and Ollama are managed separately.
- Deleting models safely is annoying.

ModelHub Windows should solve this by becoming the user’s “local model control center.”

---

## 3. Target Users

### 3.1 Primary User: Local AI Developer

This user runs models locally for app development, experimentation, or privacy.

Needs:

- Quickly see what models are installed.
- Search/download models without using terminal commands.
- Understand disk usage.
- Open model folders quickly.
- Use LM Studio or Ollama without confusion.

### 3.2 Secondary User: AI Power User

This user tries many models but is not deeply technical.

Needs:

- Clean search experience.
- Safe deletion.
- Clear “this model is huge” warnings.
- Runtime status without needing CLI knowledge.

### 3.3 Demo Audience: Conscious Engines / AI Tooling Company

This audience evaluates product engineering quality.

Needs to see:

- Real end-to-end functionality.
- Respect for their existing product architecture.
- Windows-specific thinking.
- Clean code structure.
- Good edge-case handling.
- Proper attribution and licensing behavior.

---

## 4. Goals

### 4.1 Product Goals

1. Provide one desktop app to manage local models on Windows.
2. Support Hugging Face cache, LM Studio, Ollama, and custom folders.
3. Search Hugging Face from the app.
4. Download model files with progress tracking.
5. Write downloads into Hugging Face-compatible cache structure.
6. Show runtime status for LM Studio and Ollama.
7. Provide safe model deletion using Recycle Bin where possible.
8. Provide a polished Windows desktop app plus system tray companion.

### 4.2 Engineering Goals

1. Use Tauri v2 for a lightweight desktop app.
2. Use Rust for filesystem scanning, downloads, cache writing, symlink/copy operations, and runtime checks.
3. Use React + TypeScript for UI.
4. Keep backend business logic testable outside the UI.
5. Avoid blocking the UI during scanning or downloads.
6. Build clean interfaces that can later support macOS/Linux if needed.

### 4.3 Hiring/Demo Goals

1. App should work end-to-end on a real Windows machine.
2. Demo should complete in less than 5 minutes.
3. Code should be easy for another engineer to inspect.
4. README should clearly explain relationship to original ModelHub.
5. App should not claim official affiliation unless granted.

---

## 5. Non-Goals for MVP

Do not build these in the MVP:

1. Full chat interface.
2. Custom inference runtime.
3. Fine-tuning.
4. Cloud sync.
5. Team accounts.
6. Model benchmarking.
7. RAG workflows.
8. Plugin marketplace.
9. Model conversion.
10. Automatic quantization.
11. Full Hugging Face authentication flow beyond token storage.
12. Enterprise deployment management.

These can be added later, but they will distract from the core demo.

---

## 6. Product Scope

### 6.1 MVP Scope

The MVP includes:

- Windows desktop app.
- System tray icon.
- Local model scanner.
- Hugging Face cache scanner.
- LM Studio folder scanner.
- Ollama installed model scanner through local API.
- Custom folder scanner.
- Hugging Face model search.
- Hugging Face model details view.
- File selection for downloads.
- Download manager with progress.
- Pause/resume/cancel where technically feasible.
- Hugging Face-compatible cache writer.
- Runtime status page for LM Studio and Ollama.
- Settings page.
- Safe delete through Recycle Bin.
- Open folder in Explorer.
- Copy model ID/path/API URL.

### 6.2 Post-MVP Scope

Possible v2 features:

- Duplicate model detection.
- Disk usage breakdown charts.
- Model recommendations based on hardware.
- “Run test prompt” through LM Studio/Ollama.
- Import manually downloaded models into HF cache.
- Model health check.
- GGUF metadata parser.
- GitHub releases auto-update.
- Portable app build.
- Windows startup integration.
- Multi-drive cache migration.

---

## 7. UX Direction

The app should feel like a polished Windows developer utility.

Principles:

- Simple, calm UI.
- Clear storage information.
- No flashy AI gimmicks.
- Fast scanning.
- Downloads should feel reliable.
- Dangerous actions must be reversible where possible.
- Tray should be useful but not replace the main app.

### 7.1 App Layout

Use a two-pane layout:

```text
┌─────────────────────────────────────────────────────────────┐
│ ModelHub Windows                                      ⊖ □ × │
├───────────────┬─────────────────────────────────────────────┤
│ Local         │                                             │
│ Explore       │              Main Content Area              │
│ Downloads     │                                             │
│ Runtimes      │                                             │
│ Settings      │                                             │
└───────────────┴─────────────────────────────────────────────┘
```

### 7.2 Navigation Items

1. **Local** — installed/discovered models.
2. **Explore** — Hugging Face search and model detail.
3. **Downloads** — active/completed download jobs.
4. **Runtimes** — LM Studio/Ollama status.
5. **Settings** — paths, token, scanning options, tray behavior.

---

## 8. Functional Requirements

## 8.1 Desktop App Shell

### Requirements

- App opens as a normal Windows desktop window.
- App has a persistent system tray icon.
- Closing the window should minimize to tray by default.
- Tray menu should include:
  - Open ModelHub
  - Downloads summary
  - Ollama status
  - LM Studio status
  - Pause all downloads
  - Settings
  - Quit
- “Quit” should fully exit the app.
- App should remember last window size and position.

### Acceptance Criteria

- User can launch app from Start Menu.
- User can close window and reopen from tray.
- User can fully quit from tray.
- App does not leave zombie processes after quit.

---

## 8.2 Local Model Scanner

### Sources

Scanner must support:

1. Hugging Face cache.
2. LM Studio directory.
3. Ollama API.
4. Custom folders configured by user.

### Default Windows Paths

Expected defaults:

```text
%USERPROFILE%\.cache\huggingface\hub
%USERPROFILE%\.lmstudio\models
```

The app must also check environment variables:

```text
HF_HOME
HF_HUB_CACHE
```

If present, these should override default Hugging Face paths.

### Scanning Rules

- Do not recursively crawl full model file trees unless needed.
- Prefer shallow scanning and known folder structures.
- Large directories should not freeze the UI.
- Scanning should run in Rust async/background tasks.
- Scanner should return partial results when one source fails.
- Each scanner error should be visible in diagnostics but should not break the whole scan.

### Model Fields

Each discovered model should include:

```ts
type LocalModel = {
  id: string;
  displayName: string;
  provider?: string;
  repoId?: string;
  source: "huggingface" | "lmstudio" | "ollama" | "custom";
  path?: string;
  sizeBytes?: number;
  format?: "gguf" | "safetensors" | "onnx" | "mlx" | "unknown";
  quantization?: string;
  lastModified?: string;
  files?: LocalModelFile[];
  runtimeStatus?: "available" | "loaded" | "running" | "unknown";
};
```

### Acceptance Criteria

- Existing Hugging Face cache models appear in Local tab.
- Existing LM Studio models appear in Local tab.
- Existing Ollama models appear in Local tab if Ollama is running.
- User can add a custom folder and rescan.
- Failed source scan shows a non-blocking warning.

---

## 8.3 Hugging Face Cache Scanner

### Folder Pattern

Parse Hugging Face cache folders like:

```text
models--Qwen--Qwen3-4B
```

into:

```text
Qwen/Qwen3-4B
```

### Expected Cache Layout

The app should understand:

```text
hub/
  models--org--repo/
    blobs/
    refs/
    snapshots/
```

### Requirements

- Read `refs/main` where available.
- Identify snapshot directories.
- Calculate model size from blob files where feasible.
- Avoid double-counting symlinked snapshot files.
- Detect broken cache entries and mark them as warnings.

### Acceptance Criteria

- HF cache models display readable repo IDs.
- Size calculation does not double-count symlinks.
- Broken entries do not crash scanner.

---

## 8.4 LM Studio Scanner

### Expected Directory Shape

LM Studio commonly stores models like:

```text
.lmstudio/models/
  publisher/
    model/
      file.gguf
```

### Requirements

- Scan publisher/model directories.
- Detect GGUF files.
- Parse quantization from filenames when possible.
- Provide “Open in Explorer.”
- Provide “Copy path.”

### Acceptance Criteria

- GGUF models under LM Studio appear in Local tab.
- Quantization like `Q4_K_M`, `Q5_K_M`, `Q8_0` is detected from filename.
- App handles empty LM Studio folder gracefully.

---

## 8.5 Ollama Scanner

### Requirements

- Detect whether Ollama API is reachable at:

```text
http://localhost:11434
```

- List installed models using Ollama local API.
- Show model name, size, modified date, parameter size, quantization if returned.
- Do not depend on Ollama being installed.

### Acceptance Criteria

- If Ollama is running, installed models appear.
- If Ollama is not running, Runtimes page shows “not running,” not an error stack.
- Ollama scan times out quickly.

---

## 8.6 Hugging Face Explore/Search

### Requirements

- Search Hugging Face models by query.
- Filter by:
  - Text generation
  - GGUF
  - Safetensors
  - Recently updated
  - Downloads/likes sort
- Show model cards with:
  - Repo ID
  - Author/org
  - Tags
  - Downloads
  - Likes
  - Last modified
  - Gated/private indicator
  - Available files summary

### Search Result Model

```ts
type HfModelSummary = {
  repoId: string;
  author?: string;
  tags: string[];
  downloads?: number;
  likes?: number;
  lastModified?: string;
  gated?: boolean;
  private?: boolean;
  pipelineTag?: string;
};
```

### Acceptance Criteria

- Searching `qwen` returns results.
- Clicking a result opens a details page.
- Gated/private models are visibly marked.
- Network failure shows retry UI.

---

## 8.7 Hugging Face Model Details

### Requirements

Details page should show:

- Repo ID.
- Tags.
- README summary if feasible in v2; optional for MVP.
- File list.
- File sizes.
- File extensions.
- Recommended files grouping:
  - GGUF files
  - Safetensors files
  - Tokenizer/config files
  - Other files
- Download selected files.
- Download full repo option.

### File Model

```ts
type HfModelFile = {
  path: string;
  sizeBytes?: number;
  lfs?: boolean;
  oid?: string;
  extension?: string;
  selectedByDefault?: boolean;
};
```

### Acceptance Criteria

- User can choose one GGUF file and required tokenizer/config files if needed.
- User can download selected files.
- App warns before downloading very large selections.

---

## 8.8 Download Manager

### Requirements

- Support multiple downloads.
- Show progress per file and per model.
- Show speed and estimated time if feasible.
- Support cancel.
- Support pause/resume where HTTP server and implementation support range requests.
- Persist incomplete downloads in app state.
- Use `.part` files until file is complete.
- Verify expected file size where available.
- Do not corrupt existing cache files.

### Download Job Model

```ts
type DownloadJob = {
  id: string;
  repoId: string;
  revision: string;
  commitSha?: string;
  destination: "hf_cache" | "lmstudio" | "custom";
  status: "queued" | "downloading" | "paused" | "completed" | "failed" | "cancelled";
  files: DownloadFileProgress[];
  totalBytes?: number;
  downloadedBytes: number;
  error?: string;
  createdAt: string;
  updatedAt: string;
};
```

### Acceptance Criteria

- User can start a download from model details.
- Download appears in Downloads page.
- Progress updates live.
- Cancel stops active network work.
- Completed download appears in Local tab after scan refresh.

---

## 8.9 Hugging Face-Compatible Cache Writer

### Requirements

The cache writer should write files into a Hugging Face-compatible cache layout:

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

- Download actual file content into `blobs`.
- Use actual model revision/commit SHA for snapshot folder when available.
- Write `refs/main` to the commit SHA when downloading main branch.
- Create snapshot file links to blobs.
- On Windows, try symlink first.
- If symlink fails, copy file into snapshot and record warning.
- Never delete existing blobs unless user explicitly deletes model/cache entry.
- Avoid duplicate downloads when blob already exists and size/hash matches.

### Windows Symlink Behavior

Windows symlink creation may fail unless Developer Mode or elevated permissions are enabled. The app should:

1. Try to create symlink.
2. If it fails, copy the file.
3. Show a settings warning explaining that Developer Mode can reduce duplicate disk usage.
4. Log the fallback reason in diagnostics.

### Acceptance Criteria

- Downloaded model is visible as a Hugging Face cache entry.
- App can rescan and detect it.
- Snapshot folder is created with selected files.
- `refs/main` is written when commit SHA is known.
- Windows symlink failure does not break download completion.

---

## 8.10 Runtime Status Page

### LM Studio

Show:

- Installed/detected: yes/no/unknown.
- API server running: yes/no.
- Local API base URL.
- Loaded models if endpoint returns them.
- Button: Copy OpenAI-compatible base URL.
- Button: Open LM Studio, if executable path known.

### Ollama

Show:

- API server running: yes/no.
- Local API base URL.
- Installed models.
- Button: Copy API base URL.
- Button: Refresh.

### Acceptance Criteria

- If no runtime is installed, page explains what is missing.
- If runtime is running, page shows status without needing restart.
- Network/API errors are human-readable.

---

## 8.11 Settings

### Settings Fields

```ts
type AppSettings = {
  hfCachePath?: string;
  lmStudioModelsPath?: string;
  customModelFolders: string[];
  hfTokenStored: boolean;
  minimizeToTray: boolean;
  startOnLogin: boolean;
  enableSymlinkAttempt: boolean;
  scanOnStartup: boolean;
  deleteUsesRecycleBin: boolean;
  telemetryEnabled: false;
};
```

### Requirements

- User can edit HF cache path.
- User can edit LM Studio models path.
- User can add/remove custom folders.
- User can store Hugging Face token in OS credential storage.
- User can clear token.
- User can enable/disable scan on startup.
- User can enable/disable minimize-to-tray.
- Telemetry must be off by default. MVP should avoid telemetry entirely.

### Acceptance Criteria

- Settings persist across app restarts.
- Bad paths show validation warnings.
- Tokens are not stored in plaintext config files.

---

## 8.12 Delete / Storage Management

### Requirements

- User can delete model entries where the app can safely determine ownership/path.
- Default delete behavior should use Recycle Bin.
- App must show confirmation with size and path.
- App must not delete shared Hugging Face blobs unless the selected cache entry is being removed intentionally.
- For MVP, deletion can be conservative.

### Acceptance Criteria

- User can delete a custom-folder model by sending file/folder to Recycle Bin.
- User can delete LM Studio model folder safely.
- HF cache deletion shows a clear warning.
- Delete failure shows a clear error.

---

## 9. Technical Architecture

## 9.1 Stack

Frontend:

- React
- TypeScript
- Vite
- Tailwind CSS or CSS modules
- Tauri JS API

Backend:

- Rust
- Tauri commands
- Tokio async runtime
- Reqwest for HTTP
- Serde for serialization
- Thiserror/anyhow for errors
- Walkdir for scanning where needed
- Dirs for OS paths
- Trash crate or Windows shell integration for Recycle Bin
- Keyring crate or equivalent OS credential storage for tokens

## 9.2 High-Level Architecture

```text
React UI
  ↓ Tauri invoke/events
Rust command layer
  ↓
Domain services
  ├── scanner
  ├── model_parser
  ├── hf_api
  ├── downloader
  ├── hf_cache_writer
  ├── runtime_lmstudio
  ├── runtime_ollama
  ├── settings
  └── diagnostics
  ↓
Windows filesystem / local APIs / Hugging Face API
```

## 9.3 Suggested Repo Structure

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

---

## 10. Tauri Command Contracts

These commands define the frontend/backend interface.

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

### Events From Rust to Frontend

```ts
download:updated
download:completed
download:failed
scan:completed
runtime:status_changed
diagnostics:warning
```

---

## 11. Data Models

### ScanResult

```ts
type ScanResult = {
  models: LocalModel[];
  sourceStatuses: SourceStatus[];
  totalSizeBytes?: number;
  scannedAt: string;
};
```

### SourceStatus

```ts
type SourceStatus = {
  source: "huggingface" | "lmstudio" | "ollama" | "custom";
  status: "ok" | "missing" | "error" | "disabled";
  path?: string;
  message?: string;
};
```

### RuntimeStatus

```ts
type RuntimeStatus = {
  lmStudio: {
    running: boolean;
    baseUrl: string;
    loadedModels: string[];
    error?: string;
  };
  ollama: {
    running: boolean;
    baseUrl: string;
    models: OllamaModel[];
    error?: string;
  };
};
```

---

## 12. Key User Flows

## 12.1 First Launch

1. User opens app.
2. App shows Local page.
3. App scans known sources.
4. App displays found models or empty state.
5. App shows tray icon.
6. User can go to Explore to search Hugging Face.

### Acceptance Criteria

- First launch works with no config.
- Missing folders are not treated as fatal errors.
- Empty state explains what to do next.

---

## 12.2 Search and Download Model

1. User opens Explore.
2. User searches `qwen`.
3. App displays Hugging Face results.
4. User opens a model details page.
5. User selects one GGUF file.
6. User clicks Download.
7. App shows progress in Downloads.
8. Download completes.
9. App writes file into HF cache layout.
10. Local page refreshes and shows new model.

### Acceptance Criteria

- This flow works end-to-end without terminal commands.
- User can see progress and completion.
- Downloaded model appears after refresh.

---

## 12.3 Runtime Status Check

1. User opens Runtimes.
2. App checks Ollama.
3. App checks LM Studio.
4. App displays running/not running states.
5. User copies local API URL.

### Acceptance Criteria

- Runtime checks complete quickly.
- Offline runtimes do not throw ugly errors.
- Running runtimes show useful information.

---

## 12.4 Safe Delete

1. User opens Local.
2. User selects model.
3. User clicks Delete.
4. App shows confirmation with path and size.
5. App sends file/folder to Recycle Bin.
6. App refreshes Local list.

### Acceptance Criteria

- Delete is reversible through Recycle Bin where supported.
- App does not permanently delete without explicit advanced option.

---

## 13. UI Requirements

### 13.1 Local Page

Must show:

- Search/filter input.
- Source filters.
- Total model count.
- Total disk usage if known.
- Grouped model list by source.
- Model cards/table rows.

Model row actions:

- Open folder.
- Copy model ID.
- Copy path.
- Delete.
- View details.

### 13.2 Explore Page

Must show:

- Search input.
- Filters.
- Result list.
- Loading state.
- Empty state.
- Error state.

### 13.3 Model Details Page

Must show:

- Repo ID.
- Tags.
- File list.
- Size summary.
- Selected files total size.
- Download button.

### 13.4 Downloads Page

Must show:

- Active downloads.
- Completed downloads.
- Failed downloads.
- Progress bars.
- Pause/resume/cancel.
- Error messages.

### 13.5 Runtimes Page

Must show:

- LM Studio status card.
- Ollama status card.
- Loaded/installed models.
- Copy URLs.
- Refresh button.

### 13.6 Settings Page

Must show:

- Paths.
- Custom folders.
- HF token management.
- Tray behavior.
- Startup behavior.
- Symlink warning.
- Diagnostics export.

---

## 14. Error Handling

Errors should be user-friendly.

Examples:

| Error | User Message |
|---|---|
| HF network timeout | “Could not reach Hugging Face. Check your internet connection and try again.” |
| Ollama not running | “Ollama is not running on localhost:11434.” |
| LM Studio not running | “LM Studio server is not running. Start it from LM Studio Developer settings.” |
| Symlink failed | “Windows blocked symlink creation, so ModelHub copied the file instead. This may use more disk space.” |
| Not enough disk | “Not enough free disk space for this download.” |
| Token missing for gated repo | “This model requires a Hugging Face token.” |
| Delete failed | “Could not move this model to Recycle Bin.” |

---

## 15. Security and Privacy

### Requirements

- No telemetry in MVP.
- No model data uploaded anywhere except Hugging Face API calls initiated by user search/download.
- Hugging Face token must not be stored in plaintext config.
- Hugging Face token must not be logged.
- File paths should not be sent to external services.
- Logs should be local only.
- Delete actions require confirmation.

### Acceptance Criteria

- Searching works without token for public models.
- Token can be stored and cleared.
- Logs do not include token.
- No background network calls except runtime checks and explicit HF operations.

---

## 16. Performance Requirements

- App startup: target under 3 seconds.
- Initial shallow scan: target under 5 seconds for normal users.
- UI should remain responsive during scan/download.
- Runtime API checks should timeout within 2 seconds.
- Large directory scans should be cancellable or non-blocking.
- Download progress should update at least once per second.

---

## 17. Testing Plan

### 17.1 Rust Unit Tests

Test:

- HF cache folder parsing.
- Repo ID encoding/decoding.
- Quantization parsing.
- File size formatting.
- Cache writer path generation.
- Symlink fallback decision.
- Scanner behavior with missing folders.

### 17.2 Frontend Tests

Test:

- Local page renders models.
- Empty states render.
- Error banners render.
- Download progress component updates.
- Settings form validation.

### 17.3 Manual E2E Tests

Test on Windows:

1. Fresh install.
2. Launch from Start Menu.
3. Tray behavior.
4. Scan HF cache.
5. Scan LM Studio directory.
6. Scan Ollama if running.
7. Search Hugging Face.
8. Download a small public model/file.
9. Verify cache layout.
10. Verify model appears in Local page.
11. Delete test model through Recycle Bin.
12. Restart app and verify settings persist.

---

## 18. Demo Script

Use this 5-minute demo:

1. Open ModelHub Windows.
2. Show Local page detecting existing models from HF/LM Studio/Ollama.
3. Open Runtimes page and show Ollama/LM Studio status.
4. Search Hugging Face for a model.
5. Open model details and choose a small file.
6. Start download.
7. Show progress in Downloads.
8. After completion, show it appears in Local.
9. Open folder in Explorer.
10. Explain Windows symlink/copy fallback handling.

---

## 19. Implementation Milestones

### Milestone 1: App Shell

- Tauri app boots.
- React shell with sidebar.
- Tray icon works.
- Settings persistence works.

### Milestone 2: Local Scanning

- HF cache scanner.
- LM Studio scanner.
- Custom folder scanner.
- Local page UI.

### Milestone 3: Runtime Integration

- Ollama status/list models.
- LM Studio status/list loaded models.
- Runtimes page UI.

### Milestone 4: Hugging Face Search

- Search API.
- Results UI.
- Details UI.
- File list.

### Milestone 5: Downloads + Cache Writer

- Download manager.
- Progress events.
- Cache writer.
- Windows symlink fallback.
- Completed model appears in Local.

### Milestone 6: Polish + Demo

- Safe delete.
- Error states.
- Loading states.
- README.
- Demo script.
- Build installer.

---

## 20. MVP Definition of Done

MVP is done when:

1. App runs on Windows as a desktop app.
2. App has working tray support.
3. App scans at least HF cache and LM Studio folders.
4. App detects Ollama if running.
5. App searches Hugging Face.
6. App can download selected public model files.
7. App writes downloads into HF-compatible cache layout.
8. App refreshes and shows downloaded model locally.
9. App can open model folder in Explorer.
10. App has safe error handling.
11. README explains setup, architecture, and demo.
12. AGENTS.md guides Codex implementation.

---

## 21. Legal / Attribution Notes

If this project reuses ideas or code from Conscious Engines ModelHub:

- Preserve MIT license notices for copied code.
- Clearly attribute original ModelHub where appropriate.
- Do not use Conscious Engines branding/logo without permission.
- Use “Windows prototype inspired by ModelHub” unless permission is granted.
- Keep code contribution-friendly and avoid pretending official affiliation.

---

## 22. Open Questions

1. Should the app name be “ModelHub Windows” or a distinct name until company permission is granted?
2. Should v1 write only to HF cache, or also support direct LM Studio download layout?
3. Should delete support for HF cache be conservative until duplicate/shared blob handling is perfect?
4. Should Hugging Face token support ship in MVP or immediately after MVP?
5. Should the app support portable mode for users who do not want installation?
