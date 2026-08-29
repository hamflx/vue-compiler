# vuec_runtime_tests

Internal runtime smoke helpers for vuec generated render output.

This crate is marked `publish = false` and is used by local verification to execute generated Vue 2 and Vue 3 render output against official runtimes.

The tests intentionally fail fast when their pinned official runtime dependencies
are absent. Prepare a fresh checkout before running the workspace suite:

```bash
cargo xtask sync-official-tests --locked
cargo xtask prepare-runtime-smoke
cargo test --workspace
```
