# Task List

1. [x] Create Tauri + React + Rust app shell.
2. [x] Add Windows tray support.
3. [x] Implement settings/path resolution.
4. [x] Implement Hugging Face cache scanner.
5. [x] Implement LM Studio scanner.
6. [x] Implement Ollama runtime scanner.
7. [x] Build Local page.
8. [x] Build Hugging Face search.
9. [x] Build download manager.
10. [x] Build HF-compatible cache writer.
11. [x] Refresh Local page after download.
12. [x] Add safe delete/open folder/copy actions.

## Local Model Connection Layer (post-MVP)

Plan: `docs/runtime-serving-plan.md`

13. [ ] Phase 1 — RuntimeProvider trait, Ollama/LM Studio adapters, health probes, serving setting.
14. [ ] Phase 2 — unified catalog, dedup, stable aliases, error taxonomy, per-runtime queue.
15. [ ] Phase 3 — local gateway, auth, OpenAI-compatible proxy routes.
16. [ ] Phase 4 — MCP endpoint, tool set, cold-start semantics, continuations, stdio shim.
17. [ ] Phase 5 — Connect page, runtime badges, connect panel, activity log, events.
18. [ ] Phase 6 — adopt downloaded models into Ollama/LM Studio.
19. [ ] Phase 7 — llama.cpp URL provider, capability routing, no-runtime onboarding.
