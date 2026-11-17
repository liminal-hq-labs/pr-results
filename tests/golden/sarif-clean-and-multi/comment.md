<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### Example checks: 1 failure in 2 suites

1 failed test needs attention. 2 tests passed.

- ✗ 1 test failed in `Scanners`.

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Scanners` | ✗ **Failed** | 1 | 1 | 0 | 0 | 0.0 s |
| `Semgrep` | ✓ Passed | 1 | 0 | 0 | 0 | 0.0 s |

#### Failures

<details open>
<summary>Failure 1 of 1</summary>

`Scanners` › `semgrep` › `web/app.js` › `javascript.lang.security.eval`

`eval of a dynamic string` · 0.0 s

</details>

This comment is updated in place with the latest results.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
