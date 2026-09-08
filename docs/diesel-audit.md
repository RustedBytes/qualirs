# Diesel false-positive audit

Audited on 2026-09-08 using Diesel commit
`6fa6ed01b24b24248ab2a611698d0a7c6a2e9120`.
Baseline QualiRS: `2f46eef`. Diesel source was inspected without modification.

All precision modes analyzed 869 files with no parse errors. Existing
thresholds, configuration, and ignore policies were preserved.

| Precision | Findings before | Findings after | Critical before / after |
|---|---:|---:|---:|
| Conservative | 269 | 252 | 7 / 0 |
| Balanced | 791 | 771 | 36 / 29 |
| Exploratory | 1,051 | 1,032 | 36 / 29 |

Nineteen confirmed incorrect findings are removed in all modes. One additional
finding about a documentation helper is reworded and retained only as an
exploratory candidate because its production build context is unresolved.
Findings outside the four adjusted rules are unchanged.

## Safety explanations: Q0087 and Q0094

Seven unsafe Send/Sync implementations already have adjacent explanations,
but the previous detector required the word SAFETY. Each caused both a
Q0087 warning and a Q0094 critical finding:

| Diesel source | Line | Existing explanation |
|---|---:|---|
| diesel/src/mysql_like/connection/mod.rs | 120 | Documents the client library's thread guarantees |
| diesel/src/mysql_like/connection/stmt/mod.rs | 28 | Documents the client library's thread guarantees |
| diesel/src/pg/connection/mod.rs | 133 | Cites documented transfer between threads |
| diesel/src/r2d2.rs | 210 | States that the manager does not hold a reference to T |
| diesel/src/sqlite/connection/mod.rs | 223 | Explains the invariant preventing leaked connection or statement references |
| diesel/src/sqlite/connection/sqlite_value.rs | 92 | Explains why duplication makes the value safe to send |
| diesel/src/sqlite/connection/stmt.rs | 32 | Explains the invariant preventing leaked connection or statement references |

The reference-ownership explanation follows an allow attribute on the same
line. Such comments now attach to the attributed item, while trailing comments
on unrelated statements do not.

Two more Q0087 warnings are removed:

- `diesel/src/mysql_like/connection/bind.rs:94` explains that invalidated
  buffers are rebound before the function returns.
- `diesel/src/mysql_like/connection/stmt/mod.rs:65` explains the stable
  input-bind invariant inside the callback.

Recognition is bounded to explicit safety markers and specific forms of
natural rationale about safety, invariants, documented thread guarantees, or
reference ownership. It checks for a written explanation; it does not establish
that an unsafe operation or implementation is sound. Unexplained unsafe code
continues to produce findings.

## File locking and binding usage: Q0063

`diesel_cli/src/migrations/mod.rs:275` uses
`fd_lock::RwLock` to coordinate access to a migration directory.
Replacing it with ordinary mutable state would remove the operating-system
lock.

The detector now requires a resolved `std::sync::Mutex` or
`std::sync::RwLock` constructor. It also tracks binding identity and all
later uses before claiming that a lock is unshared. Moves, borrows, aliases,
captures, and opaque macro uses suppress the suggestion. Independent functions,
shadowed bindings, and similarly named fields are kept separate.

## Dependency declarations and source context: Q0010

Two findings incorrectly treated quickcheck as development-only:

- `diesel/src/pg/types/date_and_time/quickcheck_impls.rs:7`
- `diesel/src/pg/types/floats/quickcheck_impls.rs:3`

Diesel declares quickcheck both as an optional normal dependency and as a
development dependency. The detector now parses TOML and subtracts normal
dependency names from development-only names, including target sections,
renames, subtable syntax, and workspace-inherited declarations.

`diesel/src/doctest_setup.rs:3` imports dotenvy for documentation examples.
The crate includes this helper from documentation in `diesel/src/lib.rs`.
A standalone source import does not establish production use. Known crate
entry points retain medium confidence; other files now receive low-confidence
guidance to verify the build context before changing dependencies.

## Validation

- `cargo test --locked --offline`: all 422 tests passed.
- Added permanent reductions for every removed finding, with supported positives.
- Covered imported aliases, custom APIs, shadowing, later sharing, deferred
  execution, attribute comments, misleading literals, and unrelated comments.
- CLI tests cover all precision modes, accurate locations, inline/config ignores,
  test inclusion settings, and successful default CI exits for audited examples.
- A supported local-mutex simplification was compiled and executed; both versions
  returned identical values for representative inputs.
- Reports were compared as sets of findings to account for parallel scan ordering.

The remaining findings were sampled, not exhaustively certified. For example,
the eleven `nth(0)` findings in `diesel_cli/src/config.rs` operate on
collection iterators and remain useful review suggestions.

Raw reports are available locally under `target/diesel-audit/`.
Reproduce with:

```powershell
.\target\release\qualirs.exe C:\Users\egors\Downloads\diesel --precision conservative --stats --format json --output target/diesel-audit/after-conservative.json
.\target\release\qualirs.exe C:\Users\egors\Downloads\diesel --precision balanced --format json --output target/diesel-audit/after-balanced.json
.\target\release\qualirs.exe C:\Users\egors\Downloads\diesel --precision exploratory --format json --output target/diesel-audit/after-exploratory.json
```

