conclusion: failure
title: 1 failed, 1 passed, 2 skipped

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Desk steps` | **Failed** | 1 | 1 | 2 | 0 | 7.1 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `Host tests` | 1 | 1 | 2 | 0 | 7.1 s |

#### Failures

<details open>
<summary>Failure 1 of 1</summary>

`Host tests` › `shell crates, host tests`

`exit status 101` · 6.3 s

````text
test gfx::fills_rect ... FAILED

failures:

---- gfx::fills_rect stdout ----
thread 'gfx::fills_rect' panicked at shell/desk-gfx/src/fill.rs:33:9:
assertion failed: pixel == 0x0f

test result: FAILED. 41 passed; 1 failed; 0 ignored
````

</details>

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `Host tests` › `shell crates, host tests`

`exit status 101` · 6.3 s

````text
test gfx::fills_rect ... FAILED

failures:

---- gfx::fills_rect stdout ----
thread 'gfx::fills_rect' panicked at shell/desk-gfx/src/fill.rs:33:9:
assertion failed: pixel == 0x0f

test result: FAILED. 41 passed; 1 failed; 0 ignored
````

#### Slowest tests

| Test | Time |
| --- | ---: |
| `Host tests` › `shell crates, host tests` | 6.3 s |
| `Host tests` › `Format check` | 0.8 s |
