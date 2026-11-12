conclusion: failure
title: 2 failed, 1 skipped

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Rust tests` | **Failed** | 0 | 2 | 1 | 0 | 3.1 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `pr-results-core` | 0 | 2 | 1 | 0 | 3.1 s |

#### Failures

<details open>
<summary>Failure 1 of 2</summary>

`pr-results-core` › `parse::always_fails`

``called `Option::unwrap()` on a `None` value`` · 0.1 s

Earlier attempts: 2

````text
thread 'parse::always_fails' panicked at 'called `Option::unwrap()` on a `None` value', crates/pr-results-core/src/parse.rs:17:5
````

</details>

<details open>
<summary>Failure 2 of 2</summary>

`pr-results-core` › `parse::killed_by_signal`

Errored: `test aborted with signal 11 (SIGSEGV)` · 3.0 s

</details>

Annotations: 1.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `pr-results-core` › `parse::always_fails`

``called `Option::unwrap()` on a `None` value`` · 0.1 s

````text
thread 'parse::always_fails' panicked at 'called `Option::unwrap()` on a `None` value', crates/pr-results-core/src/parse.rs:17:5
````

##### 2. `pr-results-core` › `parse::killed_by_signal`

Errored: `test aborted with signal 11 (SIGSEGV)` · 3.0 s

#### Slowest tests

| Test | Time |
| --- | ---: |
| `pr-results-core` › `parse::killed_by_signal` | 3.0 s |
| `pr-results-core` › `parse::always_fails` | 0.1 s |
