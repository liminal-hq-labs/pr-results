<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### Example checks: 4 failures in 4 suites, 1 required without results

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Rust tests` | **Failed** | 1 | 3 | 1 | 0 | 5.0 s |
| `Frontend tests` | **Failed** | 1 | 1 | 0 | 0 | 0.0 s |
| `Desk steps` | **No results** | 0 | 0 | 0 | 0 | 0.0 s |
| `Playwright` | No results | 0 | 0 | 0 | 0 | 0.0 s |

No results were uploaded for `Desk steps`.

No results were uploaded for `Playwright`.

#### Failures

<details open>
<summary>Failure 1 of 4</summary>

`Rust tests` › `pr-results-core` › `junit::sums_suite_time`

``assertion `left == right` failed⏎  left: 1904⏎ right: 1900`` · at `crates/pr-results-core/src/junit.rs:341` · 1.9 s

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

<details open>
<summary>Failure 2 of 4</summary>

`Rust tests` › `pr-results-core` › `parse::always_fails`

``called `Option::unwrap()` on a `None` value`` · at `crates/pr-results-core/src/parse.rs:17` · 0.1 s

Earlier attempts: 2

````text
thread 'parse::always_fails' panicked at 'called `Option::unwrap()` on a `None` value', crates/pr-results-core/src/parse.rs:17:5
````

</details>

<details open>
<summary>Failure 3 of 4</summary>

`Rust tests` › `pr-results-core` › `parse::killed_by_signal`

Errored: `test aborted with signal 11 (SIGSEGV)` · 3.0 s

</details>

<details open>
<summary>Failure 4 of 4</summary>

`Frontend tests` › `src/lib/format.test.ts` › `formatDuration > rounds up`

`expected '1 m 47 s' to be '1 m 48 s' // Object.is equality` · at `src/lib/format.test.ts` · 0.0 s

````text
AssertionError: expected '1 m 47 s' to be '1 m 48 s' // Object.is equality

Expected: "1 m 48 s"
Received: "1 m 47 s"

 ❯ src/lib/format.test.ts:18:36
 ❯ node_modules/@vitest/runner/dist/index.js:145:11
````

</details>

Commit [`3f9c2e1`](https://forgejo.example.invalid/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://forgejo.example.invalid/example/app/actions/runs/412) · pr-results 0.1.0
