conclusion: failure
title: 3 failed, 7 passed, 1 skipped

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Jest` | **Failed** | 7 | 3 | 1 | 0 | 0.2 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `retries` | 1 | 2 | 0 | 0 | 0.1 s |
| `math` | 6 | 1 | 1 | 0 | 0.1 s |

#### Failures

<details open>
<summary>Failure 1 of 3</summary>

`retries` › `retries never passes`

`Error: always broken` · 0.0 s

````text
Error: always broken
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
<summary>Failure 2 of 3</summary>

`retries` › `retries throws from the source`

`RangeError: division by zero` · 0.0 s

````text
RangeError: division by zero
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
<summary>Failure 3 of 3</summary>

`math` › `math division rounding fails with a stack`

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

Annotations: 3.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `retries` › `retries never passes`

`Error: always broken` · 0.0 s

````text
Error: always broken
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

##### 2. `retries` › `retries throws from the source`

`RangeError: division by zero` · 0.0 s

````text
RangeError: division by zero
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

##### 3. `math` › `math division rounding fails with a stack`

`Error: expect(received).toBe(expected) // Object.is equality` · 0.0 s

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
    at run (/work/node_modules/jest-circus/build/jestAdapterInit.js:789:3)
    at runAndTransformResultsToJestFormat (/work/node_modules/jest-circus/build/jestAdapterInit.js:2300:21)
    at jestAdapter (/work/node_modules/jest-circus/build/runner.js:111:19)
    at runTestInternal (/work/node_modules/jest-runner/build/testWorker.js:247:16)
    at runTest (/work/node_modules/jest-runner/build/testWorker.js:315:7)
    at Object.worker (/work/node_modules/jest-runner/build/testWorker.js:479:12)
````

#### Slowest tests

| Test | Time |
| --- | ---: |
| `math` › `math adds` | 0.0 s |
| `math` › `math division rounding fails with a stack` | 0.0 s |
| `math` › `math division divides` | 0.0 s |
| `math` › `math division rejects zero` | 0.0 s |
| `retries` › `retries passes on the second try` | 0.0 s |
| `retries` › `retries never passes` | 0.0 s |
| `retries` › `retries throws from the source` | 0.0 s |
| `math` › `math division rounding rounds down` | 0.0 s |
| `math` › `math is still to do` | 0.0 s |
| `math` › `works outside any describe` | 0.0 s |
