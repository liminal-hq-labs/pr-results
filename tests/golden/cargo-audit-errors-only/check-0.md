conclusion: failure
title: 2 failed, 2 skipped

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Audit` | ✗ **Failed** | 0 | 2 | 2 | 0 | 0.0 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `cargo audit` | 0 | 2 | 2 | 0 | 0.0 s |

#### Failures

<details open>
<summary>Failure 1 of 2</summary>

`cargo audit` › `time 0.1.45` › `RUSTSEC-2020-0071`

`Potential segfault in the time crate` · 0.0 s

````text
Unix-like operating systems may segfault due to dereferencing a dangling pointer.
The issue is in the use of localtime_r.
Patched versions: >=0.2.23
https://github.com/time-rs/time/issues/293
````

</details>

<details open>
<summary>Failure 2 of 2</summary>

`cargo audit` › `regex 1.5.4` › `RUSTSEC-2022-0013`

`Regexes with large repetitions on empty sub-expressions take a very long time to parse` · 0.0 s

````text
Denial of service.
Patched versions: >=1.5.5
https://github.com/rust-lang/regex/security/advisories/GHSA-m5pq-gvj9-9vr8
````

</details>

Annotations: 2.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `cargo audit` › `time 0.1.45` › `RUSTSEC-2020-0071`

`Potential segfault in the time crate` · 0.0 s

````text
Unix-like operating systems may segfault due to dereferencing a dangling pointer.
The issue is in the use of localtime_r.
Patched versions: >=0.2.23
https://github.com/time-rs/time/issues/293
````

##### 2. `cargo audit` › `regex 1.5.4` › `RUSTSEC-2022-0013`

`Regexes with large repetitions on empty sub-expressions take a very long time to parse` · 0.0 s

````text
Denial of service.
Patched versions: >=1.5.5
https://github.com/rust-lang/regex/security/advisories/GHSA-m5pq-gvj9-9vr8
````

#### Slowest tests

| Test | Time |
| --- | ---: |
| `cargo audit` › `time 0.1.45` › `RUSTSEC-2020-0071` | 0.0 s |
| `cargo audit` › `regex 1.5.4` › `RUSTSEC-2022-0013` | 0.0 s |
