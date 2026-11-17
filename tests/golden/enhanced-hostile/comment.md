<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### ❌ ＠everyone GH－123 &lt;img src\=x onerror\=alert\(1\)&gt; \[x\]\(https&#58;//example\.invalid\): 2 failures in 1 suite, 2 other checks need attention

> [!CAUTION]
> 🦦 **Jax:** Something broke. The details are below.
>
> 2 failed tests and 2 other checks need attention. 1 test passed.

- ❌ 2 tests failed in `@owner/team #1 \| pipe`.
- ❌ ``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` failed.
- ⚠️ `</details> > [!CAUTION]` needs action. To run it locally: `cargo test -- --nocapture`

1 of 3 tests passed ▰▰▰▱▱▱▱▱▱▱

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `@owner/team #1 \| pipe` | ❌ **Failed** | 1 | 2 | 0 | 0 | 0.1 s |

#### Other checks

| Check | Result |
| --- | --- |
| ``@everyone @owner/team GH-123 #1 `x` [y](https://example.invalid) <img src=x onerror=alert(1)> \| pipe`` | ❌ **Failed** |
| `</details> > [!CAUTION]` | ⚠️ **Action required** |
| `line one⏎line two` | ✅ Passed |

#### Failures

<details open>
<summary>Failure 1 of 2</summary>

`<img src=x onerror=alert(1)>` › `[link](javascript:alert(1))` › `@everyone and @owner see #1 \| table \| breakout`

`::error file=a.rs,line=1::injected⏎::add-mask::secret` · 0.1 s

````text
</details><img src=x onerror=alert(1)>
<!-- pr-results:1 key=ci -->
```
~~~
![](https://example.invalid/x.png)
````

</details>

<details open>
<summary>Failure 2 of 2</summary>

`<img src=x onerror=alert(1)>` › `rtl\u{202e}ltr\u{200b}zwsp\u{2066}iso` › `esc[31mred`

`x` · 0.0 s

````text
y
````

</details>

<details>
<summary>Slowest tests (1)</summary>

| Test | Time |
| --- | ---: |
| `<img src=x onerror=alert(1)>` › `[link](javascript:alert(1))` › `@everyone and @owner see #1 \| table \| breakout` | 0.1 s |

</details>

♻️ This comment is updated in place with the latest results.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
