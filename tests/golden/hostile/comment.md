<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### ＠everyone GH－123 &lt;img src\=x onerror\=alert\(1\)&gt; \[x\]\(https&#58;//example\.invalid\): 2 failures in 2 suites

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `@owner/team #1 \| pipe` | **Failed** | 1 | 2 | 0 | 0 | 0.1 s |
| `GH` | No results | 0 | 0 | 0 | 0 | 0.0 s |

No results were uploaded for `GH`.

#### Failures

<details open>
<summary>Failure 1 of 2</summary>

`@owner/team #1 \| pipe` › `<img src=x onerror=alert(1)>` › `[link](javascript:alert(1))` › `@everyone and @owner see #1 \| table \| breakout`

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

`@owner/team #1 \| pipe` › `<img src=x onerror=alert(1)>` › `rtl\u{202e}ltr\u{200b}zwsp\u{2066}iso` › `esc[31mred`

`x` · 0.0 s

````text
y
````

</details>

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
