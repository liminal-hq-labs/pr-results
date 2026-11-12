<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### Example checks: 2 failures in 4 suites

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Rust tests` | Passed | 3 | 0 | 1 | 0 | 0.3 s |
| `Frontend tests` | **Failed** | 4 | 1 | 0 | 0 | 0.0 s |
| `Desk steps` | **Failed** | 1 | 1 | 2 | 0 | 7.1 s |
| `Playwright` | No results | 0 | 0 | 0 | 0 | 0.0 s |

No results were uploaded for `Playwright`.

#### Failures

<details open>
<summary>Failure 1 of 2</summary>

`Frontend tests` › `src/lib/format.test.ts` › `formatDuration > rounds up`

`expected '1 m 47 s' to be '1 m 48 s' // Object.is equality` · 0.0 s

````text
AssertionError: expected '1 m 47 s' to be '1 m 48 s' // Object.is equality

Expected: "1 m 48 s"
Received: "1 m 47 s"

 ❯ src/lib/format.test.ts:18:36
 ❯ node_modules/@vitest/runner/dist/index.js:145:11
````

</details>

<details open>
<summary>Failure 2 of 2</summary>

`Desk steps` › `Host tests` › `shell crates, host tests`

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

2 input files were skipped because they could not be read or went over a limit.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
