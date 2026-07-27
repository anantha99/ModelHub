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

1. User enables **Serving** in Settings (off by default — see §3.4).
2. ModelHub reports whether a llama.cpp engine is installed, and helps install
   one if not.
3. User opens Local, picks a downloaded GGUF model, clicks **Serve**, and
   confirms context size in the dialog that appears.
4. User clicks **Connect** and copies a one-line `claude mcp add …` command.
5. Claude Code lists ModelHub's tools and can send a task to the local model.
6. The same running model answers OpenAI-compatible requests at
   `http://127.0.0.1:1710/v1` for every other tool.

## 2. Scope note against AGENTS.md

`AGENTS.md` §1 lists "Inference runtime" and "Plugin system" as out of MVP
scope, with instructions to confirm before building them. This work was
explicitly requested as post-MVP, so §1 and §15 should be amended in the same
change set that lands Phase 1 — otherwise future agents will treat this code as
scope violation and try to remove it.

## 3. Decisions

These four were open questions in the first draft and are now settled. They
shape the rest of the plan, so they are recorded here rather than in a footnote.

### 3.1 Engine binary: detect first, assist if missing

ModelHub does not bundle `llama-server`. It looks for an existing install, and
only if none is found does it offer to help the user get one. ModelHub never
downloads an engine silently or as a side effect of another action.

Detection order:

1. User setting `llamaServerPath`.
2. A previous ModelHub-assisted install under app data.
3. `llama-server.exe` on `PATH`.
4. Common install locations (package-manager shims, `%LOCALAPPDATA%\Programs\llama.cpp`).

When detection succeeds, ModelHub runs `llama-server --version` and records it.
A version too old for the flags in §5 produces a clear warning naming the
required version, not a confusing runtime failure later.

When detection fails, the Engine card offers two paths, both user-initiated:

- **Install for me** — download a pinned `llama.cpp` release, variant matched to
  detected hardware (CPU / CUDA / Vulkan), SHA-256 verified, extracted under app
  data. The pinned tag moves only in a ModelHub release; there is no auto-update.
- **I'll install it myself** — link to the official release page, name the exact
  binary to look for, and offer **Browse…** to point at it plus **Re-detect**.

Open sub-task: confirm whether current winget/scoop packages for llama.cpp exist
and are trustworthy before naming any package manager command in the UI. Do not
ship a command that has not been verified on a clean Windows machine.

### 3.2 Context size: always asked at serve time

Clicking **Serve** opens a small dialog rather than starting immediately. The
dialog asks for context size and GPU offload, and shows a live memory estimate
that updates as those change.

The estimate is computed from fields the metadata scanner already extracts into
`LocalModelTechnical` — `block_count`, `embedding_length`, `kv_heads`,
`context_length` — combined with `system_info`'s RAM and GPU numbers. It is an
approximation of KV-cache cost and must be labelled as one in the UI. It is
there to stop a user from requesting a context that will not fit, not to be
exact.

Values are prefilled from the model's declared context (capped to what fits) and
from the last values used for that model, so a repeat serve is confirm-and-go
rather than re-entry. The dialog still appears every time, as decided.

### 3.3 `run_task`: capped, with an explicit continue

MCP tool results are single-shot, so an uncapped generation looks like a hang to
the calling agent and dumps an unbounded result into its context.

`run_task` therefore caps `max_tokens` by default. When output hits the cap it
returns `finish_reason: "length"` and a `continuation_id`, and a separate
`continue_task` tool resumes from there. See §7 for the state this requires and
what it costs.

### 3.4 Serving is off until enabled in Settings

A new `servingEnabled` setting defaults to `false`. Until it is on, no port is
bound, no engine is started, and the Serve controls are visible but disabled
with a link to the setting. Turning it off stops the gateway and every served
model.

This keeps ModelHub a model manager by default and makes becoming a server an
explicit, revocable choice.

## 4. Core design: route, don't re-implement

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

## 5. Backend module layout

