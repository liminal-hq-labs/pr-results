<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### Example checks: 1 failure in 4 suites, 1 required without results

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Rust tests` | ✗ **Failed** | 1 | 1 | 0 | 1 | 2.2 s |
| `Frontend tests` | ✓ Passed | 3 | 0 | 0 | 0 | 0.0 s |
| `Playwright` | – No results | 0 | 0 | 0 | 0 | 0.0 s |
| `Desk steps` | – **No results** | 0 | 0 | 0 | 0 | 0.0 s |

No results were uploaded for `Playwright`.

No results were uploaded for `Desk steps`.

#### Failures

<details open>
<summary>Failure 1 of 1</summary>

`Rust tests` › `pr-results-core` › `junit::sums_suite_time`

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

- ↻ `Rust tests` › `pr-results-github` › `forge::retries_on_secondary_rate_limit`: failed 2 times, passed on attempt 3

</details>

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
