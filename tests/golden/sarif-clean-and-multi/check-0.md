conclusion: failure
title: 1 failed, 1 passed

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Scanners` | ✗ **Failed** | 1 | 1 | 0 | 0 | 0.0 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `semgrep` | 0 | 1 | 0 | 0 | 0.0 s |
| `trivy` | 1 | 0 | 0 | 0 | 0.0 s |

#### Failures

<details open>
<summary>Failure 1 of 1</summary>

`semgrep` › `web/app.js` › `javascript.lang.security.eval`

`eval of a dynamic string` · 0.0 s

</details>

Annotations: 1.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `semgrep` › `web/app.js` › `javascript.lang.security.eval`

`eval of a dynamic string` · 0.0 s

#### Slowest tests

| Test | Time |
| --- | ---: |
| `semgrep` › `web/app.js` › `javascript.lang.security.eval` | 0.0 s |
| `trivy` › `no findings` | 0.0 s |