```text
src-tauri/src/serve/
  mod.rs              ServeManager — Tauri managed state, mirrors DownloadManager
  registry.rs         alias → ServedModel {model id, engine, upstream, status}
  estimate.rs         context/memory estimation from LocalModelTechnical + SystemInfo
  engine/
    mod.rs            trait ModelEngine, EngineKind, EngineCapabilities
    llamacpp.rs       spawn/supervise llama-server, health poll, log capture
    ollama.rs         delegate to :11434 (reuses scanner::ollama)
    lmstudio.rs       delegate to :1234/v1
  binaries.rs         engine detection, version probe, assisted install
  gateway/
    mod.rs            axum server, bind + shutdown lifecycle
    auth.rs           bearer token, loopback bind, Origin allowlist
    openai.rs         /v1/models, /v1/chat/completions, /v1/completions, /v1/embeddings
    mcp.rs            /mcp streamable HTTP transport
  mcp/
    mod.rs            JSON-RPC framing, initialize / tools/list / tools/call
    tools.rs          tool schemas + handlers
    continuations.rs  bounded, TTL'd store backing continue_task
```

`ServeManager` follows the existing `DownloadManager` pattern exactly:
`Arc<Mutex<Inner>>`, constructed in `lib.rs` `.setup()`, registered with
`app.manage()`, injected into commands via `tauri::State`.

New crate dependencies: `axum`, `tokio` (`rt-multi-thread`, `process`, `signal`),
`tokio-stream`, `futures-util`, `uuid`, `rand`. `reqwest` needs the `stream`
feature added for async streaming proxy (the existing `blocking` usage stays as
is — downloads and scanners are untouched).

New settings fields: `serving_enabled` (default `false`), `gateway_port`
(default `1710`), `llama_server_path`, and a per-model map of last-used serve
options. `AppSettings::sanitized()` and `apply_patch` must handle these; note
that several existing fields are deliberately force-reset in those functions, so
the new ones need to be added without disturbing that behavior.

## 6. Engine abstraction

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

- Binary comes from `binaries.rs` detection (§3.1); a missing engine is a
  first-class UI state, not a serve-time error.
- Launch: `llama-server --model <gguf> --port <ephemeral> --host 127.0.0.1
  --ctx-size <from dialog> --n-gpu-layers <from dialog> --alias <alias>`.
- Port: bind `127.0.0.1:0`, read the assigned port, close, hand to the child —
  with a retry, because that has a race window.
- Readiness: poll `/health` until ready or timeout; surface stderr on failure
  instead of a bare exit code.
- Lifetime: child processes are killed on app exit and on window close. On
  Windows this needs a Job Object so a crashed ModelHub does not orphan a
  multi-GB process holding VRAM. Non-negotiable — leaked GPU memory is the
  worst failure mode here.
- Concurrency: one loaded model at a time by default, configurable. Serving a
  second model prompts to evict the first, with the memory numbers shown.

## 7. Gateway

Bind `127.0.0.1:1710` (configurable; 1234 and 11434 are taken by LM Studio and
Ollama). Nothing binds until `servingEnabled` is on and a model is served.

### OpenAI surface

- `GET /v1/models` — every currently served alias.
- `POST /v1/chat/completions` — streaming and non-streaming.
- `POST /v1/completions`, `POST /v1/embeddings` — pass-through where the
  upstream supports them, clean 501 where it does not.

Routing: read `model` from the request body, resolve the alias in the registry,
proxy to that upstream.

Note an interaction with §3.2: because context size is always chosen by a human
at serve time, an OpenAI request naming a *stopped* alias cannot silently start
it — there would be no one to answer the dialog. It returns a 409 naming the
alias and stating that the model must be served from ModelHub first. The
alternative, reusing last-known options to auto-start, would quietly bypass the
decision the dialog exists to capture.

### Aliases

Aliases are the user-facing model name, so they must be stable and predictable:
`qwen3-4b-q4_k_m` from `Qwen/Qwen3-4B` + `Q4_K_M`. Lowercased, non-alphanumerics
collapsed to `-`, deduped with a numeric suffix. Persisted with the served-model
record so an alias never silently changes under a configured editor. Renameable
in the UI.

