# Exploratory self-scan audit

The report supplied on 2026-09-08 was reproduced at commit `b312d48`:
154 files, 37 findings, including three critical findings. The audit focuses
on detector correctness; an exploratory scan is not expected to be empty.

| Mode | Before | After | Critical before / after |
|---|---:|---:|---:|
| Conservative | 0 | 0 | 0 / 0 |
| Balanced | 30 | 33 | 3 / 2 |
| Exploratory | 37 | 40 | 3 / 2 |

The updated scan covers 155 files, including the new regression test file.
Four incorrect findings disappear and seven previously missed method metrics
become visible. No parse errors are reported. Baseline mode counts are derived
from the original exploratory report's confidence levels; updated modes are
also checked through the CLI.

## Incorrect findings corrected

- **Q0079, `src/cli/stats.rs:47`:** `emit` locks stderr in one branch and
  stdout in the alternative branch. The old detector counted any methods
  named `lock`, `read`, or `write` anywhere inside a function, including
  mutually exclusive branches and nested execution bodies. It did not
  establish that guards overlapped or that receivers were synchronization
  primitives.
- **Q0035, `src/detectors/policy.rs:148`:** the reported complexity of 47
  included branches from methods of a locally declared visitor. Those methods
  have their own execution bodies and must not inflate `analysis_view`.
- **Q0035, `src/detectors/implementation/derivable_impl.rs:72`:** `fieldwise`
  included the branches of its iterator predicate closure in its function
  complexity. The closure is now excluded from the enclosing body's metric.
- **Q0036, `src/detectors/policy.rs:110`:** `cfg_without_test` was reported
  as five levels deep because every `else if` increased the nesting count.
  An `else if` continues the same decision chain; only actual nesting adds
  depth.

Q0079 now requires a locally established synchronous lock receiver and a
still-tracked synchronous guard at another blocking acquisition. Unknown
APIs, nonblocking attempts, separate branches, and deferred bodies do not
supply that evidence. Guard moves followed by explicit drops also invalidate
the original binding's guard evidence; the same correction benefits Q0084.
The remaining overlap hint is low confidence: it does not establish a
deadlock cycle or inconsistent order across callers.

Q0035 and Q0036 measure named functions, methods, and trait default methods
independently, including nested helpers and inline modules. Nested item
bodies, closures, async blocks, and const blocks do not inflate the outer
metric. Method coverage reveals existing complexity that was previously
missed, so fixing these false positives can increase the overall count.

## Unproven hints and retained findings

Q0066 used to say that `SafetyDocs::contains` was called seven times in
`source_notes.rs`. It actually counted same-named methods on strings and
sets. The exploratory hint now explicitly describes name matches and
unverified receiver types and runtime frequency. This wording change does
not add receiver type resolution or prove that inlining is beneficial.

The long-function finding on `analysis_view` remains: Q0030 measures its
physical source span, including local declarations. That is distinct from
its own execution complexity. The remaining critical complexity finding
on `Context::expr` is a newly measured method with complexity 31.
Neither structural measurement establishes a runtime safety defect.

Other structural findings, the dependency-version finding, and unproven
performance hints remain available for review. Thresholds and ignore lists
were not changed to force an empty report.

`cargo tree --locked --offline -d --depth 1` confirms Q0011: `ring` uses
`getrandom 0.2.17`, while `tempfile` uses `getrandom 0.4.3`.

## Validation

- `cargo test --locked --offline`: 446 tests passed.
- The focused regression suite also passed after extracting the deadlock
  report formatter. `cargo build --release --locked --offline` succeeds.
- Rebuilt release reports match the tested debug report in all findings;
  conservative, balanced, and exploratory modes were run directly. Default
  mode exits successfully with zero findings.
- Ten new regression tests cover exclusive output branches, standard/imported
  locks, custom APIs, shadowing, nonblocking attempts, temporary guards,
  explicit drops and moves, nested execution bodies, independently measured
  helpers/methods, `else if` ladders, and genuine deeply nested conditions.
- CLI regressions cover precision modes, exact acquisition locations, inline
  ignores, test exclusions, `skip_tests = false`, and critical exit status.
- Inline hints remain low confidence and no longer claim that method-name
  matches identify calls to the candidate.
- Raw before/after reports are under `target/self-audit/`.

No replacement snippets or automatic source transformations were introduced.

## Follow-up: refactoring the reported code

After the detector audit, the remaining source-level issues were refactored
without changing detector thresholds, confidence policies, or ignore lists.

| Mode | Before refactoring | After refactoring | Critical before / after |
|---|---:|---:|---:|
| Conservative | 0 | 0 | 0 / 0 |
| Balanced | 33 | 2 | 2 / 0 |
| Exploratory | 40 | 6 | 2 / 0 |

The refactored tree contains 156 Rust files. The changes separate item-name
resolution from expression/type inference, move test-exclusion visitors out of
`analysis_view`, and put collection-use analysis in a dedicated internal module.
Detector callbacks now delegate to named helpers for eligibility, evidence, and
reporting. Constructor and public-error checks use smaller predicates and flatter
traversal. Lock-use state owns its finding decision.

Duration formatting uses named unit constants and reserves its four possible
output parts. FFI declaration collection reserves an upper bound from the foreign
item counts. Identifier strings are created once per lookup instead of repeatedly
inside a binding search, and FFI diagnostics format identifiers directly.

Six findings remain deliberately visible: the 15-variant `Kind` taxonomy, four
unproven inline candidates, and the transitive `getrandom` version duplication.
Splitting the taxonomy solely to meet a variant-count threshold would obscure its
purpose; adding inline attributes needs profiling, and dependency upgrades are a
separate change. The old inline hint on `stmt_contains_ident` also disappears
because its callers moved into the collection module; this is a consequence of
the detector's file-local call counting, not evidence of faster code.

Validation: all 446 offline tests pass. Both the original and refactored detectors
were run on the same archived source tree at `45141bf`; all 40 complete finding
records match, including locations, confidence, messages, and suggestions. This
checks behavior on that snapshot alongside the existing positive and negative
regressions, rather than inferring correctness from a smaller self-scan count.
The release build and all three precision modes were also checked, with no parse
errors or critical exits. Reports are under `target/self-audit/refactor-*`.
