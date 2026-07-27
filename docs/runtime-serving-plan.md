# ModelHub as the Local Model Connection Layer — Implementation Plan

Status: proposal, not yet implemented.

## 1. Positioning

ModelHub does not run models. It sits **on top of the runtimes a user already
has** — Ollama, LM Studio, a llama.cpp server, whatever comes next — and turns
them into one reliable endpoint that every coding agent can use.

The pitch in one line: *install any local runtime you like; ModelHub is how your
coding tools reach it.*

This matters because the current situation is bad in a specific way. A developer
with Ollama and LM Studio installed has two model catalogs, two APIs, two ports,
two failure modes, and no way to hand a task to a local model from Claude Code
without hand-rolling something. Every coding tool then re-implements its own
half-working local-model integration. ModelHub absorbs that once.

What ModelHub owns:

- **Discovery** — which runtimes are installed, running, and what they can serve.
- **A unified catalog** — one model list across every runtime, deduplicated.
- **Protocol translation** — MCP for agents, OpenAI-compatible HTTP for editors.
- **Reliability** — the hard part. See §6.

What ModelHub does not own: inference, sampling, quantization, GPU scheduling.
Those belong to the runtime, and the runtime is better at them.

### 1.1 The primary flow

Everything in this plan serves one flow. If a phase does not move this forward,
it is not urgent:

1. User opens Local and sees every model across every runtime, plus what they
   have downloaded.
2. User picks a model and clicks **Connect** — one action.
3. If the model is not in a runtime yet, ModelHub registers it into one the user
   already has (§7). If it is, this step is a no-op.
4. The model is now in the catalog under a stable alias, reachable at
   `/mcp` and `/v1`.
5. Claude Code — already configured once — can hand it a task.

Step 2 is the whole product. A user should never have to know which runtime
ended up serving the model, or edit an editor config more than once.

### 1.2 Alignment with AGENTS.md

`AGENTS.md` §1 lists "Inference runtime" and "Plugin system" as out of MVP scope,
with instructions to confirm before building either. This work was explicitly
requested as post-MVP. §1 and §15 must be amended in the same change set that
lands Phase 1, or future agents will read this code as a scope violation and try
to remove it.

Rules from AGENTS.md that constrain this work and are easy to miss:

- **§7** — every new field must land in the Rust struct, the TypeScript type, the
  UI, and the fixtures together. The catalog and provider types touch all four.
- **§12** — every page needs loading, empty, and error states. The Connect page
  has an unusually rich error surface; it does not get an exemption.
- **§13** — no telemetry, no local paths sent anywhere. A request log (§12) stays
  on the machine and is never uploaded.
- **§14** — no network required for normal unit tests. Every provider adapter
  must be testable against recorded fixtures, not a live runtime.

## 2. What changed from the previous draft, and why

The earlier version of this plan had ModelHub spawn and supervise its own
`llama-server` child processes. That is now a deferred fallback at most (§10),
because it conflicts with the layer positioning: a layer that installs its own
engine is not a layer, it is a competing runtime with extra steps.

Four decisions were settled against the previous draft. Two survive intact, one
narrows, one is void:

| Decision | Status now |
|---|---|
| `run_task` capped, with an explicit `continue_task` | **Unchanged.** Still exactly right — see §8 |
| Serving gated behind a settings flag, off by default | **Unchanged.** See §11 |
| Context size always asked at serve time | **Narrowed.** Only applies where ModelHub issues the load *and* the runtime accepts load-time options. Ollama and LM Studio JIT-load with their own settings; there is nothing to ask in that path. The dialog now appears only on an explicit "Load via ModelHub" action |
| Detect a llama-server binary, offer an assisted install | **Void as written.** Replaced by runtime discovery (§4). The "help them install" instinct survives, but it now points at Ollama or LM Studio rather than at a bare binary |

Flagging the narrowed one because it is the least obvious: it was a good answer
to the question as asked, and the pivot changed the question.

## 3. Architecture