### Auth

- Bearer token generated when serving is first enabled, persisted in app data
  with a restrictive ACL, rotatable from the UI.
- Loopback bind only — never `0.0.0.0`, not even behind a setting.
- `Origin` header allowlist on `/mcp` (loopback origins and absent Origin only).
  This is the documented DNS-rebinding defense for local MCP HTTP servers; a
  browser page on any site can otherwise reach a localhost port.
- Token never logged, never in events, masked in the UI until revealed.

## 8. MCP surface

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
| `start_model` / `stop_model` | Explicit load/unload with memory cost reported |
| `run_task` | The main one: `{task, context?, model?, max_tokens?}` → completion |
| `continue_task` | `{continuation_id, max_tokens?}` → resumes a truncated `run_task` |
| `serving_status` | What is loaded, memory in use, last error |

`run_task` is the feature that justifies the whole design. It lets Claude Code
push bulk, cheap, or privacy-sensitive work down to a local model — summarize
forty files, classify a changelog, redact a log — without that content leaving
the machine or consuming context. When `model` is omitted, ModelHub picks by
declared capability and current load rather than failing.

`start_model` is subject to §3.2: with no human at the dialog, it starts using
the last-used options for that model and states which options it used in the
result. If the model has never been served, it returns an error directing the
user to serve it once from the app. An agent should not be choosing how much of
someone's RAM to consume.

### Continuations

`continue_task` requires ModelHub to hold the conversation state, because
`llama-server` exposes no resumable generation handle. `continuations.rs` stores
the original messages plus the text generated so far, keyed by `continuation_id`.

The honest costs:

- **Re-prefill.** Continuing re-sends the prompt and the generated prefix
  upstream. `llama-server`'s slot/prompt caching absorbs much of this when the
  model is still loaded and the slot has not been reused, but a continuation
  after a busy interval will pay full prefill again.
- **Memory.** The store is bounded by entry count and TTL, and entries are
  evicted when their model stops. An unbounded map here would be a slow leak
  driven by remote callers.
- **Staleness.** A continuation whose model has been stopped or re-served with
  different options is refused with a clear reason rather than silently resumed
  against a different configuration.

Tool results stay compact — completion text, token counts, finish reason, and
the model that answered. No transcript dumps into the caller's context.

## 9. Frontend

- **Serve page** (replaces the current Runtimes page, which becomes a section of
  it): serving on/off state, engine card (detected / missing / assisted install),
  endpoint, token with reveal/copy/rotate, loaded models with memory use, live
  request count, recent errors.
- **Serve dialog**: context size, GPU offload, alias, and a live — explicitly
  approximate — memory estimate. Prefilled from the model's declared context and
  the last values used. Shown on every serve.
- **Local page**: a Serve button per model card. Disabled with the reason where
  `Servability::No`, and disabled with a link to Settings when serving is off.
  This is the "select in the models list" entry point.
- **Connect panel**: copy-ready snippets, generated with the live port and token —
  `claude mcp add` command, OpenAI base URL + key, Continue/Cline JSON block.
  Copy buttons, no manual transcription.
- Events `serve:status_changed`, `serve:model_changed`, `serve:error` drive the
  UI. No polling, per AGENTS.md §6.
- Tray gets a serving indicator and a stop-all item.

New commands, following existing naming:

```ts
get_serve_status(): Promise<ServeStatus>
get_engine_status(): Promise<EngineStatus[]>
detect_engine(): Promise<EngineStatus>
install_engine_binary(kind: EngineKind): Promise<EngineStatus>
set_engine_path(path: string): Promise<EngineStatus>
get_serve_defaults(modelId: string): Promise<ServeDefaults>
estimate_serve_memory(input: ServeMemoryInput): Promise<ServeMemoryEstimate>
serve_model(input: ServeModelInput): Promise<ServedModel>
stop_served_model(alias: string): Promise<void>
list_served_models(): Promise<ServedModel[]>
rename_served_alias(alias: string, next: string): Promise<ServedModel>
rotate_gateway_token(): Promise<string>
get_connect_snippets(): Promise<ConnectSnippets>
```

