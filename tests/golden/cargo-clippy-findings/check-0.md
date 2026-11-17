conclusion: failure
title: 4 failed

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Clippy` | ✗ **Failed** | 0 | 4 | 0 | 0 | 0.0 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `clippy` | 0 | 4 | 0 | 0 | 0.0 s |

#### Failures

<details open>
<summary>Failure 1 of 4</summary>

`clippy` › `crates/pr-results-forge/src/publish.rs` › `clippy::too_many_arguments`

`this function has too many arguments (8/7)` · 0.0 s

````text
error: this function has too many arguments (8/7)
   --> crates/pr-results-forge/src/publish.rs:454:1
    |
454 | fn finish<H: Host>(
    | ^^^^^^^^^^^^^^^^^^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#too_many_arguments
````

</details>

<details open>
<summary>Failure 2 of 4</summary>

`clippy` › `src/lib.rs` › `clippy::needless_return`

``unneeded `return` statement`` · 0.0 s

````text
warning: unneeded `return` statement
  --> src/lib.rs:12:5
````

</details>

<details open>
<summary>Failure 3 of 4</summary>

`clippy` › `unused_imports`

`` unused import: `x` `` · 0.0 s

````text
warning: unused import: `x`
 --> /home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/dep-1.0.0/src/lib.rs:3:5
````

</details>

<details open>
<summary>Failure 4 of 4</summary>

`clippy` › `src/main.rs` › `E0308`

`mismatched types` · 0.0 s

````text
error[E0308]: mismatched types
 --> src/main.rs:8:9
````

</details>

Annotations: 3.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `clippy` › `crates/pr-results-forge/src/publish.rs` › `clippy::too_many_arguments`

`this function has too many arguments (8/7)` · 0.0 s

````text
error: this function has too many arguments (8/7)
   --> crates/pr-results-forge/src/publish.rs:454:1
    |
454 | fn finish<H: Host>(
    | ^^^^^^^^^^^^^^^^^^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#too_many_arguments
````

##### 2. `clippy` › `src/lib.rs` › `clippy::needless_return`

``unneeded `return` statement`` · 0.0 s

````text
warning: unneeded `return` statement
  --> src/lib.rs:12:5
````

##### 3. `clippy` › `unused_imports`

`` unused import: `x` `` · 0.0 s

````text
warning: unused import: `x`
 --> /home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/dep-1.0.0/src/lib.rs:3:5
````

##### 4. `clippy` › `src/main.rs` › `E0308`

`mismatched types` · 0.0 s

````text
error[E0308]: mismatched types
 --> src/main.rs:8:9
````

#### Slowest tests

| Test | Time |
| --- | ---: |
| `clippy` › `crates/pr-results-forge/src/publish.rs` › `clippy::too_many_arguments` | 0.0 s |
| `clippy` › `src/lib.rs` › `clippy::needless_return` | 0.0 s |
| `clippy` › `unused_imports` | 0.0 s |
| `clippy` › `src/main.rs` › `E0308` | 0.0 s |