```
   Claude Code        Cline · Continue · Zed · aider · Codex        ModelHub UI
        │ MCP                       │ OpenAI API                        │
        ▼                           ▼                                   ▼
  ┌──────────────────────────────────────────────────────────────────────┐
  │                  ModelHub connection layer  127.0.0.1:1710           │
  │  ┌────────────┐  ┌──────────────┐  ┌───────────────────────────────┐ │
  │  │ MCP server │  │ OpenAI proxy │  │ reliability: health · retry · │ │
  │  │   /mcp     │  │    /v1/*     │  │ cold start · queue · errors   │ │
  │  └────────────┘  └──────────────┘  └───────────────────────────────┘ │
  │              unified catalog · aliases · capability routing          │
  └────────┬─────────────────┬──────────────────┬────────────────────────┘
           │ adapter         │ adapter          │ adapter
           ▼                 ▼                  ▼
      Ollama :11434     LM Studio :1234    llama.cpp server
      (already yours)   (already yours)    (bring your own URL)
```

Every arrow below the layer points at software the user chose and installed.
ModelHub adds no engine of its own.

## 4. Provider adapters

```rust
pub trait RuntimeProvider: Send + Sync {
    fn id(&self) -> ProviderId;
    fn probe(&self) -> ProviderState;                 // installed / running / version
    fn catalog(&self) -> Result<Vec<ProviderModel>, ProviderError>;
    fn loaded(&self) -> Result<Vec<LoadedModel>, ProviderError>;
    fn chat(&self, req: ChatRequest) -> Result<ChatStream, ProviderError>;
    fn capabilities(&self) -> ProviderCapabilities;   // what this adapter can actually do
}
```

`ProviderCapabilities` exists because the runtimes differ, and the layer must not
pretend otherwise. What an adapter cannot do, the UI and the MCP tools must
report as unsupported rather than fail at call time.

| | Ollama | LM Studio | llama.cpp server |
|---|---|---|---|
| Discovery | `/api/tags`, `/api/show` — **already implemented** in `scanner::ollama` | folder scan + REST API — **already implemented** in `scanner::lmstudio` | single configured URL |
| Loaded state | `/api/ps` | REST API model state — partly wired already | `/health` |
| Chat | native `/api/chat` + OpenAI `/v1` | OpenAI `/v1` | OpenAI `/v1` |
| JIT load on request | yes | yes, recent versions | no — one model per process |
| Explicit load/unload | `keep_alive` | `lms load` / `lms unload` CLI | out of scope |
| Load-time context size | `options.num_ctx` | load config | fixed at process start |

Two adapters are already half-built. `scanner::ollama` hits `/api/tags` and
`/api/show`; `scanner::lmstudio` scans the models folder and enriches from LM
Studio's REST API, including model state. The adapter work is largely lifting
that existing code behind the trait, not writing it fresh.

### On wrapping the vendor SDKs

Worth being direct about this, since it was part of the ask. Ollama and LM Studio
ship JS and Python SDKs; ModelHub's backend is Rust. Using those SDKs means
bundling a Node or Python runtime inside a Tauri app — a large dependency, a
large attack surface, and a packaging problem on Windows, in exchange for
convenience wrappers over HTTP calls the app already makes today.

The recommendation is to talk to the documented HTTP APIs directly from Rust —
which is what both scanners already do — and to shell out to a runtime's CLI
(`ollama`, `lms`) only for control-plane operations the HTTP API does not cover,
when that CLI is present. This achieves the "reuse what is already there" goal
without inheriting another language runtime.

If the intent was specifically to reuse SDK code rather than SDK convenience,
that changes the packaging story enough to be worth deciding explicitly. See §16.

## 5. Unified catalog

One model list across every runtime. The problems worth solving here:

- **Deduplication.** `qwen3:4b` in Ollama and `Qwen3-4B-GGUF` in LM Studio are
  often the same weights. Match on digest where available, then on normalized
  name plus quantization plus size, and present one entry that lists which
  runtimes can serve it. Never merge on name alone — quantizations differ.
- **Normalized metadata.** Context length, parameter size, quantization, and
  capability flags reported the same way regardless of source. Most of this
  normalization already exists in `scanner::metadata` and `LocalModel`.
- **Stable aliases.** The alias is what a coding tool writes into its config, so
  it must not change when a model gets loaded elsewhere or a runtime restarts.
  `qwen3-4b-q4_k_m`, persisted, renameable, deduped with a numeric suffix.
