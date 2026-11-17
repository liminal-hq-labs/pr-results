<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### Example checks: 8 failures in 1 suite

8 failed tests need attention.

- ✗ 8 tests failed in `Rust tests`.

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Rust tests` | ✗ **Failed** | 0 | 8 | 0 | 0 | 1.6 s |

#### Failures

<details open>
<summary>Failure 1 of 8</summary>

`app` › `it_is_fine`

`boom` · 0.2 s

Run it locally:

````text
cargo nextest run -E 'binary_id(=app) & test(=it_is_fine)'
````

</details>

<details open>
<summary>Failure 2 of 8</summary>

`app` › `quote'; echo hi; '`

`boom` · 0.2 s

</details>

<details open>
<summary>Failure 3 of 8</summary>

`app` › `has space`

`boom` · 0.2 s

</details>

<details open>
<summary>Failure 4 of 8</summary>

`app` › `` back`tick` ``

`boom` · 0.2 s

</details>

<details open>
<summary>Failure 5 of 8</summary>

`app` › `dollar$(id)`

`boom` · 0.2 s

</details>

<details open>
<summary>Failure 6 of 8</summary>

`app` › `line break`

`boom` · 0.2 s

</details>

<details open>
<summary>Failure 7 of 8</summary>

`app` › `café`

`boom` · 0.2 s

</details>

<details open>
<summary>Failure 8 of 8</summary>

`app` › `test(=x) \| test(=y)`

`boom` · 0.2 s

</details>

<details>
<summary>Slowest tests (5)</summary>

| Test | Time |
| --- | ---: |
| `app` › `it_is_fine` | 0.2 s |
| `app` › `quote'; echo hi; '` | 0.2 s |
| `app` › `has space` | 0.2 s |
| `app` › `` back`tick` `` | 0.2 s |
| `app` › `dollar$(id)` | 0.2 s |

</details>

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
