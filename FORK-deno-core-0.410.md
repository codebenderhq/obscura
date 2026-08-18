# Fork: deno-core-0.410 branch

This branch ports `obscura-js` (and the crates that depend on it) from
`deno_core` 0.350 to **0.410.0** (v8 150.4.0). It exists because upstream
`obscura` still tracks `deno_core` 0.350. The goal is to share one V8 engine
with Atlas (which also runs 0.410), and to pick up the v8 150.x pinned-scope
model.

## What changed (vs `main`)

Only 5 files differ from `main`:

| File | Lines | What |
|------|-------|------|
| `crates/obscura-js/src/runtime.rs` | ~360 | The hotspot. See breakdown below. |
| `Cargo.lock` | ~422 | `deno_core` 0.410 + companion crate version bumps. |
| `crates/obscura-js/Cargo.toml` | 6 | `deno_core` / `deno_error` version bumps. |
| `crates/obscura-js/src/ops.rs` | 22 | `deno_core` 0.410 op/serde API port. |
| `crates/obscura-js/src/module_loader.rs` | 16 | `ModuleLoader` trait signature port for 0.410. |

`Atlas` (a separate repo) carries its own mirror changes: `Cargo.toml`,
`build.rs`, `src/main.rs`, `js/runtime.js`. Those never conflict with obscura
merges.

## runtime.rs edits (the merge hotspot)

1. **`IsoEnter` RAII guard** (added near top of file). v8 150.x
   `ContextScope::new` panics if the isolate is not the *current* one on the
   thread that runs the scope. obscura-js executes its page isolate across
   blocking threads (the director's isolate may be current on the pooled
   thread), so every `HandleScope` + `ContextScope` site must enter the isolate
   on the executing thread. The guard calls `isolate.enter()` on construction
   and `isolate.exit()` (unsafe) on drop.
   - Applied at: `create_realm_context`, `take_ops_handoff` (manual
     enter/exit), `share_ops_with_realm` (x2), `run_navigation`, `run_script`
     (x2), and the eval sites (~lines 2188 / 3041). Grep for `IsoEnter::new`
     to find all of them.
   - **Keep this.** It is required by the v8 150.x threading model. If a future
     merge touches any of those functions, re-apply the guard (2-line insertion
     after the `let isolate = &mut *self.runtime.v8_isolate();` line, and change
     `v8::HandleScope::new(isolate)` to
     `v8::HandleScope::new(&mut *iso_guard.iso)`).

2. **Option 1 API** (additive). `new_with_runtime_factory(base_url, proxy, build)`
   lets a host inject the `JsRuntime` instead of obscura spinning up its own
   isolate. `runtime_options(loader)` exposes obscura's exact
   `RuntimeOptions` (extension + `startup_snapshot`). Construction was
   refactored into `build_state` / `finish_build`; the existing `new()` path is
   unchanged.
   - **Safe to keep.** It is additive and does not alter existing behavior.
   - Test: `cargo test -p obscura-js build_from_external_runtime_runs_scripts`.

3. **`take_ops_handoff`** uses a manual `isolate.enter()` / `unsafe {
   isolate.exit() }` block (same reason as the guard).

## Merge guidance (when pulling upstream `main`)

- **Expect conflicts in `runtime.rs` only.** It is a large file upstream churns
  constantly. Our edits are small and localized (8 guard sites + 2 refactors),
  so conflicts are usually a handful of hunks that resolve by re-applying the
  guard at each scope site.
- **`Cargo.toml` / `Cargo.lock` / `ops.rs` / `module_loader.rs`**: conflict
  only if upstream also bumps `deno_core`. In that case re-apply the version
  and re-port `ops.rs` / `module_loader.rs` for the newer API; the `IsoEnter`
  guard logic carries over unchanged (the `enter`/`exit`/`ContextScope`
  requirements are stable in v8 150.x).
- **Keep:** the `IsoEnter` guard (mandatory), the `new_with_runtime_factory`
  API (additive). **Re-evaluate:** the `startup_snapshot` path and cppgc handling
  if upstream changes `deno_crypto` or the bootstrap.
- Merge early and often: small, frequent merges from `upstream/main` produce
  small, easy conflicts. One big catch-up produces a painful `runtime.rs`.

## Verify after a merge

```
cargo build -p obscura-js
cargo test -p obscura-js build_from_external_runtime_runs_scripts
# End-to-end via the CLI (exercises create_realm_context / take_ops_handoff):
cargo build -p obscura-cli
./target/debug/obscura fetch "data:text/html,<p id=x>hi</p>" \
  --eval "(function(){ return document.getElementById('x').textContent; })()"
```