- **Provenance.** Each entry says which runtime will actually answer, so a slow
  or wrong response is debuggable.

## 6. The reliability layer

This is the part that justifies ModelHub existing, and the part every ad-hoc
integration gets wrong. "Reliably" was the operative word in the brief.

Local runtimes fail in ways cloud APIs do not:

| Failure | What a naive client does | What ModelHub does |
|---|---|---|
| Runtime not running | hangs, or a raw connection error | fast-fail with a probe, name the runtime, say how to start it |
| Model not loaded, cold start 30s+ | looks like a hang; agent gives up or retries into a thundering herd | report `loading` with progress, hold the request, never silently duplicate it |
| OOM / model too big for VRAM | retries forever into the same wall | classify as terminal, do not retry, report required vs available memory |
| Context length exceeded | opaque 400 | classify, report the model's real limit and the request's size |
| Runtime restarted mid-request | dangling request | detect, fail cleanly, mark upstream unhealthy |
| Two agents hit one runtime at once | queue thrash, model swap storms | serialize per runtime, bounded queue, backpressure |
| Runtime down but still configured | every catalog call stalls | circuit-break, serve last-known catalog marked stale |

Concretely:

- **Health probes** per provider on a short timeout, cached briefly so the
  catalog never blocks on a dead runtime. The existing scanners already use
  500ms–short timeouts and treat "not running" as normal status, which is the
  right instinct to build on.
- **A normalized error taxonomy** — `RuntimeDown`, `ModelNotFound`,
  `ModelLoading`, `OutOfMemory`, `ContextExceeded`, `Timeout`, `Cancelled`,
  `Upstream` — each mapping to an actionable message. Retry policy is a property
  of the variant, not a global setting: transient ones retry with backoff,
  terminal ones never do.
- **Cold-start handling** is the one most worth getting right. A model swap can
  take 30+ seconds. The MCP tool must return something meaningful in that window
  rather than appearing hung — see §8.
- **Per-runtime serialization.** A local runtime with one GPU is not a cloud
  endpoint. Concurrent requests for different models cause repeated load/unload
  thrash that is far slower than queueing. ModelHub queues per runtime with a
  bounded depth and rejects past it rather than growing unboundedly.
- **Cancellation propagates.** A dropped client request cancels the upstream
  request; otherwise a cancelled agent leaves a GPU busy for a minute.
- **Pre-flight context check.** An agent sending forty files to a 4k-context
  model should be told before the request is dispatched, not after the runtime
  returns an opaque 400 a minute later. ModelHub estimates prompt size against
  the model's known context limit and fails fast with both numbers and the
  suggestion to pick a longer-context model from the catalog. The estimate is
  approximate — tokenizers differ per model — so it warns near the limit and only
  hard-fails when clearly over.

### 6.1 The endpoint has to outlive the window

The plan so far assumes ModelHub is running. It frequently will not be, and this
is the most likely way the whole thing feels broken in practice.

Once a developer puts `http://127.0.0.1:1710/mcp` into their Claude Code config,
that endpoint is expected to answer. If it only answers while a desktop window
happens to be open, then closing ModelHub silently breaks every configured tool,
and the failure surfaces inside the *other* tool as a confusing MCP error.

The app already has the pieces to fix this — `minimize_to_tray` and
`start_on_login` are existing settings, and tray support is built:

- Enabling serving implies the gateway runs whenever ModelHub runs, and closing
  the window minimizes to tray rather than exiting. The tray already does this;
  it needs to be non-optional while serving is on, with the reason stated.
- Enabling serving offers to turn on `start_on_login`, so the endpoint is up
  after a reboot without the user thinking about it. Offered, not forced.
- The stdio shim gets a clear, actionable error when the gateway is unreachable —
  naming ModelHub and telling the user to start it — rather than a transport
  failure the host renders as a generic MCP crash.

The alternative, a background service independent of the app, is a much larger
commitment: a Windows service, its own updater, its own permissions story. Not
for this version, but the tray path should not paint us into a corner if it
becomes necessary.

## 7. Making ModelHub's own downloads usable

