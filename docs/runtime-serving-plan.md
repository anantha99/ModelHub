# ModelHub Runtime & Agent Serving — Implementation Plan

Status: proposal, not yet implemented.

## 1. Goal

Today ModelHub finds and downloads models. It cannot run them, and nothing
outside the app can use them.

This plan makes a downloaded model usable in one action ("Serve") and makes the
served model reachable from coding tools — Claude Code, Cline, Continue, Zed,
aider, Codex CLI — through a single stable local endpoint. A coding agent should
be able to hand a subtask to a local model the same way it calls any other tool.

Success looks like this:

1. User opens Local, picks a downloaded GGUF model, clicks **Serve**.
2. ModelHub starts a backend for that model and reports "Ready".
3. User clicks **Connect** and copies a one-line `claude mcp add …` command.
4. Claude Code lists ModelHub's tools and can send a task to the local model.
5. The same running model answers OpenAI-compatible requests at
   `http://127.0.0.1:1710/v1` for every other tool.

## 2. Scope note against AGENTS.md

`AGENTS.md` §1 lists "Inference runtime" and "Plugin system" as out of MVP
scope, with instructions to confirm before building them. This work was
explicitly requested as post-MVP, so §1 and §15 should be amended in the same
change set that lands Phase 1 — otherwise future agents will treat this code as
scope violation and try to remove it.

## 3. Core design decision: route, don't re-implement

ModelHub does **not** implement inference. `llama.cpp`'s `llama-server` already
exposes an OpenAI-compatible API, and Ollama and LM Studio already run their own
servers. ModelHub becomes the **router and control plane** in front of them.

```
                 Claude Code            Cline / Continue / Zed / aider
                      │  MCP                        │  OpenAI API
                      ▼                             ▼
              ┌───────────────────────────────────────────┐
              │        ModelHub gateway  127.0.0.1:1710   │
              │   /mcp        (MCP streamable HTTP)       │
              │   /v1/*       (OpenAI-compatible)         │
              │   bearer token · loopback-only · Origin   │
              └───────────────┬───────────────────────────┘
                              │  alias → upstream resolution
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
  llama-server          Ollama                 LM Studio
  (ModelHub-owned       (delegated,            (delegated,
   child process,        :11434)                :1234)
   ephemeral port)
```

Why this shape:

- **One endpoint, one token.** Tools configure ModelHub once. Which engine
  actually hosts a model becomes an implementation detail the user can change
  without touching their editor config.
- **Small surface to own.** ModelHub owns process supervision, aliasing, auth,
  and protocol translation — not sampling, not GGUF parsing, not CUDA.
- **Ollama/LM Studio become assets, not competitors.** Models already installed
  there show up in the same list and serve through the same endpoint.

### Why two protocols and not one

They are not redundant — they serve different clients:

| | MCP (`/mcp`) | OpenAI (`/v1`) |
|---|---|---|
| Consumer | Claude Code, Claude Desktop, any MCP host | Cline, Continue, Zed, aider, Codex, OpenAI SDKs |
| Shape | Tool calls the agent decides to make | Chat completions the editor drives |
| Use | Orchestration — "delegate this subtask to the 4B" | Substitution — "this editor's model *is* the local model" |

Claude Code does not consume OpenAI-compatible endpoints for its own inference,
so MCP is the only way to reach it. Most other tools do not speak MCP for
inference, so `/v1` is the only way to reach them. Both are required.

## 4. Backend module layout

```text
src-tauri/src/serve/
  mod.rs              ServeManager — Tauri managed state, mirrors DownloadManager
  registry.rs         alias → ServedModel {model id, engine, upstream, status}
  engine/
    mod.rs            trait ModelEngine, EngineKind, EngineCapabilities
    llamacpp.rs       spawn/supervise llama-server, health poll, log capture
    ollama.rs         delegate to :11434 (reuses scanner::ollama)
    lmstudio.rs       delegate to :1234/v1
  binaries.rs         llama-server discovery, pinned download, checksum, version
  gateway/
    mod.rs            axum server, bind + shutdown lifecycle
    auth.rs           bearer token, loopback bind, Origin allowlist
    openai.rs         /v1/models, /v1/chat/completions, /v1/completions, /v1/embeddings
    mcp.rs            /mcp streamable HTTP transport
  mcp/
    mod.rs            JSON-RPC framing, initialize / tools/list / tools/call
    tools.rs          tool schemas + handlers
```

`ServeManager` follows the existing `DownloadManager` pattern exactly:
`Arc<Mutex<Inner>>`, constructed in `lib.rs` `.setup()`, registered with
`app.manage()`, injected into commands via `tauri::State`.

New crate dependencies: `axum`, `tokio` (`rt-multi-thread`, `process`, `signal`),
`tokio-stream`, `futures-util`, `uuid`, `rand`. `reqwest` needs the `stream`
feature added for async streaming proxy (the existing `blocking` usage stays as
is — downloads and scanners are untouched).

