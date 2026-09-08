# Polkadot SDK false-positive audit

Audited the working files at `C:\Users\egors\Downloads\polkadot-sdk` on
2026-09-08. The checkout's HEAD is
`8e9a9efc409af2ed41244f8e6ebcbdde2b44ac36`, but its index is empty and the
source files are untracked, so that revision alone does not identify the
audited working snapshot. Source files and the index were not edited.
QualiRS baseline: `d6276cf`.

All completed source scans analyzed 5,113 files with no parse errors.

| Precision | Before | After | Critical before / after |
|---|---:|---:|---:|
| Conservative | 334 | 319 | 5 / 0 |
| Balanced | 4,173 | 4,012 | 269 / 240 |
| Exploratory | 6,624 | 6,485 | 269 / 240 |

The conservative scan now exits successfully. Balanced and exploratory modes
retain critical structural heuristics (such as long functions); they are not
claims of established unsafe behavior.

## Confirmed false positives

Both Q0087 and Q0094 missed existing threading explanations on these five
unsafe marker implementations:

| File | Lines | Explanation |
|---|---|---|
| `cumulus/pallets/parachain-system/src/validate_block/trie_cache.rs` | 157, 158 | "This is safe here since we are single-threaded in WASM" |
| `cumulus/pallets/parachain-system/src/validate_block/trie_recorder.rs` | 144, 145 | Same rationale, shared by adjacent Send/Sync impls |
| `substrate/primitives/runtime-interface/src/wasm.rs` | 96 | "Wasm does not support threads, so this is safe; qed." |

The same correction removes two Q0087 warnings at
`substrate/primitives/runtime/src/offchain/http.rs:107,115`. Both accessors
explain that their header bytes originate as `&str`, documenting the invariant
for `from_utf8_unchecked`.

The fix recognizes bounded causal explanations with a nearby `because` or
`since` following `safe`, or `so`, `therefore`, or `hence` following a rationale
and preceding `safe`. It recognizes documentation, not proof that the code is
sound. A bare assertion of safety remains insufficient.

Existing structural boundaries remain in force: notes on unrelated statements,
different types, string literals, nested functions, closures, or deferred async
bodies do not provide blanket coverage. Adjacent marker impls for the same
type may still share an explanation.

### Search loops: Q0075

At `polkadot/node/network/approval-distribution/src/lib.rs:1720`, the loop logs
an unknown assignment, awaits a reputation update, records metrics, and then
returns false. A synchronous iterator predicate is not an equivalent
transformation. Detection now requires only a conditional boolean return,
without extra branch work, an else branch, or predicate expressions containing
await, early exits, or opaque macros. The simple search loops remain candidates.

### Intentional cleanup: Q0068

At `polkadot/node/core/pvf/src/artifacts.rs:245,249`, the enclosing loop explicitly
documents best-effort cleanup and says to ignore errors. These Results remain
visible at low confidence in exploratory mode. The explanation is scoped to
that loop and does not cover subsequent operations or nested functions,
closures, and async bodies. Ordinary unhandled Results remain high confidence.

### Unproven dependency cycles: Q0004

All 24 self-import cycle claims were incorrect: a module importing names from
its own namespace or children does not prove a dependency cycle. For example,
`bridges/relays/lib-substrate-relay/src/equivocation/mod.rs` declares `source`
and `target` child modules, then imports their types. Those imports are normal
Rust organization. These findings are removed.

The separate import-count heuristic previously counted external crates as
internal dependencies, inspected only the first member of grouped imports,
and arbitrarily excluded roots containing underscores. It now counts distinct
explicit `crate::root::...` import roots, including all grouped imports and
aliases. Unqualified external paths and direct item imports are not evidence
of internal namespaces. The existing threshold is unchanged. A large count
is only a low-confidence coupling hint: its message explicitly states that
reciprocal dependencies have not been verified.

The 146 old Q0004 findings (24 critical self-import claims and 122 warning
heuristics) are replaced by 20 exploratory coupling hints. Seven of those
files had earlier Q0004 findings; 13 are newly recognized by correctly walking
grouped imports. No actual cycle is claimed for these hints.

The old regression that demanded a critical finding for a self-namespace
import was corrected. The showcase now supplies explicit crate-relative
imports for the exploratory hint rather than relying on external crate names.

## Findings retained after review

- Q0057 at `substrate/client/network/sync/src/strategy/chain_sync.rs:2304`
  correctly identifies a full Vec sort used solely to select its median.
- Sampled Q0080 findings discard actual Tokio JoinHandles, including a
  `spawn_blocking` call. Detachment alone does not establish a bug.
- Q0078 remains on the synchronous filesystem calls in the documented PVF
  cleanup loop: intentional error discarding does not make those calls async.
- Q0089 at `substrate/primitives/io/src/global_alloc.rs:81,83` operates on an
  explicitly typed raw pointer.

Outside the five corrected rule codes, counts and finding locations are
unchanged. One existing exploratory Q0019 message can alternate between
`entry` and `store` at `polkadot/node/core/approval-voting/src/ops.rs:201`:
both receivers have nine calls and its HashMap tie selection is unordered.
That reporting variation is not claimed fixed.

## Method

The initial scan waited on a child `cargo tree` process. Retrying with
`CARGO_NET_OFFLINE=true` still spent most of its time on Cargo's shared
package-cache lock and per-crate dependency resolution. Those runs were stopped.
The completed comparison scans use standalone source analysis: their process
PATH contains only the Windows system directory, so Cargo is unavailable.
Q0011 dependency-version findings are therefore outside this audit's scope.
No detector configuration, ignore list, or threshold was changed. The dependency
subprocess bottleneck is not claimed fixed.

The baseline was consolidated into one exploratory run. Conservative and
balanced counts are derived from the report's high and high/medium confidence
findings, respectively: precision is a reporting filter in the engine and
does not change detector execution. CLI regressions exercise all three modes
directly. Raw reports and the recorded target Git status are under
`target/polkadot-audit/`.

The final reports are produced by the rebuilt release executable. To reproduce
the source-only comparison in a separate PowerShell process:

```powershell
$env:PATH = "$env:SystemRoot\System32"
.\target\release\qualirs.exe C:\Users\egors\Downloads\polkadot-sdk --threads 4 --precision conservative --stats --format json --output target/polkadot-audit/after-conservative.json
.\target\release\qualirs.exe C:\Users\egors\Downloads\polkadot-sdk --threads 4 --precision balanced --stats --format json --output target/polkadot-audit/after-balanced.json
.\target\release\qualirs.exe C:\Users\egors\Downloads\polkadot-sdk --threads 4 --precision exploratory --stats --format json --output target/polkadot-audit/after-exploratory.json
```

## Validation

- Direct detector regressions pair documented implementations with genuine
  missing-documentation findings.
- `cargo test --locked --offline`: all 436 tests pass, including 11 new audit tests.
- `cargo build --release --locked --offline` succeeds.
- QualiRS's own default scan reports no findings.
- A representative loop-to-`any` transformation is compiled with rustc and
  executed on empty inputs, matching inputs, and nonmatching inputs; return
  values and predicate evaluation counts agree.
- CLI tests cover conservative, balanced, and exploratory output, exact line
  locations, inline/config ignores, and critical exit status.
- The documented reductions no longer cause critical CI exits; undocumented
  marker impls still do.
- No thresholds, ignore lists, or replacement snippets were changed. Detector
  confidence changes are tied to the evidence described above.

The remaining findings require review; this audit is not a soundness assessment
of Polkadot SDK or a claim of zero false positives.