There is a gap the layer positioning creates and must answer: a GGUF ModelHub
downloaded into the Hugging Face cache is invisible to Ollama and LM Studio. If
ModelHub does not run models, a downloaded model is unreachable — which would
gut the feature the app already has.

The answer is **adopt into runtime**: register a downloaded model with a runtime
the user already has, then serve it through that.

- **Ollama** — generate a Modelfile with `FROM <path-to.gguf>` and run
  `ollama create <alias>`. Documented, and it makes the model a first-class
  Ollama model afterwards.
- **LM Studio** — place or link the GGUF into LM Studio's models directory under
  the publisher/model layout the scanner already understands, then re-index.

This is a strong fit for the positioning: ModelHub finds and fetches models, then
hands them to the runtime the user chose. It also means the download manager,
cache writer, and scanners already built stay valuable rather than becoming
vestigial.

Symlink-versus-copy applies here exactly as it does in the cache writer, and
should reuse that logic and its warning behavior rather than reimplementing it.

## 8. MCP surface

Transports: **streamable HTTP** at `/mcp` (preferred), plus a small
`modelhub-mcp.exe` **stdio shim** for hosts without HTTP transport support. The
shim holds no logic — it reads endpoint and token from app data and proxies.

```
claude mcp add --transport http modelhub http://127.0.0.1:1710/mcp \
  --header "Authorization: Bearer <token>"
```

### Tools

Deliberately small. An agent facing fifteen tools uses none of them well.

| Tool | Purpose |
|---|---|
| `list_models` | Unified catalog: alias, runtime, size, context, capabilities, load state |
| `run_task` | The main one: `{task, context?, model?, max_tokens?}` → completion |
| `continue_task` | `{continuation_id, max_tokens?}` → resumes a truncated `run_task` |
| `list_runtimes` | Which runtimes are installed, running, healthy |
| `load_model` / `unload_model` | Where the runtime supports it; reports unsupported plainly |

Deliberately **not** exposed as tools: adoption (§7), settings changes, model
deletion, and token rotation. Those write to disk, shell out to runtime CLIs, or
change security posture, and they belong to a human in the UI. The MCP surface
stays read-plus-inference: an agent can discover models, load them, and run work
on them, and nothing else.

`run_task` is the reason to build this. It lets Claude Code push bulk, cheap, or
privacy-sensitive work down to a local model — summarize forty files, classify a
changelog, redact a log — without that content leaving the machine or consuming
the orchestrator's context. With `model` omitted, ModelHub routes by declared
capability, current load state, and runtime health, preferring an
already-loaded model over one that would force a cold swap.

**Cold start inside a tool call.** MCP tool results are single-shot, so a 40s
model load looks like a hang. `run_task` returns promptly with
`status: "loading"`, the model, and an estimated wait when a load is required and
the caller did not opt into waiting; the agent can retry or do other work. With
`wait: true` it holds, up to a bounded timeout. Silently blocking for an
unbounded time is the one behavior to avoid — it teaches agents that local models
are broken.

**Continuations.** `run_task` caps `max_tokens` by default; on truncation it
returns `finish_reason: "length"` plus a `continuation_id` that `continue_task`
resumes. The honest costs, unchanged from the previous draft:

- *Re-prefill* — no runtime here exposes a resumable generation handle, so
  ModelHub holds the messages and generated prefix and re-sends. Prompt/slot
  caching in the runtime absorbs much of this while the model stays loaded, but
  a continuation after a busy interval pays full prefill again.
- *Memory* — the store is bounded by count and TTL, evicted when the model
  unloads. An unbounded map here is a leak driven by remote callers.
- *Staleness* — a continuation whose model unloaded or reloaded with different
  options is refused with a reason, not silently resumed against a different
  configuration.

Results stay compact: text, finish reason, token counts, and which runtime and
model answered.

## 9. OpenAI surface

`/v1/models`, `/v1/chat/completions` (streaming and not), `/v1/completions`,
`/v1/embeddings` where the upstream supports them and a clean 501 where it does
not. Routing reads `model` from the body, resolves the alias, and proxies to the
owning provider with the reliability layer in front.

