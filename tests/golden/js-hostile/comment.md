<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### ＠everyone ＠owner/team GH－123 ＃1 \`x\` \[y\]\(https&#58;//example\.invalid\) &lt;img src\=x onerror\=alert\(1\)&gt; \| pi… (2 more characters omitted): 17 failures in 4 suites

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Vitest` | **Failed** | 6 | 3 | 2 | 0 | 0.0 s |
| `Bun` | **Failed** | 5 | 5 | 2 | 0 | 0.0 s |
| `Jest` | **Failed** | 7 | 3 | 1 | 0 | 0.2 s |
| `Playwright` | **Failed** | 10 | 6 | 4 | 0 | 0.1 s |

#### Failures

<details open>
<summary>Failure 1 of 17</summary>

`Vitest` › `test/math.test.js` › ``math > division > rounding > @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe``

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
<summary>Failure 2 of 17</summary>

`Vitest` › `test/sub/retry.test.js` › `retries > never passes`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

Earlier attempts: 2

````text
Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
 ❯ test/sub/retry.test.js:21:9
````

</details>

<details open>
<summary>Failure 3 of 17</summary>

`Vitest` › `test/sub/retry.test.js` › `retries > throws from the source`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

Earlier attempts: 2

````text
RangeError: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
 ❯ divide src/math.js:12:9
 ❯ test/sub/retry.test.js:25:10
````

</details>

<details open>
<summary>Failure 4 of 17</summary>

`Bun` › `test/math.test.js` › `math > division > rounding` › ``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe``

`expect(received).toBe(expected)` · 0.0 s

````text
AssertionError: expect(received).toBe(expected)

Expected: 5
Received: 4

      at test/math.test.js:29:23
````

</details>

<details open>
<summary>Failure 5 of 17</summary>

`Bun` › `test/sub/retry.test.js` › `retries` › `passes on the second try`

`not yet` · 0.0 s

````text
Error: not yet
      at test/sub/retry.test.js:15:29
````

</details>

<details open>
<summary>Failure 6 of 17</summary>

`Bun` › `test/sub/retry.test.js` › `retries` › `never passes`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

````text
Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
      at test/sub/retry.test.js:21:34
````

</details>

<details open>
<summary>Failure 7 of 17</summary>

`Bun` › `test/sub/retry.test.js` › `retries` › `fails with an empty message`

`Error` · 0.0 s

````text
Error
      at test/sub/retry.test.js:25:21
````

</details>

<details open>
<summary>Failure 8 of 17</summary>

`Bun` › `test/sub/retry.test.js` › `retries` › `throws from the source`

``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

````text
RangeError: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
      at divide (src/math.js:12:42)
      at test/sub/retry.test.js:29:10
````

</details>

<details open>
<summary>Failure 9 of 17</summary>

`Jest` › `retries` › `retries never passes`

``Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

````text
Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
    at Object.<anonymous> (/work/test/sub/retry.test.js:22:9)
    at Promise.finally.completed (/work/node_modules/jest-circus/build/jestAdapterInit.js:1834:28)
    at new Promise (<anonymous>)
    at callAsyncCircusFn (/work/node_modules/jest-circus/build/jestAdapterInit.js:1774:10)
    at _callCircusTest (/work/node_modules/jest-circus/build/jestAdapterInit.js:1167:40)
    at processTicksAndRejections (node:internal/process/task_queues:103:5)
    at _runTestInContext (/work/node_modules/jest-circus/build/jestAdapterInit.js:1103:3)
    at rerunTest (/work/node_modules/jest-circus/build/jestAdapterInit.js:977:7)
    at _runTestsForDescribeBlockOnce (/work/node_modules/jest-circus/build/jestAdapterInit.js:1033:5)
    at _runTestsForDescribeBlock (/work/node_modules/jest-circus/build/jestAdapterInit.js:824:5)
    at _runTestsForDescribeBlockOnce (/work/node_modules/jest-circus/build/jestAdapterInit.js:1003:11)
    at _runTestsForDescribeBlock (/work/node_modules/jest-circus/build/jestAdapterInit.js:824:5)
    at run (/work/node_modules/jest-circus/build/jestAdapterInit.js:789:3)
    at runAndTransformResultsToJestFormat (/work/node_modules/jest-circus/build/jestAdapterInit.js:2300:21)
    at jestAdapter (/work/node_modules/jest-circus/build/runner.js:111:19)
    at runTestInternal (/work/node_modules/jest-runner/build/testWorker.js:247:16)
    at runTest (/work/node_modules/jest-runner/build/testWorker.js:315:7)
    at Object.worker (/work/node_modules/jest-runner/build/testWorker.js:479:12)
````

</details>

<details open>
<summary>Failure 10 of 17</summary>

`Jest` › `retries` › `retries throws from the source`

``RangeError: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

````text
RangeError: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> | pipe
    at divide (/work/src/math.js:12:9)
    at Object.divide (/work/test/sub/retry.test.js:26:10)
    at Promise.finally.completed (/work/node_modules/jest-circus/build/jestAdapterInit.js:1834:28)
    at new Promise (<anonymous>)
    at callAsyncCircusFn (/work/node_modules/jest-circus/build/jestAdapterInit.js:1774:10)
    at _callCircusTest (/work/node_modules/jest-circus/build/jestAdapterInit.js:1167:40)
    at processTicksAndRejections (node:internal/process/task_queues:103:5)
    at _runTestInContext (/work/node_modules/jest-circus/build/jestAdapterInit.js:1103:3)
    at rerunTest (/work/node_modules/jest-circus/build/jestAdapterInit.js:977:7)
    at _runTestsForDescribeBlockOnce (/work/node_modules/jest-circus/build/jestAdapterInit.js:1033:5)
    at _runTestsForDescribeBlock (/work/node_modules/jest-circus/build/jestAdapterInit.js:824:5)
    at _runTestsForDescribeBlockOnce (/work/node_modules/jest-circus/build/jestAdapterInit.js:1003:11)
    at _runTestsForDescribeBlock (/work/node_modules/jest-circus/build/jestAdapterInit.js:824:5)
    at run (/work/node_modules/jest-circus/build/jestAdapterInit.js:789:3)
    at runAndTransformResultsToJestFormat (/work/node_modules/jest-circus/build/jestAdapterInit.js:2300:21)
    at jestAdapter (/work/node_modules/jest-circus/build/runner.js:111:19)
    at runTestInternal (/work/node_modules/jest-runner/build/testWorker.js:247:16)
    at runTest (/work/node_modules/jest-runner/build/testWorker.js:315:7)
    at Object.worker (/work/node_modules/jest-runner/build/testWorker.js:479:12)
````

</details>

<details open>
<summary>Failure 11 of 17</summary>

`Jest` › `math` › ``math division rounding @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe``

`Error: expect(received).toBe(expected) // Object.is equality` · 0.0 s