## 5. Engine abstraction

```rust
pub trait ModelEngine: Send + Sync {
    fn kind(&self) -> EngineKind;
    fn can_serve(&self, model: &LocalModel) -> Servability;
    fn start(&self, model: &LocalModel, opts: &ServeOptions) -> Result<Upstream, ServeError>;
    fn stop(&self, handle: &ServeHandle) -> Result<(), ServeError>;
    fn health(&self, upstream: &Upstream) -> HealthState;
}
```

`Servability` is deliberately a three-state value — `Yes`, `No { reason }`,
`Unknown` — because the UI must explain *why* a model has no Serve button.

**Format reality check.** `llama.cpp` serves GGUF only. Safetensors models
(the majority of HF repos) need vLLM or transformers, neither of which is a
reasonable Windows dependency today. Those models will report
`No { reason: "Safetensors models need a Python runtime ModelHub does not
manage yet." }`. This is a real limitation and the UI must say so plainly
rather than showing a button that fails.

### llama-server supervision (`llamacpp.rs`)

- Binary resolution order: user setting → bundled sidecar → previous managed
  download → `PATH` → not available.
- Managed download: pinned `llama.cpp` release tag, variant chosen by detected
  hardware (CPU / CUDA / Vulkan), SHA-256 verified, extracted under app data.
  No auto-update; the pinned tag moves only in a ModelHub release.
- Launch: `llama-server --model <gguf> --port <ephemeral> --host 127.0.0.1
  --ctx-size <n> --n-gpu-layers <n> --alias <alias>`.
- Port: bind `127.0.0.1:0`, read the assigned port, close, hand to the child —
  with a retry, because that has a race window.
- Readiness: poll `/health` until ready or timeout; surface stderr on failure
  instead of a bare exit code.
- Lifetime: child processes are killed on app exit and on window close. On
  Windows this needs a Job Object so a crashed ModelHub does not orphan a
  multi-GB process holding VRAM. Non-negotiable — leaked GPU memory is the
  worst failure mode here.
- Concurrency: one loaded model at a time by default, configurable. Serving a
  second model prompts to evict the first, with the RAM/VRAM numbers shown.

## 6. Gateway

Bind `127.0.0.1:1710` (configurable; 1234 and 11434 are taken by LM Studio and
Ollama). Off by default — the user turns serving on explicitly.

### OpenAI surface

- `GET /v1/models` — every currently served alias.
- `POST /v1/chat/completions` — streaming and non-streaming.
- `POST /v1/completions`, `POST /v1/embeddings` — pass-through where the
  upstream supports them, clean 501 where it does not.

Routing: read `model` from the request body, resolve the alias in the registry,
proxy to that upstream. If the alias is known but stopped, start it on demand
(configurable) rather than erroring — that is what makes the endpoint feel
stable to an editor.

### Aliases

Aliases are the user-facing model name, so they must be stable and predictable:
`qwen3-4b-q4_k_m` from `Qwen/Qwen3-4B` + `Q4_K_M`. Lowercased, non-alphanumerics
collapsed to `-`, deduped with a numeric suffix. Persisted with the served-model
record so an alias never silently changes under a configured editor. Renameable
in the UI.

### Auth

- Bearer token generated on first enable, persisted in app data with a
  restrictive ACL, rotatable from the UI.
- Loopback bind only — never `0.0.0.0`, not even behind a setting.
- `Origin` header allowlist on `/mcp` (loopback origins and absent Origin only).
  This is the documented DNS-rebinding defense for local MCP HTTP servers; a
  browser page on any site can otherwise reach a localhost port.
- Token never logged, never in events, masked in the UI until revealed.

## 7. MCP surface

Two transports, because tools differ:

- **Streamable HTTP** at `/mcp` — served by the running app. Preferred.
  `claude mcp add --transport http modelhub http://127.0.0.1:1710/mcp --header
  "Authorization: Bearer <token>"`.
- **stdio shim** — a small `modelhub-mcp.exe` that speaks stdio MCP and forwards
  to the HTTP gateway, for hosts without HTTP transport support. It holds no
  logic; it reads endpoint + token from app data and proxies.

### Tools

Kept deliberately small. An orchestrating agent that sees fifteen tools uses
none of them well.

| Tool | Purpose |
|---|---|
| `list_local_models` | Servable models with alias, size, context, capabilities, state |
| `start_model` / `stop_model` | Explicit load/unload with RAM cost reported |
| `run_task` | The main one: `{task, context?, model?, max_tokens?}` → completion |
| `serving_status` | What is loaded, memory in use, last error |

