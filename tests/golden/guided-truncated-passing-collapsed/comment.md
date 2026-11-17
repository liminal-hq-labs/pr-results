<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### Example checks: 24 failures in 6 suites, 1 other check needs attention

24 failed tests and 1 other check need attention. 58 tests passed, 8 of them only after a retry.

- ✗ 24 tests failed in `Rust tests`.
- ✗ `Security audit` failed. To run it locally: `cargo audit`
- ↻ 8 tests in `Rust tests` passed only after a retry, so they may be flaky.

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Rust tests` | ✗ **Failed** | 20 | 24 | 0 | 8 | 18.9 s |
| 5 other suites | ✓ Passed | 30 | 0 | 0 | 0 | 6.8 s |

#### Other checks

| Check | Result |
| --- | --- |
| `Security audit` | ✗ **Failed** |
| 6 passing checks | ✓ Passed |

24 failures are not listed here. See the check runs.

The list of 8 flaky tests was left out to fit GitHub's size limit. The table counts them.

The list of slowest tests was left out to fit GitHub's size limit. Each check run lists its ten slowest.

5 passing suites were combined into one row to fit GitHub's size limit.

6 passing checks were combined into one row to fit GitHub's size limit.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
