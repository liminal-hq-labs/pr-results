conclusion: failure
title: 5 failed

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Scan` | ✗ **Failed** | 0 | 5 | 0 | 0 | 0.0 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `@everyone GH-123 <img src=x onerror=alert(1)>` | 0 | 5 | 0 | 0 | 0.0 s |

#### Failures

<details open>
<summary>Failure 1 of 5</summary>

`@everyone GH-123 <img src=x onerror=alert(1)>` › ``@owner/team #1 `x` \| pipe``

`[y](https://example.invalid) <script>alert(1)</script> GH-123 \u{202e} reversed` · 0.0 s

</details>

<details open>
<summary>Failure 2 of 5</summary>

`@everyone GH-123 <img src=x onerror=alert(1)>` › `absolute`

`an absolute path cannot be mapped to the commit` · 0.0 s

</details>

<details open>
<summary>Failure 3 of 5</summary>

`@everyone GH-123 <img src=x onerror=alert(1)>` › `url`

`a url` · 0.0 s

</details>

<details open>
<summary>Failure 4 of 5</summary>

`@everyone GH-123 <img src=x onerror=alert(1)>` › `src/x.rs` › `bad-lines`

`a line of zero and a line that does not fit` · 0.0 s

</details>

<details open>
<summary>Failure 5 of 5</summary>

`@everyone GH-123 <img src=x onerror=alert(1)>` › `src/ok.rs` › `ok`

`a real location` · 0.0 s

</details>

Annotations: 2.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `@everyone GH-123 <img src=x onerror=alert(1)>` › ``@owner/team #1 `x` \| pipe``

`[y](https://example.invalid) <script>alert(1)</script> GH-123 \u{202e} reversed` · 0.0 s

##### 2. `@everyone GH-123 <img src=x onerror=alert(1)>` › `absolute`

`an absolute path cannot be mapped to the commit` · 0.0 s

##### 3. `@everyone GH-123 <img src=x onerror=alert(1)>` › `url`

`a url` · 0.0 s

##### 4. `@everyone GH-123 <img src=x onerror=alert(1)>` › `src/x.rs` › `bad-lines`

`a line of zero and a line that does not fit` · 0.0 s

##### 5. `@everyone GH-123 <img src=x onerror=alert(1)>` › `src/ok.rs` › `ok`

`a real location` · 0.0 s

#### Slowest tests

| Test | Time |
| --- | ---: |
| `@everyone GH-123 <img src=x onerror=alert(1)>` › ``@owner/team #1 `x` \| pipe`` | 0.0 s |
| `@everyone GH-123 <img src=x onerror=alert(1)>` › `absolute` | 0.0 s |
| `@everyone GH-123 <img src=x onerror=alert(1)>` › `url` | 0.0 s |
| `@everyone GH-123 <img src=x onerror=alert(1)>` › `src/x.rs` › `bad-lines` | 0.0 s |
| `@everyone GH-123 <img src=x onerror=alert(1)>` › `src/ok.rs` › `ok` | 0.0 s |