This is what Cline, Continue, Zed, aider, and Codex consume. Claude Code does not
consume OpenAI endpoints for its own inference, so MCP is the only way to reach
it — the two surfaces are complementary, not redundant.

Because JIT-loading runtimes handle cold start themselves, an OpenAI request for
a known-but-unloaded alias is passed straight through — the 409 the previous
draft required no longer applies, since there is no dialog to answer.

That is true per provider, not universally. A llama.cpp server serves whatever
model it was started with and cannot load another; a request for an alias it does
not hold returns a clean error naming the model it does have. The gateway must
branch on `ProviderCapabilities`, not assume JIT everywhere.

## 10. Optional, deferred: ModelHub-managed llama.cpp

Kept explicitly out of the main path, recorded so the decision is not relitigated.

If a user has *no* runtime installed, the layer has nothing to stand on. Two
possible answers: point them at Ollama or LM Studio with a good onboarding
screen (cheap, consistent with the positioning), or have ModelHub spawn its own
`llama-server` (expensive — process supervision, Windows Job Objects, GPU memory
leak risk, binary distribution).

Recommendation: onboarding first. Revisit spawning only if real usage shows
people stuck with no runtime and unwilling to install one. If it is ever built,
it becomes just another `RuntimeProvider` behind the same trait, which is why
the abstraction is worth having from the start.

## 11. Backend layout

```text
src-tauri/src/link/
  mod.rs              LinkManager — Tauri managed state, mirrors DownloadManager
  provider/
    mod.rs            trait RuntimeProvider, ProviderId, ProviderCapabilities
    ollama.rs         wraps + extends scanner::ollama; /api/ps, /api/chat
    lmstudio.rs       wraps + extends scanner::lmstudio; /v1, optional lms CLI
    llamacpp.rs       user-supplied server URL
  catalog.rs          unified catalog, dedup, alias assignment
  health.rs           probes, circuit breaker, cached state
  reliability.rs      error taxonomy, retry policy, per-runtime queue, cancellation
  adopt.rs            register a downloaded model into Ollama / LM Studio
  gateway/
    mod.rs            axum server, bind + shutdown lifecycle
    auth.rs           bearer token, loopback bind, Origin allowlist
    openai.rs         /v1/* proxy routes
    mcp.rs            /mcp streamable HTTP transport
  mcp/
    mod.rs            JSON-RPC framing, initialize / tools/list / tools/call
    tools.rs          tool schemas + handlers
    continuations.rs  bounded, TTL'd continuation store
```

`LinkManager` follows the existing `DownloadManager` pattern: `Arc<Mutex<Inner>>`,
built in `lib.rs` `.setup()`, registered with `app.manage()`, injected via
`tauri::State`.

New dependencies: `axum`, `tokio` (`rt-multi-thread`, `process`), `tokio-stream`,
`futures-util`, `uuid`, `rand`, and the `stream` feature on `reqwest`. The
existing blocking `reqwest` usage in scanners and downloads stays untouched.

New settings: `serving_enabled` (default `false`), `gateway_port` (default
`1710`), per-provider enable flags and base URLs. `AppSettings::sanitized()` and
`apply_patch` force-reset several existing fields deliberately — new fields must
be added without disturbing that.

**Security**, unchanged and non-negotiable: loopback bind only, bearer token
generated on first enable and stored with a restrictive ACL, `Origin` allowlist
on `/mcp` against DNS rebinding, token never logged or emitted in events.

## 12. Frontend

- **Connect page** (replaces Runtimes): serving on/off, endpoint, token with
  reveal/copy/rotate, and a card per runtime showing installed/running/healthy,
  version, model count, and last error.
- **Local page**: each model shows which runtimes can serve it, with **Adopt
  into…** where it is downloaded but not registered anywhere. This is the
  "select it in the model list" entry point.
- **Connect panel**: copy-ready `claude mcp add` command, OpenAI base URL and
  key, and Continue/Cline JSON — generated with the live port and token.
- **Activity**: recent requests with model, runtime, latency, and outcome. Local
  inference fails often enough that a visible request log is a feature, not
  debug output.
- Events `link:runtime_changed`, `link:catalog_changed`, `link:request` drive the
  UI. No polling, per AGENTS.md §6.

