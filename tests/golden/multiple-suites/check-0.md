conclusion: success
title: 3 passed, 1 skipped

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Rust tests` | Passed | 3 | 0 | 1 | 0 | 0.3 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `pr-results-core` | 3 | 0 | 0 | 0 | 0.0 s |
| `pr-results::bin/pr-results` | 0 | 0 | 1 | 0 | 0.3 s |

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Slowest tests

| Test | Time |
| --- | ---: |
| `pr-results-core` › `junit::parses_nested_suite` | 0.0 s |
| `pr-results-core` › `junit::parses_empty_suite` | 0.0 s |
| `pr-results-core` › `safe::escapes_pipes` | 0.0 s |
