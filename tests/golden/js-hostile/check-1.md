conclusion: failure
title: 5 failed, 5 passed, 2 skipped

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Bun` | **Failed** | 5 | 5 | 2 | 0 | 0.0 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `test/math.test.js` | 5 | 1 | 2 | 0 | 0.0 s |
| `test/sub/retry.test.js` | 0 | 4 | 0 | 0 | 0.0 s |

#### Failures

<details open>
<summary>Failure 1 of 5</summary>

`test/math.test.js` › `math > division > rounding` › ``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe``

`expect(received).toBe(expected)` · 0.0 s

````text
AssertionError: expect(received).toBe(expected)

Expected: 5
Received: 4

      at test/math.test.js:29:23
````

</details>

<details open>
<summary>Failure 2 of 5</summary>

`test/sub/retry.test.js` › `retries` › `passes on the second try`

`not yet` · 0.0 s

````text
Error: not yet
      at test/sub/retry.test.js:15:29
````

</details>

<details open>
<summary>Failure 3 of 5</summary>

`test/sub/retry.test.js` › `retries` › `never passes`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

````text
Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
      at test/sub/retry.test.js:21:34
````

</details>

<details open>
<summary>Failure 4 of 5</summary>

`test/sub/retry.test.js` › `retries` › `fails with an empty message`

`Error` · 0.0 s

````text
Error
      at test/sub/retry.test.js:25:21
````

</details>

<details open>
<summary>Failure 5 of 5</summary>

`test/sub/retry.test.js` › `retries` › `throws from the source`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

````text
RangeError: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
      at divide (src/math.js:12:42)
      at test/sub/retry.test.js:29:10
````

</details>

Annotations: 5.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `test/math.test.js` › `math > division > rounding` › ``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe``

`expect(received).toBe(expected)` · 0.0 s

````text
AssertionError: expect(received).toBe(expected)

Expected: 5
Received: 4

      at test/math.test.js:29:23
````

##### 2. `test/sub/retry.test.js` › `retries` › `passes on the second try`

`not yet` · 0.0 s

````text
Error: not yet
      at test/sub/retry.test.js:15:29
````

##### 3. `test/sub/retry.test.js` › `retries` › `never passes`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

````text
Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
      at test/sub/retry.test.js:21:34
````

##### 4. `test/sub/retry.test.js` › `retries` › `fails with an empty message`

`Error` · 0.0 s

````text
Error
      at test/sub/retry.test.js:25:21
````

##### 5. `test/sub/retry.test.js` › `retries` › `throws from the source`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

````text
RangeError: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
      at divide (src/math.js:12:42)
      at test/sub/retry.test.js:29:10
````

#### Slowest tests

| Test | Time |
| --- | ---: |
| `test/math.test.js` › `math > division` › `rejects zero` | 0.0 s |
| `test/math.test.js` › `math > division > rounding` › ``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` | 0.0 s |
| `test/math.test.js` › `math` › `adds` | 0.0 s |
| `test/math.test.js` › `math > division` › `divides` | 0.0 s |
| `test/math.test.js` › `math > division > rounding` › `rounds down` | 0.0 s |
| `test/math.test.js` › `works outside any describe` | 0.0 s |
| `test/sub/retry.test.js` › `retries` › `passes on the second try` | 0.0 s |
| `test/sub/retry.test.js` › `retries` › `never passes` | 0.0 s |
| `test/sub/retry.test.js` › `retries` › `fails with an empty message` | 0.0 s |
| `test/sub/retry.test.js` › `retries` › `throws from the source` | 0.0 s |