`run_task` is the feature that justifies the whole design. It lets Claude Code
push bulk, cheap, or privacy-sensitive work down to a local model — summarize
forty files, classify a changelog, redact a log — without that content leaving
the machine or consuming context. When `model` is omitted, ModelHub picks by
declared capability and current load rather than failing.

Tool results stay compact — a completion plus token counts and the model that
answered. No transcript dumps into the caller's context.

## 8. Frontend

- **Serve page** (replaces the current Runtimes page, which becomes a section of
  it): gateway on/off, endpoint, token with reveal/copy/rotate, engine status,
  loaded models with memory use, live request count, recent errors.
- **Local page**: a Serve toggle per model card. Disabled with a tooltip reason
  where `Servability::No`. This is the "select in the models list" entry point.
- **Connect panel**: copy-ready snippets, generated with the live port and token —
  `claude mcp add` command, OpenAI base URL + key, Continue/Cline JSON block.
  Copy buttons, no manual transcription.
- Events `serve:status_changed`, `serve:model_changed`, `serve:error` drive the
  UI. No polling, per AGENTS.md §6.
- Tray gets a serving indicator and a stop-all item.

New commands, following existing naming:

```ts
get_serve_status(): Promise<ServeStatus>
set_gateway_enabled(enabled: boolean): Promise<ServeStatus>
serve_model(input: ServeModelInput): Promise<ServedModel>
stop_served_model(alias: string): Promise<void>
list_served_models(): Promise<ServedModel[]>
rename_served_alias(alias: string, next: string): Promise<ServedModel>
rotate_gateway_token(): Promise<string>
get_connect_snippets(): Promise<ConnectSnippets>
get_engine_status(): Promise<EngineStatus[]>
install_engine_binary(kind: EngineKind): Promise<EngineStatus>
```

## 9. Phasing

Each phase is independently shippable and leaves the app working.

| Phase | Deliverable | Demoable result |
|---|---|---|
| 1 | `binaries.rs`, engine trait, `llamacpp.rs` supervision, `ServeManager`, start/stop commands | A GGUF model loads and answers on its own port |
| 2 | axum gateway, auth, alias registry, OpenAI routes, on-demand start | `curl` and Cline both talk to `:1710/v1` |
| 3 | MCP HTTP endpoint, tool set, stdio shim binary, connect snippets | Claude Code calls `run_task` against a local model |
| 4 | Serve page, per-model toggle, connect panel, events, tray | Whole loop is mouse-driven |
| 5 | Ollama + LM Studio delegation engines, capability routing, eviction policy | One endpoint fronts all three backends |

Phases 1–3 are the substance; 4 makes it a product; 5 is the aggregation payoff.

## 10. Testing

Everything below is unit-testable with no network and no model files, which
matters because CI cannot download a 4 GB GGUF:

- Alias generation: collision, unicode, very long repo IDs, stability across restarts.
- Registry resolution: unknown alias, stopped alias, duplicate registration.
- llama-server argument construction from `LocalModel` + `ServeOptions`.
- `Servability` decisions per format — GGUF yes, safetensors no with reason.
- MCP JSON-RPC framing: `initialize`, `tools/list`, malformed request, unknown tool.
- Auth: missing token, wrong token, non-loopback `Origin` rejected.
- Port allocation retry when the chosen port is taken between probe and spawn.
- Binary resolution order and checksum mismatch handling.

Live inference tests stay manual and `#[ignore]`d, per AGENTS.md §14.

## 11. Risks

| Risk | Mitigation |
|---|---|
| Orphaned llama-server holding VRAM after a crash | Windows Job Object kills children with the parent; startup sweep for stale ModelHub-owned processes |
| Local HTTP server reachable from a browser page | Loopback bind + bearer token + Origin allowlist; off by default |
| Engine binary download is large and hardware-specific | Pinned release, hardware-matched variant, checksum, explicit user action — never a silent background download |
| Safetensors models cannot be served | Stated plainly in the UI with the reason; not hidden behind a failing button |
| Scope creep into a chat app | No chat UI in ModelHub. Serving only. The client is always an external tool |
| Port collisions | Configurable port, clear error naming the conflicting process |

## 12. Open questions

1. **Bundle or download `llama-server`?** Bundling adds 50–200 MB per hardware
   variant to the installer; downloading adds a first-run step and a network
   dependency. Recommendation: download, with a pinned tag and checksum.
2. **Default context size.** llama.cpp defaults are conservative; larger values
   cost RAM sharply. Recommendation: derive from the model's declared context
   and available RAM, capped, user-overridable.
3. **Should `run_task` stream?** MCP tool results are single-shot. Long
   generations will look like a hang to the caller. Recommendation: cap
   `max_tokens` by default and report truncation explicitly.
4. **Does serving belong behind a settings flag for v1?** It changes ModelHub
   from a manager into a server. Recommendation: ship it enabled-but-idle —
   nothing listens until the user serves a model.
