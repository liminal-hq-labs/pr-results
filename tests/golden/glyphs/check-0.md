conclusion: failure
title: 1 failed, 1 passed, 1 flaky

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Rust tests` | ✗ **Failed** | 1 | 1 | 0 | 1 | 2.2 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `pr-results-core` | 1 | 1 | 0 | 0 | 1.9 s |
| `pr-results-github` | 0 | 0 | 0 | 1 | 0.3 s |

#### Failures

<details open>
<summary>Failure 1 of 1</summary>

`pr-results-core` › `junit::sums_suite_time`

``assertion `left == right` failed⏎  left: 1904⏎ right: 1900`` · 1.9 s

````text
thread 'junit::sums_suite_time' (4121) panicked at crates/pr-results-core/src/junit.rs:341:9:
assertion `left == right` failed
  left: 1904
 right: 1900
stack backtrace:
   0: rust_begin_unwind
   1: core::panicking::panic_fmt
   2: core::panicking::assert_failed_inner
   3: pr_results_core::junit::tests::sums_suite_time
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
````

</details>

<details>
<summary>Flaky: passed after a retry (1)</summary>

- ↻ `pr-results-github` › `forge::retries_on_secondary_rate_limit`: failed 2 times, passed on attempt 3

</details>

Annotations: 1.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `pr-results-core` › `junit::sums_suite_time`

``assertion `left == right` failed⏎  left: 1904⏎ right: 1900`` · 1.9 s

````text
thread 'junit::sums_suite_time' (4121) panicked at crates/pr-results-core/src/junit.rs:341:9:
assertion `left == right` failed
  left: 1904
 right: 1900
stack backtrace:
   0: rust_begin_unwind
   1: core::panicking::panic_fmt
   2: core::panicking::assert_failed_inner
   3: pr_results_core::junit::tests::sums_suite_time
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
````

#### Slowest tests

| Test | Time |
| --- | ---: |
| `pr-results-core` › `junit::sums_suite_time` | 1.9 s |
| `pr-results-github` › `forge::retries_on_secondary_rate_limit` | 0.3 s |
| `pr-results-core` › `safe::keeps_text_inert` | 0.0 s |
