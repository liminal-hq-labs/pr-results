conclusion: failure
title: 3 failed, 6 passed, 2 skipped

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Vitest` | **Failed** | 6 | 3 | 2 | 0 | 0.0 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `test/math.test.js` | 5 | 1 | 2 | 0 | 0.0 s |
| `test/sub/retry.test.js` | 1 | 2 | 0 | 0 | 0.0 s |

#### Failures

<details open>
<summary>Failure 1 of 3</summary>

`test/math.test.js` › ``math > division > rounding > @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe``

`expected 4 to be 5 // Object.is equality` · 0.0 s

Earlier attempts: 2

````text
AssertionError: expected 4 to be 5 // Object.is equality

- Expected
+ Received

- 5
+ 4

 ❯ test/math.test.js:29:23
````

</details>

<details open>
<summary>Failure 2 of 3</summary>

`test/sub/retry.test.js` › `retries > never passes`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

Earlier attempts: 2

````text
Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
 ❯ test/sub/retry.test.js:21:9
````

</details>

<details open>
<summary>Failure 3 of 3</summary>

`test/sub/retry.test.js` › `retries > throws from the source`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

Earlier attempts: 2

````text
RangeError: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
 ❯ divide src/math.js:12:9
 ❯ test/sub/retry.test.js:25:10
````

</details>

Annotations: 3.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `test/math.test.js` › ``math > division > rounding > @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe``

`expected 4 to be 5 // Object.is equality` · 0.0 s

````text
AssertionError: expected 4 to be 5 // Object.is equality

- Expected
+ Received

- 5
+ 4

 ❯ test/math.test.js:29:23
````

##### 2. `test/sub/retry.test.js` › `retries > never passes`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

````text
Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
 ❯ test/sub/retry.test.js:21:9
````

##### 3. `test/sub/retry.test.js` › `retries > throws from the source`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

````text
RangeError: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
 ❯ divide src/math.js:12:9
 ❯ test/sub/retry.test.js:25:10
````

#### Slowest tests

| Test | Time |
| --- | ---: |
| `test/math.test.js` › ``math > division > rounding > @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` | 0.0 s |
| `test/sub/retry.test.js` › `retries > passes on the second try` | 0.0 s |
| `test/math.test.js` › `math > adds` | 0.0 s |
| `test/sub/retry.test.js` › `retries > never passes` | 0.0 s |
| `test/sub/retry.test.js` › `retries > throws from the source` | 0.0 s |
| `test/math.test.js` › `math > division > divides` | 0.0 s |
| `test/math.test.js` › `math > division > rejects zero` | 0.0 s |
| `test/math.test.js` › `math > division > rounding > rounds down` | 0.0 s |
| `test/math.test.js` › `works outside any describe` | 0.0 s |
