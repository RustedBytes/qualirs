# Codex false-positive audit

Audited on 2026-09-08 using the local Codex checkout at commit
`2cbbf0c9b542a36a1c3284b5e804917635b6f666`.
The baseline QualiRS commit was `0bf3570`.
Codex source was inspected without modification.

All runs analyzed 3,913 Rust files with no parse errors, using the existing
configuration, thresholds, and ignore policies.

| Precision | Before | After | Critical before / after |
|---|---:|---:|---:|
| Conservative | 1,482 | 1,439 | 2 / 2 |
| Balanced | 6,505 | 6,462 | 447 / 447 |
| Exploratory | 8,802 | 8,767 | 447 / 447 |

The audit confirmed 28 incorrect findings across three rules. All 28 disappear
in every precision mode. The total reduction also includes bounded exclusions
where a suggested transformation could not be established; it must not be
interpreted as a count of confirmed defects in the original report.

## Confirmed incorrect findings

Paths below are relative to `codex-rs/`.

### Q0074: synchronous combinators cannot preserve these match arms

Fourteen findings contain an executed await or `?` inside an arm. Moving
that expression into an ordinary map closure either fails compilation or
changes the enclosing error-propagation boundary. One other finding occurs
inside a const constructor, where the suggested runtime combinator cannot
preserve the const API.

| Source | Lines | Evidence |
|---|---|---|
| app-server/src/request_processors/apps_processor.rs | 61 | Await in Some arm |
| app-server/src/request_processors/thread_processor.rs | 4391 | Await and propagation in Some arm |
| cli/src/debug_sandbox.rs | 348 | Await and propagation in Some arm |
| code-mode-protocol/src/host/types.rs | 41 | Const constructor |
| core/src/guardian/review.rs | 814 | Await and propagation |
| core/src/session/mod.rs | 3022 | Await inside constructed tuple |
| ext/goal/src/api.rs | 179, 300 | Await and propagation |
| ext/skills/src/loader/host.rs | 155 | Await in Some arm |
| network-proxy/src/proxy.rs | 1526 | Await on optional task |
| state/src/runtime/projects.rs | 100 | Await and propagation |
| tui/src/app/app_server_events.rs | 313 | Await acquiring optional guard |
| tui/src/tooltips.rs | 293, 297, 301 | Propagation out of the enclosing operation |

The detector now tracks const contexts, resolves standard constructor paths,
and requires simple unguarded bindings. Seven additional matches with opaque
macros or unsupported subpatterns are excluded because equivalence is
unresolved. Three matches transforming both Result branches remain exploratory:
two closures can impose conflicting ownership or borrowing requirements.

### Q0091: existing safe wrappers were missed

All 11 reported foreign declarations already have calls inside safe functions
or methods. Their wrapper names do not necessarily resemble the C symbols.

| Source | Declaration lines |
|---|---|
| cli/src/debug_sandbox/pid_tracker.rs | 31 |
| exec-server/src/no_follow/windows.rs | 63 |
| http-client/src/outbound_proxy/macos.rs | 69, 70, 76 |
| rmcp-client/src/macos_stdio.rs | 96 |
| utils/pty/src/win/job.rs | 29 |
| windows-sandbox-rs/src/bin/managed_deny_probe/win.rs | 12 |
| windows-sandbox-rs/src/file_write.rs | 45 |
| windows-sandbox-rs/src/no_reparse_dir.rs | 58 |
| windows-sandbox-rs/src/setup_launch.rs | 22 |

Wrapper evidence now comes from resolved calls in safe function or method
bodies, including closures used by those functions. Similar names alone no
longer suppress findings. If no local caller is found, the finding is low
confidence: a wrapper can live outside the analyzed module or file. The rule
does not prove that an existing wrapper's implementation is sound.

### Q0061: iterator construction is evaluated before the loop

Two PathBuf constructors at `windows-sandbox-rs/src/audit.rs:78` are
elements of the array iterated by a for loop. Each is evaluated once, before
iteration. They no longer produce findings.

Five other PathBuf constructions remain exploratory: three are stored in
separate owned issue records, and two create path prefixes used to build
owned map keys. Fixed input alone does not prove that their allocations can
be eliminated. Known parser constructors with literal input retain supported
positive coverage.

## Validation and retained findings

- `cargo test --locked --offline`: all 415 tests passed.
- Permanent source reductions cover aliases, custom APIs, shadowing, nested
  execution contexts, const initializers, guards, early exits, and ownership.
- CLI regressions check all precision modes, exact finding locations,
  inline/config ignores, test exclusions, and successful default CI exits.
- Representative supported Option/Result map transformations were compiled
  and executed, comparing results on both branches.
- The unsafe showcase now uses an unsafe caller for its unwrapped FFI example.
- The two conservative critical findings at
  `utils/pty/src/win/psuedocon.rs:118–119` remain: the unsafe Send/Sync
  implementations have no adjacent safety explanations.
- The audit sampled the remaining report; it did not establish that all
  remaining findings are correct.

Raw before/after JSON reports are stored locally in `target/codex-audit/`.
Reproduce with the same checkout and settings:

```powershell
.\target\release\qualirs.exe C:\Users\egors\Downloads\codex --precision conservative --format json --output target/codex-audit/after-conservative.json
.\target\release\qualirs.exe C:\Users\egors\Downloads\codex --precision balanced --format json --output target/codex-audit/after-balanced.json
.\target\release\qualirs.exe C:\Users\egors\Downloads\codex --precision exploratory --format json --output target/codex-audit/after-exploratory.json
```

