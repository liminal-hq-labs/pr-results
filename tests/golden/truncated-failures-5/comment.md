<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### Example checks: 24 failures in 6 suites

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Rust tests` | **Failed** | 20 | 24 | 0 | 8 | 18.9 s |
| `Alpha checks` | Passed | 6 | 0 | 0 | 0 | 1.4 s |
| `Bravo checks` | Passed | 6 | 0 | 0 | 0 | 1.4 s |
| `Charlie checks` | Passed | 6 | 0 | 0 | 0 | 1.4 s |
| `Delta checks` | Passed | 6 | 0 | 0 | 0 | 1.4 s |
| `Echo checks` | Passed | 6 | 0 | 0 | 0 | 1.4 s |

#### Failures

<details open>
<summary>Failure 1 of 24</summary>

`Rust tests` › `core` › `module_0::fails_0`

`assertion failed: value 0 is out of range` · 0.0 s

First 3 of 13 lines:

````text
thread 'module_0::fails_0' panicked at crates/core/src/m0.rs:10:5:
line 0: expected 0 to equal 1 in module_0 while checking item 0
line 1: expected 0 to equal 1 in module_0 while checking item 1
````

</details>

<details open>
<summary>Failure 2 of 24</summary>

`Rust tests` › `core` › `module_1::fails_1`

`assertion failed: value 1 is out of range` · 0.0 s

First 3 of 13 lines:

````text
thread 'module_1::fails_1' panicked at crates/core/src/m1.rs:11:5:
line 0: expected 1 to equal 2 in module_1 while checking item 0
line 1: expected 1 to equal 2 in module_1 while checking item 1
````

</details>

<details open>
<summary>Failure 3 of 24</summary>

`Rust tests` › `core` › `module_2::fails_2`

`assertion failed: value 2 is out of range` · 0.1 s

First 3 of 13 lines:

````text
thread 'module_2::fails_2' panicked at crates/core/src/m2.rs:12:5:
line 0: expected 2 to equal 3 in module_2 while checking item 0
line 1: expected 2 to equal 3 in module_2 while checking item 1
````

</details>

<details open>
<summary>Failure 4 of 24</summary>

`Rust tests` › `core` › `module_3::fails_3`

`assertion failed: value 3 is out of range` · 0.1 s

First 3 of 13 lines:

````text
thread 'module_3::fails_3' panicked at crates/core/src/m3.rs:13:5:
line 0: expected 3 to equal 4 in module_3 while checking item 0
line 1: expected 3 to equal 4 in module_3 while checking item 1
````

</details>

<details open>
<summary>Failure 5 of 24</summary>

`Rust tests` › `core` › `module_4::fails_4`

`assertion failed: value 4 is out of range` · 0.1 s

First 3 of 13 lines:

````text
thread 'module_4::fails_4' panicked at crates/core/src/m4.rs:14:5:
line 0: expected 4 to equal 5 in module_4 while checking item 0
line 1: expected 4 to equal 5 in module_4 while checking item 1
````

</details>

…and 19 more failures in the check runs.

The list of 8 flaky tests was left out to fit GitHub's size limit. The table counts them.

Failure excerpts were shortened to 3 lines each to fit GitHub's size limit.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
