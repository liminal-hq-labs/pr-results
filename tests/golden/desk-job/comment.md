<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### Desk checks: 1 failure in 1 suite

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Desk steps` | **Failed** | 3 | 1 | 0 | 0 | 21.7 s |

#### Failures

<details open>
<summary>Failure 1 of 1</summary>

`check (host)` › `Host tests` › `shell crates, host tests`

`exit status 101` · 6.3 s

Last 20 of 31 lines:

````text
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [12/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [13/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [14/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [15/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [16/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [17/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [18/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [19/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [20/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [21/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [22/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [23/24]
   Compiling desk-shell v0.1.0 (shell/crates/desk-shell) [24/24]
error[E0308]: mismatched types
  --> shell/desk-gfx/src/fill.rs:33:9
   |
33 |         pixel == 0x0f
   |         ^^^^^^^^^^^^^ expected `u8`, found `bool`

error: could not compile `desk-gfx` (lib test) due to 1 previous error
````

</details>

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
