<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### ✅ Example checks: All 1 passed, 1 flaky

> [!NOTE]
> 🦦 **Jax:** Everything passed, though a test needed a retry. Worth a look.
>
> Everything passed: 1 test in 1 suite, in 0.3 s of test time. 1 test only passed after a retry.

- 🎲 1 test in `Rust tests` passed only after a retry, so it may be flaky.

1 of 1 tests passed ▰▰▰▰▰▰▰▰▰▰

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Rust tests` | ✅ Passed | 0 | 0 | 0 | 1 | 0.3 s |

<details>
<summary>Flaky: passed after a retry (1)</summary>

- 🎲 `pr-results-github` › `forge::retries_on_secondary_rate_limit`: failed 2 times, passed on attempt 3

</details>

<details>
<summary>Slowest tests (1)</summary>

| Test | Time |
| --- | ---: |
| `pr-results-github` › `forge::retries_on_secondary_rate_limit` | 0.3 s |

</details>

♻️ This comment is updated in place with the latest results.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