Per AGENTS.md §12, each of these needs loading, empty, and error states. They are
not decorative here — they are most of the page:

- *Loading* — probing runtimes, with the short timeouts the scanners already use.
- *Empty* — no runtime installed at all. This is the onboarding moment (§10), not
  a blank panel. It should name Ollama and LM Studio and link to them.
- *Empty, variant* — runtimes present but no models in any of them.
- *Error* — a runtime is installed but unreachable, or reachable but returning
  garbage. Per runtime, not one page-level banner, since one dead runtime must not
  make a healthy one look broken.

## 13. Phasing

The previous draft ordered these by architectural layer, which meant nothing
worked end to end until phase 6 of 7. That is the wrong shape for a plan whose
central risk is "does an MCP-wrapped local model actually feel usable inside
Claude Code?" — a question that stays unanswered the entire time.

Reordered so the first phase is a walking skeleton: one runtime, one path, all
the way through. Everything after it widens or hardens something already proven.

| Phase | Deliverable | Demoable result |
|---|---|---|
| 1 | **Walking skeleton.** `RuntimeProvider` trait, Ollama adapter only, minimal catalog, gateway with auth, `/mcp` with `list_models` + `run_task`, `serving_enabled` setting | **Claude Code runs a task on a local model.** One runtime, no UI beyond a toggle |
| 2 | Reliability core: error taxonomy, health probes, circuit breaker, per-runtime queue, cold-start semantics, cancellation, pre-flight context check | The skeleton stops lying when things break |
| 3 | LM Studio adapter, unified catalog with dedup, stable aliases | One catalog across two runtimes |
| 4 | `adopt.rs` — register downloaded models into a runtime | Download → Connect → usable. The §1.1 flow is closed |
| 5 | OpenAI proxy routes with streaming | Cline, Continue, Zed, aider all work |
| 6 | Connect page, per-model runtime badges, connect panel, activity log, tray and lifecycle work (§6.1) | Mouse-driven, and survives closing the window |
| 7 | `continue_task` and the continuation store, stdio shim | Long generations; hosts without HTTP transport |
| 8 | llama.cpp URL provider, capability routing, no-runtime onboarding | Third runtime, and a real empty state |

Phase 1 is deliberately narrow and deliberately ugly. It exists to retire the
central risk in days rather than weeks. If `run_task` through MCP turns out to
feel wrong in practice, that finding is worth far more before phases 2–8 are
built on top of it than after.

Phase 4 is where the existing app reconnects to the new one — until it lands,
everything ModelHub downloads is still unreachable, and the download manager,
cache writer, and HF scanner remain orphaned from the feature. It is placed ahead
of the OpenAI surface and the UI for that reason.

`continue_task` sits at phase 7 because the cap alone is correct-if-limited
behavior: `run_task` truncates and says so. Resuming is an improvement on a
working thing, not a prerequisite.

## 14. Testing

AGENTS.md §14 requires network-free unit tests, which is also the only way this
is testable at all — CI has no GPU, no Ollama, and no 4 GB model. Every adapter
must therefore be built against recorded response fixtures from day one, not
retrofitted with them later.

- **Provider adapters** — parse recorded `/api/tags`, `/api/show`, `/api/ps`, and
  `/v1/models` payloads, including truncated, empty, and unexpected-shape bodies.
  The existing `tests/fixtures` layout extends naturally to `ollama_api_sample/`
  and `lmstudio_api_sample/`.
- **Catalog dedup** — same model in two runtimes merges; same name with different
  quantizations does not; missing digests fall back to name plus size without
  over-merging.
- **Alias generation** — collisions, unicode, very long repo IDs, and stability
  across restarts. An alias that changes silently breaks a configured editor.
- **Error taxonomy** — every upstream failure shape maps to exactly one variant,
  and the retry policy per variant is asserted. A test that `OutOfMemory` never
  retries is worth more than most of the others.
- **Circuit breaker** — opens after repeated failures, serves stale catalog
  marked stale, closes on recovery.
- **Per-runtime queue** — bounded depth, rejection past it, ordering, and
  cancellation removing a queued request.
