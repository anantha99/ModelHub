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

13. [ ] Phase 1 — walking skeleton: provider trait, Ollama adapter, gateway, MCP run_task.
14. [ ] Phase 2 — reliability core: error taxonomy, health, circuit breaker, queue, cold start.
15. [ ] Phase 3 — LM Studio adapter, unified catalog, dedup, stable aliases.
16. [ ] Phase 4 — adopt downloaded models into a runtime.
17. [ ] Phase 5 — OpenAI-compatible proxy routes with streaming.
18. [ ] Phase 6 — Connect page, runtime badges, connect panel, activity log, tray lifecycle.
19. [ ] Phase 7 — continue_task and continuation store, stdio shim.
20. [ ] Phase 8 — llama.cpp URL provider, capability routing, no-runtime onboarding.