First 20 of 26 lines:

````text
Error: expect(received).toBe(expected) // Object.is equality

Expected: 5
Received: 4
    at Object.toBe (/work/test/math.test.js:28:23)
    at Promise.finally.completed (/work/node_modules/jest-circus/build/jestAdapterInit.js:1834:28)
    at new Promise (<anonymous>)
    at callAsyncCircusFn (/work/node_modules/jest-circus/build/jestAdapterInit.js:1774:10)
    at _callCircusTest (/work/node_modules/jest-circus/build/jestAdapterInit.js:1167:40)
    at processTicksAndRejections (node:internal/process/task_queues:103:5)
    at _runTestInContext (/work/node_modules/jest-circus/build/jestAdapterInit.js:1103:3)
    at runTestWithContext (/work/node_modules/jest-circus/build/jestAdapterInit.js:996:5)
    at _runTestsForDescribeBlockOnce (/work/node_modules/jest-circus/build/jestAdapterInit.js:1008:11)
    at _runTestsForDescribeBlock (/work/node_modules/jest-circus/build/jestAdapterInit.js:824:5)
    at _runTestsForDescribeBlockOnce (/work/node_modules/jest-circus/build/jestAdapterInit.js:1003:11)
    at _runTestsForDescribeBlock (/work/node_modules/jest-circus/build/jestAdapterInit.js:824:5)
    at _runTestsForDescribeBlockOnce (/work/node_modules/jest-circus/build/jestAdapterInit.js:1003:11)
    at _runTestsForDescribeBlock (/work/node_modules/jest-circus/build/jestAdapterInit.js:824:5)
    at _runTestsForDescribeBlockOnce (/work/node_modules/jest-circus/build/jestAdapterInit.js:1003:11)
    at _runTestsForDescribeBlock (/work/node_modules/jest-circus/build/jestAdapterInit.js:824:5)
````

</details>

<details open>
<summary>Failure 12 of 17</summary>

`Playwright` › `math.spec.js [chromium]` › ``math › division › rounding › @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe``

`expect(received).toBe(expected) // Object.is equality` · 0.0 s

First 20 of 65 lines:

