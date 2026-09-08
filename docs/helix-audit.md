# Helix false-positive audit

Audited on 2026-09-08 using Helix commit
`079a789e8cb08ead67f19e1971a1b7438b37354b`.
The baseline included the Diesel improvements, subsequently committed as
`ff1aa1d`. Helix source was inspected without modification.

All completed runs analyzed 251 files with no parse errors.

| Precision | Before | After | Critical before / after |
|---|---:|---:|---:|
| Conservative | 88 | 86 | 4 / 4 |
| Balanced | 653 | 651 | 38 / 38 |
| Exploratory | 908 | 906 | 38 / 38 |

## Confirmed false positives

Q0087 incorrectly reports missing safety explanations at
`helix-vcs/src/diff/line_cache.rs:95` and
`helix-vcs/src/diff/line_cache.rs:112`.

Each iterator closure contains a lifetime transmute. The preceding comment
explicitly names the transmute and explains backing-storage validity,
invalidation when the stored rope changes, and the lifetime of exposed
references. QualiRS previously discarded that explanation at the closure
boundary.

The fix associates a note with a single named unsafe call inside a synchronous
closure in the same statement, provided the note also describes a validity
condition. Generic construction comments, unrelated calls, multiple unsafe
operations, nested functions/closures, and async bodies do not inherit it.
This is documentation association, not execution-state or soundness inference.

Both warnings disappear in every precision mode. Other findings remain unchanged.

## Findings retained after review

- Q0088 at both transmute locations remains critical: these are actual calls to
  the resolved standard transmute function. Existing documentation does not
  eliminate the need to review lifetime invariants.
- Q0094 at `helix-event/src/hook.rs:90–91` remains critical: the
  unsafe Sync/Send implementations have no adjacent safety explanations.
- Sampled Q0080 findings correctly identify discarded Tokio JoinHandles.
  Several tasks use separate cancellation controllers or report completion
  through channels/callbacks. Discarding a handle is not itself proof of a bug.
- The Q0068 breakpoint finding at
  `helix-view/src/handlers/dap.rs:385` discards a synchronous Result;
  the nearby TODO about futures does not change the actual function signature.

The remaining report was sampled, not exhaustively certified.

## Validation

- `cargo test --locked --offline`: all 425 tests passed.
- Added reductions for both audited closures with positive transmute coverage.
- Regressions cover mismatched calls, multiple unsafe operations, unrelated
  statements, string literals, construction-only notes, nested functions and
  closures, and async contexts.
- CLI regressions verify all precision modes, accurate locations, inline ignores,
  and retention of critical transmute findings and their CI exit status.
- No replacement snippets, severity thresholds, or ignore lists were changed.

The first exploratory scan stalled and was stopped after a single-thread retry
completed. Updated comparisons used `--threads 1`. The stall's cause was
not established and is not claimed fixed by this change.

Raw reports are stored locally under `target/helix-audit/`; the completed
exploratory baseline is `before-exploratory-retry.json`.

```powershell
.\target\release\qualirs.exe C:\Users\egors\Downloads\helix --threads 1 --precision conservative --stats --format json --output target/helix-audit/after-conservative.json
.\target\release\qualirs.exe C:\Users\egors\Downloads\helix --threads 1 --precision balanced --format json --output target/helix-audit/after-balanced.json
.\target\release\qualirs.exe C:\Users\egors\Downloads\helix --threads 1 --precision exploratory --format json --output target/helix-audit/after-exploratory.json
```

