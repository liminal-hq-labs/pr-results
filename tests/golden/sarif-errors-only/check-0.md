conclusion: failure
title: 1 failed, 2 skipped

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Clippy` | ✗ **Failed** | 0 | 1 | 2 | 0 | 0.0 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `clippy` | 0 | 1 | 2 | 0 | 0.0 s |

#### Failures

<details open>
<summary>Failure 1 of 1</summary>

`clippy` › `crates/pr-results-forge/src/publish.rs` › `clippy::too_many_arguments`

`this function has too many arguments (8/7)` · 0.0 s

</details>

Annotations: 1.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `clippy` › `crates/pr-results-forge/src/publish.rs` › `clippy::too_many_arguments`

`this function has too many arguments (8/7)` · 0.0 s

#### Slowest tests

| Test | Time |
| --- | ---: |
| `clippy` › `crates/pr-results-forge/src/publish.rs` › `clippy::too_many_arguments` | 0.0 s |
