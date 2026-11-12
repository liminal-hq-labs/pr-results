conclusion: failure
title: 1 failed, 4 passed

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Frontend tests` | **Failed** | 4 | 1 | 0 | 0 | 0.0 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `src/lib/format.test.ts` | 3 | 1 | 0 | 0 | 0.0 s |
| `src/lib/path.test.ts` | 1 | 0 | 0 | 0 | 0.0 s |

#### Failures

<details open>
<summary>Failure 1 of 1</summary>

`src/lib/format.test.ts` › `formatDuration > rounds up`

`expected '1 m 47 s' to be '1 m 48 s' // Object.is equality` · 0.0 s

````text
AssertionError: expected '1 m 47 s' to be '1 m 48 s' // Object.is equality

Expected: "1 m 48 s"
Received: "1 m 47 s"

 ❯ src/lib/format.test.ts:18:36
 ❯ node_modules/@vitest/runner/dist/index.js:145:11
````

</details>

Annotations: 1.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `src/lib/format.test.ts` › `formatDuration > rounds up`

`expected '1 m 47 s' to be '1 m 48 s' // Object.is equality` · 0.0 s

````text
AssertionError: expected '1 m 47 s' to be '1 m 48 s' // Object.is equality

Expected: "1 m 48 s"
Received: "1 m 47 s"

 ❯ src/lib/format.test.ts:18:36
 ❯ node_modules/@vitest/runner/dist/index.js:145:11
````

#### Slowest tests

| Test | Time |
| --- | ---: |
| `src/lib/format.test.ts` › `formatDuration > rounds up` | 0.0 s |
| `src/lib/format.test.ts` › `formatDuration > writes minutes and seconds` | 0.0 s |
| `src/lib/format.test.ts` › `formatDuration > writes sub-second times` | 0.0 s |
| `src/lib/format.test.ts` › `formatDuration > keeps zero` | 0.0 s |
| `src/lib/path.test.ts` › `joins segments` | 0.0 s |