- **Pre-flight context check** — warns near the limit, hard-fails clearly over,
  degrades to "unknown" when the model's context length was never discovered.
- **MCP framing** — `initialize`, `tools/list`, malformed JSON-RPC, unknown tool,
  and the `status: "loading"` cold-start response shape.
- **Continuations** — TTL expiry, eviction on unload, unknown or stale id refused,
  store bounded under repeated calls.
- **Auth** — missing token, wrong token, non-loopback `Origin` rejected.
- **Serving disabled** — every command and route refuses cleanly with one reason.
- **Adoption** — Modelfile generation, path escaping for paths with spaces, and
  refusal to overwrite an existing runtime model of the same name.

Anything touching a live runtime stays manual and `#[ignore]`d.

## 15. Risks

| Risk | Mitigation |
|---|---|
| Endpoint dies when the window closes, silently breaking every configured tool | §6.1 — tray-resident while serving, offer `start_on_login`, actionable shim error |
| Cold start reads as a hang and agents learn local models are broken | `status: "loading"` with an estimate; bounded `wait: true`; never block unbounded |
| A dead runtime makes the whole catalog look broken | Per-runtime health and error surfaces; circuit breaker; stale-marked cache |
| Downloads stay unreachable because adoption slips | Adoption is phase 4, ahead of the OpenAI surface and the UI |
| Adapters drift as runtimes change their APIs | Fixture-based tests per adapter; version probe; capability flags rather than assumptions |
| LM Studio REST namespace is already possibly wrong in the scanner | §17.1 — verify before the adapter is built on it |
| Local HTTP surface reachable from a browser page | Loopback bind, bearer token, `Origin` allowlist, nothing bound until serving is on |
| MCP surface used to mutate the machine | Tools are read-plus-inference only; adoption, deletion, and settings stay in the UI |
| Model swap thrash when two tools call at once | Per-runtime serialization with bounded queue; prefer already-loaded models when routing |
| Scope creep into a chat app | No chat UI. The client is always an external tool |
| Layer has nothing to stand on when no runtime is installed | Onboarding empty state (§10, §12); ModelHub-managed llama.cpp stays deferred, not deleted |

## 16. Decisions to confirm

1. **Vendor SDKs vs direct HTTP** (§4). Recommendation: direct HTTP from Rust,
   optional CLI shell-out for control-plane gaps. Wrapping the JS/Python SDKs
   means shipping Node or Python inside the app.
2. **Third runtime in v1.** llama.cpp server by URL is nearly free once the trait
   exists. Jan, vLLM, KoboldCpp are each a small adapter — worth naming which,
   if any, matter to you.
3. **An Anthropic-compatible surface as well?** The brief said "all coding agents
   and tools." MCP lets Claude Code *delegate* to a local model, but it cannot
   make Claude Code *run on* one. A third surface speaking the Anthropic Messages
   API would, since Claude Code honours `ANTHROPIC_BASE_URL`. That is a different
   product claim — substitution rather than orchestration — and local models will
   disappoint anyone who expects them to replace the driving model outright. Not
   planned above; worth an explicit yes or no rather than a silent omission.
4. **Adoption without a runtime CLI.** §7 assumes `ollama` is on `PATH` for
   `ollama create`. If it is not, adoption via Ollama needs either an HTTP path
   or a clear "install the CLI" message. Confirm which before phase 4.

Resolved since the last draft: phase placement for adoption — it is now phase 4,
ahead of the OpenAI surface and the UI, per the reasoning in §13.

## 17. Follow-ups to verify during implementation

1. `scanner::lmstudio` queries `/api/v1/models`, but LM Studio's documented REST
   namespace is `/api/v0`. One of those is wrong or version-dependent, and the
   adapter depends on it. Verify against a running LM Studio before building on
   it — this may be a latent bug in the current scanner.
2. Confirm `/api/ps` response shape for loaded-model state and whether it exposes
   enough to estimate remaining capacity.
3. Confirm which LM Studio versions support JIT loading and the REST API, and
   what the adapter should do on older ones.
4. Measure real cold-start times for a model swap on a representative machine to
   set the `run_task` wait timeout and the estimate returned with
   `status: "loading"`.
