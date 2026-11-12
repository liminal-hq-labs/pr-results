conclusion: failure
title: 6 failed, 10 passed, 4 skipped

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Playwright` | **Failed** | 10 | 6 | 4 | 0 | 0.0 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `math.spec.js [chromium]` | 4 | 1 | 2 | 0 | 0.0 s |
| `sub/retry.spec.js [chromium]` | 1 | 2 | 0 | 0 | 0.0 s |
| `math.spec.js [firefox]` | 4 | 1 | 2 | 0 | 0.0 s |
| `sub/retry.spec.js [firefox]` | 1 | 2 | 0 | 0 | 0.0 s |

#### Failures

<details open>
<summary>Failure 1 of 6</summary>

`math.spec.js [chromium]` › `math › division › rounding › fails with a stack`

`expect(received).toBe(expected) // Object.is equality` · 0.0 s

First 20 of 65 lines:

````text
[chromium] › math.spec.js:24:4 › math › division › rounding › fails with a stack ─────────────────

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: 5
    Received: 4

      24 | 			test('fails with a stack', async ({}, testInfo) => {
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
<summary>Failure 2 of 6</summary>

`sub/retry.spec.js [chromium]` › `retries › never passes`

Errored: `always broken` · 0.0 s

First 20 of 44 lines:

````text
[chromium] › sub/retry.spec.js:14:2 › retries › never passes ─────────────────────────────────────

    Error: always broken

      13 |
      14 | 	test('never passes', () => {
    > 15 | 		throw new Error('always broken');
         | 		      ^
      16 | 	});
      17 |
      18 | 	test('throws from the source', () => {
        at /work/test/sub/retry.spec.js:15:9

    Error Context: ../test-results/sub-retry-retries-never-passes-chromium/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

    Error: always broken

      13 |
````

</details>

<details open>
<summary>Failure 3 of 6</summary>

`sub/retry.spec.js [chromium]` › `retries › throws from the source`

Errored: `division by zero` · 0.0 s

First 20 of 53 lines:

````text
[chromium] › sub/retry.spec.js:18:2 › retries › throws from the source ───────────────────────────

    RangeError: division by zero

       at ../src/math.js:12

      10 | export function divide(a, b) {
      11 | 	if (b === 0) {
    > 12 | 		throw new RangeError('division by zero');
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
<summary>Failure 4 of 6</summary>

`math.spec.js [firefox]` › `math › division › rounding › fails with a stack`

`expect(received).toBe(expected) // Object.is equality` · 0.0 s

First 20 of 65 lines:

````text
[firefox] › math.spec.js:24:4 › math › division › rounding › fails with a stack ──────────────────

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: 5
    Received: 4

      24 | 			test('fails with a stack', async ({}, testInfo) => {
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
<summary>Failure 5 of 6</summary>

`sub/retry.spec.js [firefox]` › `retries › never passes`

Errored: `always broken` · 0.0 s

First 20 of 44 lines:

````text
[firefox] › sub/retry.spec.js:14:2 › retries › never passes ──────────────────────────────────────

    Error: always broken

      13 |
      14 | 	test('never passes', () => {
    > 15 | 		throw new Error('always broken');
         | 		      ^
      16 | 	});
      17 |
      18 | 	test('throws from the source', () => {
        at /work/test/sub/retry.spec.js:15:9

    Error Context: ../test-results/sub-retry-retries-never-passes-firefox/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

    Error: always broken

      13 |
````

</details>

<details open>
<summary>Failure 6 of 6</summary>

`sub/retry.spec.js [firefox]` › `retries › throws from the source`

Errored: `division by zero` · 0.0 s

First 20 of 53 lines:

````text
[firefox] › sub/retry.spec.js:18:2 › retries › throws from the source ────────────────────────────

    RangeError: division by zero

       at ../src/math.js:12

      10 | export function divide(a, b) {
      11 | 	if (b === 0) {
    > 12 | 		throw new RangeError('division by zero');
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

Annotations: 6.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `math.spec.js [chromium]` › `math › division › rounding › fails with a stack`

`expect(received).toBe(expected) // Object.is equality` · 0.0 s

````text
[chromium] › math.spec.js:24:4 › math › division › rounding › fails with a stack ─────────────────

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: 5
    Received: 4

      24 | 			test('fails with a stack', async ({}, testInfo) => {
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

    Error Context: ../test-results/math-math-division-rounding-fails-with-a-stack-chromium/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: 5
    Received: 4

      24 | 			test('fails with a stack', async ({}, testInfo) => {
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

    Error Context: ../test-results/math-math-division-rounding-fails-with-a-stack-chromium-retry1/error-context.md

    Retry #2 ───────────────────────────────────────────────────────────────────────────────────────

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: 5
    Received: 4

      24 | 			test('fails with a stack', async ({}, testInfo) => {
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

    Error Context: ../test-results/math-math-division-rounding-fails-with-a-stack-chromium-retry2/error-context.md
````

##### 2. `sub/retry.spec.js [chromium]` › `retries › never passes`

Errored: `always broken` · 0.0 s

````text
[chromium] › sub/retry.spec.js:14:2 › retries › never passes ─────────────────────────────────────

    Error: always broken

      13 |
      14 | 	test('never passes', () => {
    > 15 | 		throw new Error('always broken');
         | 		      ^
      16 | 	});
      17 |
      18 | 	test('throws from the source', () => {
        at /work/test/sub/retry.spec.js:15:9

    Error Context: ../test-results/sub-retry-retries-never-passes-chromium/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

    Error: always broken

      13 |
      14 | 	test('never passes', () => {
    > 15 | 		throw new Error('always broken');
         | 		      ^
      16 | 	});
      17 |
      18 | 	test('throws from the source', () => {
        at /work/test/sub/retry.spec.js:15:9

    Error Context: ../test-results/sub-retry-retries-never-passes-chromium-retry1/error-context.md

    Retry #2 ───────────────────────────────────────────────────────────────────────────────────────

    Error: always broken

      13 |
      14 | 	test('never passes', () => {
    > 15 | 		throw new Error('always broken');
         | 		      ^
      16 | 	});
      17 |
      18 | 	test('throws from the source', () => {
        at /work/test/sub/retry.spec.js:15:9

    Error Context: ../test-results/sub-retry-retries-never-passes-chromium-retry2/error-context.md
````

##### 3. `sub/retry.spec.js [chromium]` › `retries › throws from the source`

Errored: `division by zero` · 0.0 s

````text
[chromium] › sub/retry.spec.js:18:2 › retries › throws from the source ───────────────────────────

    RangeError: division by zero

       at ../src/math.js:12

      10 | export function divide(a, b) {
      11 | 	if (b === 0) {
    > 12 | 		throw new RangeError('division by zero');
         | 		      ^
      13 | 	}
      14 | 	return a / b;
      15 | }
        at divide (/work/src/math.js:12:9)
        at /work/test/sub/retry.spec.js:19:10

    Error Context: ../test-results/sub-retry-retries-throws-from-the-source-chromium/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

    RangeError: division by zero

       at ../src/math.js:12

      10 | export function divide(a, b) {
      11 | 	if (b === 0) {
    > 12 | 		throw new RangeError('division by zero');
         | 		      ^
      13 | 	}
      14 | 	return a / b;
      15 | }
        at divide (/work/src/math.js:12:9)
        at /work/test/sub/retry.spec.js:19:10

    Error Context: ../test-results/sub-retry-retries-throws-from-the-source-chromium-retry1/error-context.md

    Retry #2 ───────────────────────────────────────────────────────────────────────────────────────

    RangeError: division by zero

       at ../src/math.js:12

      10 | export function divide(a, b) {
      11 | 	if (b === 0) {
    > 12 | 		throw new RangeError('division by zero');
         | 		      ^
      13 | 	}
      14 | 	return a / b;
      15 | }
        at divide (/work/src/math.js:12:9)
        at /work/test/sub/retry.spec.js:19:10

    Error Context: ../test-results/sub-retry-retries-throws-from-the-source-chromium-retry2/error-context.md
````

##### 4. `math.spec.js [firefox]` › `math › division › rounding › fails with a stack`

`expect(received).toBe(expected) // Object.is equality` · 0.0 s

````text
[firefox] › math.spec.js:24:4 › math › division › rounding › fails with a stack ──────────────────

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: 5
    Received: 4

      24 | 			test('fails with a stack', async ({}, testInfo) => {
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

    Error Context: ../test-results/math-math-division-rounding-fails-with-a-stack-firefox/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: 5
    Received: 4

      24 | 			test('fails with a stack', async ({}, testInfo) => {
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

    Error Context: ../test-results/math-math-division-rounding-fails-with-a-stack-firefox-retry1/error-context.md

    Retry #2 ───────────────────────────────────────────────────────────────────────────────────────

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: 5
    Received: 4

      24 | 			test('fails with a stack', async ({}, testInfo) => {
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

    Error Context: ../test-results/math-math-division-rounding-fails-with-a-stack-firefox-retry2/error-context.md
````

##### 5. `sub/retry.spec.js [firefox]` › `retries › never passes`

Errored: `always broken` · 0.0 s

````text
[firefox] › sub/retry.spec.js:14:2 › retries › never passes ──────────────────────────────────────

    Error: always broken

      13 |
      14 | 	test('never passes', () => {
    > 15 | 		throw new Error('always broken');
         | 		      ^
      16 | 	});
      17 |
      18 | 	test('throws from the source', () => {
        at /work/test/sub/retry.spec.js:15:9

    Error Context: ../test-results/sub-retry-retries-never-passes-firefox/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

    Error: always broken

      13 |
      14 | 	test('never passes', () => {
    > 15 | 		throw new Error('always broken');
         | 		      ^
      16 | 	});
      17 |
      18 | 	test('throws from the source', () => {
        at /work/test/sub/retry.spec.js:15:9

    Error Context: ../test-results/sub-retry-retries-never-passes-firefox-retry1/error-context.md

    Retry #2 ───────────────────────────────────────────────────────────────────────────────────────

    Error: always broken

      13 |
      14 | 	test('never passes', () => {
    > 15 | 		throw new Error('always broken');
         | 		      ^
      16 | 	});
      17 |
      18 | 	test('throws from the source', () => {
        at /work/test/sub/retry.spec.js:15:9

    Error Context: ../test-results/sub-retry-retries-never-passes-firefox-retry2/error-context.md
````

##### 6. `sub/retry.spec.js [firefox]` › `retries › throws from the source`

Errored: `division by zero` · 0.0 s

````text
[firefox] › sub/retry.spec.js:18:2 › retries › throws from the source ────────────────────────────

    RangeError: division by zero

       at ../src/math.js:12

      10 | export function divide(a, b) {
      11 | 	if (b === 0) {
    > 12 | 		throw new RangeError('division by zero');
         | 		      ^
      13 | 	}
      14 | 	return a / b;
      15 | }
        at divide (/work/src/math.js:12:9)
        at /work/test/sub/retry.spec.js:19:10

    Error Context: ../test-results/sub-retry-retries-throws-from-the-source-firefox/error-context.md

    Retry #1 ───────────────────────────────────────────────────────────────────────────────────────

    RangeError: division by zero

       at ../src/math.js:12

      10 | export function divide(a, b) {
      11 | 	if (b === 0) {
    > 12 | 		throw new RangeError('division by zero');
         | 		      ^
      13 | 	}
      14 | 	return a / b;
      15 | }
        at divide (/work/src/math.js:12:9)
        at /work/test/sub/retry.spec.js:19:10

    Error Context: ../test-results/sub-retry-retries-throws-from-the-source-firefox-retry1/error-context.md

    Retry #2 ───────────────────────────────────────────────────────────────────────────────────────

    RangeError: division by zero

       at ../src/math.js:12

      10 | export function divide(a, b) {
      11 | 	if (b === 0) {
    > 12 | 		throw new RangeError('division by zero');
         | 		      ^
      13 | 	}
      14 | 	return a / b;
      15 | }
        at divide (/work/src/math.js:12:9)
        at /work/test/sub/retry.spec.js:19:10

    Error Context: ../test-results/sub-retry-retries-throws-from-the-source-firefox-retry2/error-context.md
````

#### Slowest tests

| Test | Time |
| --- | ---: |
| `math.spec.js [firefox]` › `math › division › rounding › fails with a stack` | 0.0 s |
| `math.spec.js [chromium]` › `math › division › rounding › fails with a stack` | 0.0 s |
| `sub/retry.spec.js [firefox]` › `retries › never passes` | 0.0 s |
| `sub/retry.spec.js [chromium]` › `retries › passes on the second try` | 0.0 s |
| `sub/retry.spec.js [chromium]` › `retries › throws from the source` | 0.0 s |
| `sub/retry.spec.js [firefox]` › `retries › passes on the second try` | 0.0 s |
| `sub/retry.spec.js [firefox]` › `retries › throws from the source` | 0.0 s |
| `math.spec.js [chromium]` › `math › adds` | 0.0 s |
| `math.spec.js [chromium]` › `works outside any describe` | 0.0 s |
| `sub/retry.spec.js [chromium]` › `retries › never passes` | 0.0 s |
