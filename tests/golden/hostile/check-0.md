conclusion: failure
title: 2 failed, 1 passed

--- summary ---
| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `@owner/team #1 \| pipe` | **Failed** | 1 | 2 | 0 | 0 | 0.1 s |

| Group | Passed | Failed | Skipped | Flaky | Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `<img src=x onerror=alert(1)>` | 1 | 2 | 0 | 0 | 0.1 s |

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

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0

--- text ---
#### Failure details

##### 1. `<img src=x onerror=alert(1)>` › `[link](javascript:alert(1))` › `@everyone and @owner see #1 \| table \| breakout`

`::error file=a.rs,line=1::injected⏎::add-mask::secret` · 0.1 s

````text
</details><img src=x onerror=alert(1)>
<!-- pr-results:1 key=ci -->
```
~~~
![](https://example.invalid/x.png)
````

##### 2. `<img src=x onerror=alert(1)>` › `rtl\u{202e}ltr\u{200b}zwsp\u{2066}iso` › `esc[31mred`

`x` · 0.0 s

````text
y
````

#### Slowest tests

| Test | Time |
| --- | ---: |
| `<img src=x onerror=alert(1)>` › `[link](javascript:alert(1))` › `@everyone and @owner see #1 \| table \| breakout` | 0.1 s |
| `<img src=x onerror=alert(1)>` › `rtl\u{202e}ltr\u{200b}zwsp\u{2066}iso` › `esc[31mred` | 0.0 s |
| `<img src=x onerror=alert(1)>` › `c` › `plain` | 0.0 s |