## 10. Phasing

Each phase is independently shippable and leaves the app working.

| Phase | Deliverable | Demoable result |
|---|---|---|
| 1 | `servingEnabled` setting, `binaries.rs` detection + version probe + assisted install, engine card UI | App reports whether an engine is present and helps get one |
| 2 | Engine trait, `llamacpp.rs` supervision, `estimate.rs`, `ServeManager`, serve dialog | A GGUF model loads at a chosen context size and answers on its own port |
| 3 | axum gateway, auth, alias registry, OpenAI routes | `curl` and Cline both talk to `:1710/v1` |
| 4 | MCP HTTP endpoint, tool set, continuations, stdio shim, connect snippets | Claude Code calls `run_task` and `continue_task` against a local model |
| 5 | Serve page, per-model button, connect panel, events, tray | Whole loop is mouse-driven |
| 6 | Ollama + LM Studio delegation engines, capability routing, eviction policy | One endpoint fronts all three backends |

Phases 2–4 are the substance; 5 makes it a product; 6 is the aggregation payoff.

## 11. Testing

Everything below is unit-testable with no network and no model files, which
matters because CI cannot download a 4 GB GGUF:

- Alias generation: collision, unicode, very long repo IDs, stability across restarts.
- Registry resolution: unknown alias, stopped alias (409 path), duplicate registration.
- llama-server argument construction from `LocalModel` + `ServeOptions`.
- Engine detection order and precedence; version-too-old warning; checksum mismatch.
- Memory estimation from `LocalModelTechnical`, including models missing the
  fields the estimate needs — it must degrade to "unknown", not to a wrong number.
- `Servability` decisions per format — GGUF yes, safetensors no with reason.
- MCP JSON-RPC framing: `initialize`, `tools/list`, malformed request, unknown tool.
- Continuations: TTL expiry, eviction on model stop, unknown/stale id refused,
  store bounded under repeated calls.
- Auth: missing token, wrong token, non-loopback `Origin` rejected.
- Serving disabled: every serve command refuses cleanly with the same reason.
- Port allocation retry when the chosen port is taken between probe and spawn.

Live inference tests stay manual and `#[ignore]`d, per AGENTS.md §14.

## 12. Risks

| Risk | Mitigation |
|---|---|
| Orphaned llama-server holding VRAM after a crash | Windows Job Object kills children with the parent; startup sweep for stale ModelHub-owned processes |
| Local HTTP server reachable from a browser page | Loopback bind + bearer token + Origin allowlist; nothing binds until serving is enabled and a model is served |
| Users stall at "no engine installed" | Detection covers PATH and common install locations; assisted install is one click; manual path has Browse + Re-detect |
| Memory estimate is wrong and the user trusts it | Labelled approximate in the UI; degrades to "unknown" when GGUF metadata is incomplete; llama-server's own failure is still surfaced verbatim |
| Serve dialog friction on every serve | Prefilled from last-used values, so repeat serves are confirm-and-go |
| Continuation store grows unbounded from remote calls | Bounded entry count, TTL, eviction on model stop |
| Safetensors models cannot be served | Stated plainly in the UI with the reason; not hidden behind a failing button |
| Scope creep into a chat app | No chat UI in ModelHub. Serving only. The client is always an external tool |
| Port collisions | Configurable port, clear error naming the conflicting process |

## 13. Follow-ups to resolve during implementation

1. Verify whether a trustworthy winget/scoop package for llama.cpp exists before
   naming any package-manager command in the assisted-install UI.
2. Confirm the pinned `llama.cpp` release tag and the exact asset names for the
   CPU / CUDA / Vulkan variants, and record the SHA-256 for each.
3. Measure re-prefill cost of `continue_task` against a loaded model to decide
   whether the default `max_tokens` cap is set high enough to make continuations
   rare in practice.