````text
[chromium] › math.spec.js:24:4 › math › division › rounding › @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe ─────────────────

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: 5
    Received: 4

      24 | 			test('@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe', async ({}, testInfo) => {
      25 | 				await testInfo.attach('note', { body: 'a note for the report', contentType: 'text/plain' });
    > 26 | 				expect(add(2, 2)).toBe(5);
         | 				                  ^
      27 | 			});
      28 | 		});
      29 | 	});
        at /work/test/math.spec.js:26:23

    attachment #1: note (text/plain) ───────────────────────────────────────────────────────────────
    a note for the report
    ────────────────────────────────────────────────────────────────────────────────────────────────

````

</details>

<details open>
<summary>Failure 13 of 17</summary>

`Playwright` › `sub/retry.spec.js [chromium]` › `retries › never passes`

Errored: ``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

First 20 of 44 lines:

````text
[chromium] › sub/retry.spec.js:14:2 › retries › never passes ─────────────────────────────────────

    Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe

      13 |
      14 | 	test('never passes', () => {
    > 15 | 		throw new Error('@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe');
         | 		      ^
      16 | 	});
      17 |
      18 | 	test('throws from the source', () => {
        at /work/test/sub/retry.spec.js:15:9

    Error Context: ../test-results/sub-retry-retries-never-passes-chromium/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

    Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe

      13 |
````

</details>

<details open>
<summary>Failure 14 of 17</summary>

`Playwright` › `sub/retry.spec.js [chromium]` › `retries › throws from the source`

Errored: ``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

First 20 of 53 lines:

````text
[chromium] › sub/retry.spec.js:18:2 › retries › throws from the source ───────────────────────────

    RangeError: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe

       at ../src/math.js:12

      10 | export function divide(a, b) {
      11 | 	if (b === 0) {
    > 12 | 		throw new RangeError('@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe');
         | 		      ^
      13 | 	}
      14 | 	return a / b;
      15 | }
        at divide (/work/src/math.js:12:9)
        at /work/test/sub/retry.spec.js:19:10

    Error Context: ../test-results/sub-retry-retries-throws-from-the-source-chromium/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

````

</details>

<details open>
<summary>Failure 15 of 17</summary>

`Playwright` › `math.spec.js [firefox]` › ``math › division › rounding › @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe``

`expect(received).toBe(expected) // Object.is equality` · 0.0 s

First 20 of 65 lines:

````text
[firefox] › math.spec.js:24:4 › math › division › rounding › @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe ──────────────────

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: 5
    Received: 4

      24 | 			test('@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe', async ({}, testInfo) => {
      25 | 				await testInfo.attach('note', { body: 'a note for the report', contentType: 'text/plain' });
    > 26 | 				expect(add(2, 2)).toBe(5);
         | 				                  ^
      27 | 			});
      28 | 		});
      29 | 	});
        at /work/test/math.spec.js:26:23

    attachment #1: note (text/plain) ───────────────────────────────────────────────────────────────
    a note for the report
    ────────────────────────────────────────────────────────────────────────────────────────────────

````

</details>

<details open>
<summary>Failure 16 of 17</summary>

`Playwright` › `sub/retry.spec.js [firefox]` › `retries › never passes`

Errored: ``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

First 20 of 44 lines:

````text
[firefox] › sub/retry.spec.js:14:2 › retries › never passes ──────────────────────────────────────

    Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe

      13 |
      14 | 	test('never passes', () => {
    > 15 | 		throw new Error('@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe');
         | 		      ^
      16 | 	});
      17 |
      18 | 	test('throws from the source', () => {
        at /work/test/sub/retry.spec.js:15:9

    Error Context: ../test-results/sub-retry-retries-never-passes-firefox/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

    Error: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe

      13 |
````

</details>

<details open>
<summary>Failure 17 of 17</summary>

`Playwright` › `sub/retry.spec.js [firefox]` › `retries › throws from the source`

Errored: ``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` · 0.0 s

First 20 of 53 lines:

````text
[firefox] › sub/retry.spec.js:18:2 › retries › throws from the source ────────────────────────────

    RangeError: @everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe

       at ../src/math.js:12

      10 | export function divide(a, b) {
      11 | 	if (b === 0) {
    > 12 | 		throw new RangeError('@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) &lt;img src=x onerror=alert(1)&gt; | pipe');
         | 		      ^
      13 | 	}
      14 | 	return a / b;
      15 | }
        at divide (/work/src/math.js:12:9)
        at /work/test/sub/retry.spec.js:19:10

    Error Context: ../test-results/sub-retry-retries-throws-from-the-source-firefox/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

````

</details>

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
