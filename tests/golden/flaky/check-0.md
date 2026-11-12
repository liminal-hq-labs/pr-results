conclusion: success
title: 1 flaky

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Rust tests` | Passed | 0 | 0 | 0 | 1 | 0.3 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `pr-results-github` | 0 | 0 | 0 | 1 | 0.3 s |

<details>
<summary>Flaky: passed after a retry (1)</summary>

- `pr-results-github` › `forge::retries_on_secondary_rate_limit`: failed 2 times, passed on attempt 3

</details>

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Slowest tests

| Test | Time |
| --- | ---: |
| `pr-results-github` › `forge::retries_on_secondary_rate_limit` | 0.3 s |
