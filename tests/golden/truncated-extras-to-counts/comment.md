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

````text
thread 'module_0::fails_0' panicked at crates/core/src/m0.rs:10:5:
line 0: expected 0 to equal 1 in module_0 while checking item 0
line 1: expected 0 to equal 1 in module_0 while checking item 1
line 2: expected 0 to equal 1 in module_0 while checking item 2
line 3: expected 0 to equal 1 in module_0 while checking item 3
line 4: expected 0 to equal 1 in module_0 while checking item 4
line 5: expected 0 to equal 1 in module_0 while checking item 5
line 6: expected 0 to equal 1 in module_0 while checking item 6
line 7: expected 0 to equal 1 in module_0 while checking item 7
line 8: expected 0 to equal 1 in module_0 while checking item 8
line 9: expected 0 to equal 1 in module_0 while checking item 9
line 10: expected 0 to equal 1 in module_0 while checking item 10
line 11: expected 0 to equal 1 in module_0 while checking item 11
````

</details>

<details open>
<summary>Failure 2 of 24</summary>

`Rust tests` › `core` › `module_1::fails_1`

`assertion failed: value 1 is out of range` · 0.0 s

````text
thread 'module_1::fails_1' panicked at crates/core/src/m1.rs:11:5:
line 0: expected 1 to equal 2 in module_1 while checking item 0
line 1: expected 1 to equal 2 in module_1 while checking item 1
line 2: expected 1 to equal 2 in module_1 while checking item 2
line 3: expected 1 to equal 2 in module_1 while checking item 3
line 4: expected 1 to equal 2 in module_1 while checking item 4
line 5: expected 1 to equal 2 in module_1 while checking item 5
line 6: expected 1 to equal 2 in module_1 while checking item 6
line 7: expected 1 to equal 2 in module_1 while checking item 7
line 8: expected 1 to equal 2 in module_1 while checking item 8
line 9: expected 1 to equal 2 in module_1 while checking item 9
line 10: expected 1 to equal 2 in module_1 while checking item 10
line 11: expected 1 to equal 2 in module_1 while checking item 11
````

</details>

<details open>
<summary>Failure 3 of 24</summary>

`Rust tests` › `core` › `module_2::fails_2`

`assertion failed: value 2 is out of range` · 0.1 s

````text
thread 'module_2::fails_2' panicked at crates/core/src/m2.rs:12:5:
line 0: expected 2 to equal 3 in module_2 while checking item 0
line 1: expected 2 to equal 3 in module_2 while checking item 1
line 2: expected 2 to equal 3 in module_2 while checking item 2
line 3: expected 2 to equal 3 in module_2 while checking item 3
line 4: expected 2 to equal 3 in module_2 while checking item 4
line 5: expected 2 to equal 3 in module_2 while checking item 5
line 6: expected 2 to equal 3 in module_2 while checking item 6
line 7: expected 2 to equal 3 in module_2 while checking item 7
line 8: expected 2 to equal 3 in module_2 while checking item 8
line 9: expected 2 to equal 3 in module_2 while checking item 9
line 10: expected 2 to equal 3 in module_2 while checking item 10
line 11: expected 2 to equal 3 in module_2 while checking item 11
````

</details>

<details open>
<summary>Failure 4 of 24</summary>

`Rust tests` › `core` › `module_3::fails_3`

`assertion failed: value 3 is out of range` · 0.1 s

````text
thread 'module_3::fails_3' panicked at crates/core/src/m3.rs:13:5:
line 0: expected 3 to equal 4 in module_3 while checking item 0
line 1: expected 3 to equal 4 in module_3 while checking item 1
line 2: expected 3 to equal 4 in module_3 while checking item 2
line 3: expected 3 to equal 4 in module_3 while checking item 3
line 4: expected 3 to equal 4 in module_3 while checking item 4
line 5: expected 3 to equal 4 in module_3 while checking item 5
line 6: expected 3 to equal 4 in module_3 while checking item 6
line 7: expected 3 to equal 4 in module_3 while checking item 7
line 8: expected 3 to equal 4 in module_3 while checking item 8
line 9: expected 3 to equal 4 in module_3 while checking item 9
line 10: expected 3 to equal 4 in module_3 while checking item 10
line 11: expected 3 to equal 4 in module_3 while checking item 11
````

</details>

<details open>
<summary>Failure 5 of 24</summary>

`Rust tests` › `core` › `module_4::fails_4`

`assertion failed: value 4 is out of range` · 0.1 s

````text
thread 'module_4::fails_4' panicked at crates/core/src/m4.rs:14:5:
line 0: expected 4 to equal 5 in module_4 while checking item 0
line 1: expected 4 to equal 5 in module_4 while checking item 1
line 2: expected 4 to equal 5 in module_4 while checking item 2
line 3: expected 4 to equal 5 in module_4 while checking item 3
line 4: expected 4 to equal 5 in module_4 while checking item 4
line 5: expected 4 to equal 5 in module_4 while checking item 5
line 6: expected 4 to equal 5 in module_4 while checking item 6
line 7: expected 4 to equal 5 in module_4 while checking item 7
line 8: expected 4 to equal 5 in module_4 while checking item 8
line 9: expected 4 to equal 5 in module_4 while checking item 9
line 10: expected 4 to equal 5 in module_4 while checking item 10
line 11: expected 4 to equal 5 in module_4 while checking item 11
````

</details>

<details open>
<summary>Failure 6 of 24</summary>

`Rust tests` › `core` › `module_5::fails_5`

`assertion failed: value 5 is out of range` · 0.2 s

````text
thread 'module_5::fails_5' panicked at crates/core/src/m5.rs:15:5:
line 0: expected 5 to equal 6 in module_5 while checking item 0
line 1: expected 5 to equal 6 in module_5 while checking item 1
line 2: expected 5 to equal 6 in module_5 while checking item 2
line 3: expected 5 to equal 6 in module_5 while checking item 3
line 4: expected 5 to equal 6 in module_5 while checking item 4
line 5: expected 5 to equal 6 in module_5 while checking item 5
line 6: expected 5 to equal 6 in module_5 while checking item 6
line 7: expected 5 to equal 6 in module_5 while checking item 7
line 8: expected 5 to equal 6 in module_5 while checking item 8
line 9: expected 5 to equal 6 in module_5 while checking item 9
line 10: expected 5 to equal 6 in module_5 while checking item 10
line 11: expected 5 to equal 6 in module_5 while checking item 11
````

</details>

<details open>
<summary>Failure 7 of 24</summary>

`Rust tests` › `core` › `module_6::fails_6`

`assertion failed: value 6 is out of range` · 0.2 s

````text
thread 'module_6::fails_6' panicked at crates/core/src/m6.rs:16:5:
line 0: expected 6 to equal 7 in module_6 while checking item 0
line 1: expected 6 to equal 7 in module_6 while checking item 1
line 2: expected 6 to equal 7 in module_6 while checking item 2
line 3: expected 6 to equal 7 in module_6 while checking item 3
line 4: expected 6 to equal 7 in module_6 while checking item 4
line 5: expected 6 to equal 7 in module_6 while checking item 5
line 6: expected 6 to equal 7 in module_6 while checking item 6
line 7: expected 6 to equal 7 in module_6 while checking item 7
line 8: expected 6 to equal 7 in module_6 while checking item 8
line 9: expected 6 to equal 7 in module_6 while checking item 9
line 10: expected 6 to equal 7 in module_6 while checking item 10
line 11: expected 6 to equal 7 in module_6 while checking item 11
````

</details>

<details open>
<summary>Failure 8 of 24</summary>

`Rust tests` › `core` › `module_7::fails_7`

`assertion failed: value 7 is out of range` · 0.3 s

````text
thread 'module_7::fails_7' panicked at crates/core/src/m7.rs:17:5:
line 0: expected 7 to equal 8 in module_7 while checking item 0
line 1: expected 7 to equal 8 in module_7 while checking item 1
line 2: expected 7 to equal 8 in module_7 while checking item 2
line 3: expected 7 to equal 8 in module_7 while checking item 3
line 4: expected 7 to equal 8 in module_7 while checking item 4
line 5: expected 7 to equal 8 in module_7 while checking item 5
line 6: expected 7 to equal 8 in module_7 while checking item 6
line 7: expected 7 to equal 8 in module_7 while checking item 7
line 8: expected 7 to equal 8 in module_7 while checking item 8
line 9: expected 7 to equal 8 in module_7 while checking item 9
line 10: expected 7 to equal 8 in module_7 while checking item 10
line 11: expected 7 to equal 8 in module_7 while checking item 11
````

</details>

<details open>
<summary>Failure 9 of 24</summary>

`Rust tests` › `core` › `module_8::fails_8`

`assertion failed: value 8 is out of range` · 0.3 s

````text
thread 'module_8::fails_8' panicked at crates/core/src/m8.rs:18:5:
line 0: expected 8 to equal 9 in module_8 while checking item 0
line 1: expected 8 to equal 9 in module_8 while checking item 1
line 2: expected 8 to equal 9 in module_8 while checking item 2
line 3: expected 8 to equal 9 in module_8 while checking item 3
line 4: expected 8 to equal 9 in module_8 while checking item 4
line 5: expected 8 to equal 9 in module_8 while checking item 5
line 6: expected 8 to equal 9 in module_8 while checking item 6
line 7: expected 8 to equal 9 in module_8 while checking item 7
line 8: expected 8 to equal 9 in module_8 while checking item 8
line 9: expected 8 to equal 9 in module_8 while checking item 9
line 10: expected 8 to equal 9 in module_8 while checking item 10
line 11: expected 8 to equal 9 in module_8 while checking item 11
````

</details>

<details open>
<summary>Failure 10 of 24</summary>

`Rust tests` › `core` › `module_9::fails_9`

`assertion failed: value 9 is out of range` · 0.3 s

````text
thread 'module_9::fails_9' panicked at crates/core/src/m9.rs:19:5:
line 0: expected 9 to equal 10 in module_9 while checking item 0
line 1: expected 9 to equal 10 in module_9 while checking item 1
line 2: expected 9 to equal 10 in module_9 while checking item 2
line 3: expected 9 to equal 10 in module_9 while checking item 3
line 4: expected 9 to equal 10 in module_9 while checking item 4
line 5: expected 9 to equal 10 in module_9 while checking item 5
line 6: expected 9 to equal 10 in module_9 while checking item 6
line 7: expected 9 to equal 10 in module_9 while checking item 7
line 8: expected 9 to equal 10 in module_9 while checking item 8
line 9: expected 9 to equal 10 in module_9 while checking item 9
line 10: expected 9 to equal 10 in module_9 while checking item 10
line 11: expected 9 to equal 10 in module_9 while checking item 11
````

</details>

<details open>
<summary>Failure 11 of 24</summary>

`Rust tests` › `core` › `module_10::fails_10`

`assertion failed: value 10 is out of range` · 0.4 s

````text
thread 'module_10::fails_10' panicked at crates/core/src/m10.rs:20:5:
line 0: expected 10 to equal 11 in module_10 while checking item 0
line 1: expected 10 to equal 11 in module_10 while checking item 1
line 2: expected 10 to equal 11 in module_10 while checking item 2
line 3: expected 10 to equal 11 in module_10 while checking item 3
line 4: expected 10 to equal 11 in module_10 while checking item 4
line 5: expected 10 to equal 11 in module_10 while checking item 5
line 6: expected 10 to equal 11 in module_10 while checking item 6
line 7: expected 10 to equal 11 in module_10 while checking item 7
line 8: expected 10 to equal 11 in module_10 while checking item 8
line 9: expected 10 to equal 11 in module_10 while checking item 9
line 10: expected 10 to equal 11 in module_10 while checking item 10
line 11: expected 10 to equal 11 in module_10 while checking item 11
````

</details>

<details open>
<summary>Failure 12 of 24</summary>

`Rust tests` › `core` › `module_11::fails_11`

`assertion failed: value 11 is out of range` · 0.4 s

````text
thread 'module_11::fails_11' panicked at crates/core/src/m11.rs:21:5:
line 0: expected 11 to equal 12 in module_11 while checking item 0
line 1: expected 11 to equal 12 in module_11 while checking item 1
line 2: expected 11 to equal 12 in module_11 while checking item 2
line 3: expected 11 to equal 12 in module_11 while checking item 3
line 4: expected 11 to equal 12 in module_11 while checking item 4
line 5: expected 11 to equal 12 in module_11 while checking item 5
line 6: expected 11 to equal 12 in module_11 while checking item 6
line 7: expected 11 to equal 12 in module_11 while checking item 7
line 8: expected 11 to equal 12 in module_11 while checking item 8
line 9: expected 11 to equal 12 in module_11 while checking item 9
line 10: expected 11 to equal 12 in module_11 while checking item 10
line 11: expected 11 to equal 12 in module_11 while checking item 11
````

</details>

<details open>
<summary>Failure 13 of 24</summary>

`Rust tests` › `core` › `module_12::fails_12`

`assertion failed: value 12 is out of range` · 0.4 s

````text
thread 'module_12::fails_12' panicked at crates/core/src/m12.rs:22:5:
line 0: expected 12 to equal 13 in module_12 while checking item 0
line 1: expected 12 to equal 13 in module_12 while checking item 1
line 2: expected 12 to equal 13 in module_12 while checking item 2
line 3: expected 12 to equal 13 in module_12 while checking item 3
line 4: expected 12 to equal 13 in module_12 while checking item 4
line 5: expected 12 to equal 13 in module_12 while checking item 5
line 6: expected 12 to equal 13 in module_12 while checking item 6
line 7: expected 12 to equal 13 in module_12 while checking item 7
line 8: expected 12 to equal 13 in module_12 while checking item 8
line 9: expected 12 to equal 13 in module_12 while checking item 9
line 10: expected 12 to equal 13 in module_12 while checking item 10
line 11: expected 12 to equal 13 in module_12 while checking item 11
````

</details>

<details open>
<summary>Failure 14 of 24</summary>

`Rust tests` › `core` › `module_13::fails_13`

`assertion failed: value 13 is out of range` · 0.5 s

````text
thread 'module_13::fails_13' panicked at crates/core/src/m13.rs:23:5:
line 0: expected 13 to equal 14 in module_13 while checking item 0
line 1: expected 13 to equal 14 in module_13 while checking item 1
line 2: expected 13 to equal 14 in module_13 while checking item 2
line 3: expected 13 to equal 14 in module_13 while checking item 3
line 4: expected 13 to equal 14 in module_13 while checking item 4
line 5: expected 13 to equal 14 in module_13 while checking item 5
line 6: expected 13 to equal 14 in module_13 while checking item 6
line 7: expected 13 to equal 14 in module_13 while checking item 7
line 8: expected 13 to equal 14 in module_13 while checking item 8
line 9: expected 13 to equal 14 in module_13 while checking item 9
line 10: expected 13 to equal 14 in module_13 while checking item 10
line 11: expected 13 to equal 14 in module_13 while checking item 11
````

</details>

<details open>
<summary>Failure 15 of 24</summary>

`Rust tests` › `core` › `module_14::fails_14`

`assertion failed: value 14 is out of range` · 0.5 s

````text
thread 'module_14::fails_14' panicked at crates/core/src/m14.rs:24:5:
line 0: expected 14 to equal 15 in module_14 while checking item 0
line 1: expected 14 to equal 15 in module_14 while checking item 1
line 2: expected 14 to equal 15 in module_14 while checking item 2
line 3: expected 14 to equal 15 in module_14 while checking item 3
line 4: expected 14 to equal 15 in module_14 while checking item 4
line 5: expected 14 to equal 15 in module_14 while checking item 5
line 6: expected 14 to equal 15 in module_14 while checking item 6
line 7: expected 14 to equal 15 in module_14 while checking item 7
line 8: expected 14 to equal 15 in module_14 while checking item 8
line 9: expected 14 to equal 15 in module_14 while checking item 9
line 10: expected 14 to equal 15 in module_14 while checking item 10
line 11: expected 14 to equal 15 in module_14 while checking item 11
````

</details>

<details open>
<summary>Failure 16 of 24</summary>

`Rust tests` › `core` › `module_15::fails_15`

`assertion failed: value 15 is out of range` · 0.6 s

````text
thread 'module_15::fails_15' panicked at crates/core/src/m15.rs:25:5:
line 0: expected 15 to equal 16 in module_15 while checking item 0
line 1: expected 15 to equal 16 in module_15 while checking item 1
line 2: expected 15 to equal 16 in module_15 while checking item 2
line 3: expected 15 to equal 16 in module_15 while checking item 3
line 4: expected 15 to equal 16 in module_15 while checking item 4
line 5: expected 15 to equal 16 in module_15 while checking item 5
line 6: expected 15 to equal 16 in module_15 while checking item 6
line 7: expected 15 to equal 16 in module_15 while checking item 7
line 8: expected 15 to equal 16 in module_15 while checking item 8
line 9: expected 15 to equal 16 in module_15 while checking item 9
line 10: expected 15 to equal 16 in module_15 while checking item 10
line 11: expected 15 to equal 16 in module_15 while checking item 11
````

</details>

<details open>
<summary>Failure 17 of 24</summary>

`Rust tests` › `core` › `module_16::fails_16`

`assertion failed: value 16 is out of range` · 0.6 s

````text
thread 'module_16::fails_16' panicked at crates/core/src/m16.rs:26:5:
line 0: expected 16 to equal 17 in module_16 while checking item 0
line 1: expected 16 to equal 17 in module_16 while checking item 1
line 2: expected 16 to equal 17 in module_16 while checking item 2
line 3: expected 16 to equal 17 in module_16 while checking item 3
line 4: expected 16 to equal 17 in module_16 while checking item 4
line 5: expected 16 to equal 17 in module_16 while checking item 5
line 6: expected 16 to equal 17 in module_16 while checking item 6
line 7: expected 16 to equal 17 in module_16 while checking item 7
line 8: expected 16 to equal 17 in module_16 while checking item 8
line 9: expected 16 to equal 17 in module_16 while checking item 9
line 10: expected 16 to equal 17 in module_16 while checking item 10
line 11: expected 16 to equal 17 in module_16 while checking item 11
````

</details>

<details open>
<summary>Failure 18 of 24</summary>

`Rust tests` › `core` › `module_17::fails_17`

`assertion failed: value 17 is out of range` · 0.6 s

````text
thread 'module_17::fails_17' panicked at crates/core/src/m17.rs:27:5:
line 0: expected 17 to equal 18 in module_17 while checking item 0
line 1: expected 17 to equal 18 in module_17 while checking item 1
line 2: expected 17 to equal 18 in module_17 while checking item 2
line 3: expected 17 to equal 18 in module_17 while checking item 3
line 4: expected 17 to equal 18 in module_17 while checking item 4
line 5: expected 17 to equal 18 in module_17 while checking item 5
line 6: expected 17 to equal 18 in module_17 while checking item 6
line 7: expected 17 to equal 18 in module_17 while checking item 7
line 8: expected 17 to equal 18 in module_17 while checking item 8
line 9: expected 17 to equal 18 in module_17 while checking item 9
line 10: expected 17 to equal 18 in module_17 while checking item 10
line 11: expected 17 to equal 18 in module_17 while checking item 11
````

</details>

<details open>
<summary>Failure 19 of 24</summary>

`Rust tests` › `core` › `module_18::fails_18`

`assertion failed: value 18 is out of range` · 0.7 s

````text
thread 'module_18::fails_18' panicked at crates/core/src/m18.rs:28:5:
line 0: expected 18 to equal 19 in module_18 while checking item 0
line 1: expected 18 to equal 19 in module_18 while checking item 1
line 2: expected 18 to equal 19 in module_18 while checking item 2
line 3: expected 18 to equal 19 in module_18 while checking item 3
line 4: expected 18 to equal 19 in module_18 while checking item 4
line 5: expected 18 to equal 19 in module_18 while checking item 5
line 6: expected 18 to equal 19 in module_18 while checking item 6
line 7: expected 18 to equal 19 in module_18 while checking item 7
line 8: expected 18 to equal 19 in module_18 while checking item 8
line 9: expected 18 to equal 19 in module_18 while checking item 9
line 10: expected 18 to equal 19 in module_18 while checking item 10
line 11: expected 18 to equal 19 in module_18 while checking item 11
````

</details>

<details open>
<summary>Failure 20 of 24</summary>

`Rust tests` › `core` › `module_19::fails_19`

`assertion failed: value 19 is out of range` · 0.7 s

````text
thread 'module_19::fails_19' panicked at crates/core/src/m19.rs:29:5:
line 0: expected 19 to equal 20 in module_19 while checking item 0
line 1: expected 19 to equal 20 in module_19 while checking item 1
line 2: expected 19 to equal 20 in module_19 while checking item 2
line 3: expected 19 to equal 20 in module_19 while checking item 3
line 4: expected 19 to equal 20 in module_19 while checking item 4
line 5: expected 19 to equal 20 in module_19 while checking item 5
line 6: expected 19 to equal 20 in module_19 while checking item 6
line 7: expected 19 to equal 20 in module_19 while checking item 7
line 8: expected 19 to equal 20 in module_19 while checking item 8
line 9: expected 19 to equal 20 in module_19 while checking item 9
line 10: expected 19 to equal 20 in module_19 while checking item 10
line 11: expected 19 to equal 20 in module_19 while checking item 11
````

</details>

<details open>
<summary>Failure 21 of 24</summary>

`Rust tests` › `core` › `module_20::fails_20`

`assertion failed: value 20 is out of range` · 0.7 s

````text
thread 'module_20::fails_20' panicked at crates/core/src/m20.rs:30:5:
line 0: expected 20 to equal 21 in module_20 while checking item 0
line 1: expected 20 to equal 21 in module_20 while checking item 1
line 2: expected 20 to equal 21 in module_20 while checking item 2
line 3: expected 20 to equal 21 in module_20 while checking item 3
line 4: expected 20 to equal 21 in module_20 while checking item 4
line 5: expected 20 to equal 21 in module_20 while checking item 5
line 6: expected 20 to equal 21 in module_20 while checking item 6
line 7: expected 20 to equal 21 in module_20 while checking item 7
line 8: expected 20 to equal 21 in module_20 while checking item 8
line 9: expected 20 to equal 21 in module_20 while checking item 9
line 10: expected 20 to equal 21 in module_20 while checking item 10
line 11: expected 20 to equal 21 in module_20 while checking item 11
````

</details>

<details open>
<summary>Failure 22 of 24</summary>

`Rust tests` › `core` › `module_21::fails_21`

`assertion failed: value 21 is out of range` · 0.8 s

````text
thread 'module_21::fails_21' panicked at crates/core/src/m21.rs:31:5:
line 0: expected 21 to equal 22 in module_21 while checking item 0
line 1: expected 21 to equal 22 in module_21 while checking item 1
line 2: expected 21 to equal 22 in module_21 while checking item 2
line 3: expected 21 to equal 22 in module_21 while checking item 3
line 4: expected 21 to equal 22 in module_21 while checking item 4
line 5: expected 21 to equal 22 in module_21 while checking item 5
line 6: expected 21 to equal 22 in module_21 while checking item 6
line 7: expected 21 to equal 22 in module_21 while checking item 7
line 8: expected 21 to equal 22 in module_21 while checking item 8
line 9: expected 21 to equal 22 in module_21 while checking item 9
line 10: expected 21 to equal 22 in module_21 while checking item 10
line 11: expected 21 to equal 22 in module_21 while checking item 11
````

</details>

<details open>
<summary>Failure 23 of 24</summary>

`Rust tests` › `core` › `module_22::fails_22`

`assertion failed: value 22 is out of range` · 0.8 s

````text
thread 'module_22::fails_22' panicked at crates/core/src/m22.rs:32:5:
line 0: expected 22 to equal 23 in module_22 while checking item 0
line 1: expected 22 to equal 23 in module_22 while checking item 1
line 2: expected 22 to equal 23 in module_22 while checking item 2
line 3: expected 22 to equal 23 in module_22 while checking item 3
line 4: expected 22 to equal 23 in module_22 while checking item 4
line 5: expected 22 to equal 23 in module_22 while checking item 5
line 6: expected 22 to equal 23 in module_22 while checking item 6
line 7: expected 22 to equal 23 in module_22 while checking item 7
line 8: expected 22 to equal 23 in module_22 while checking item 8
line 9: expected 22 to equal 23 in module_22 while checking item 9
line 10: expected 22 to equal 23 in module_22 while checking item 10
line 11: expected 22 to equal 23 in module_22 while checking item 11
````

</details>

<details open>
<summary>Failure 24 of 24</summary>

`Rust tests` › `core` › `module_23::fails_23`

`assertion failed: value 23 is out of range` · 0.9 s

````text
thread 'module_23::fails_23' panicked at crates/core/src/m23.rs:33:5:
line 0: expected 23 to equal 24 in module_23 while checking item 0
line 1: expected 23 to equal 24 in module_23 while checking item 1
line 2: expected 23 to equal 24 in module_23 while checking item 2
line 3: expected 23 to equal 24 in module_23 while checking item 3
line 4: expected 23 to equal 24 in module_23 while checking item 4
line 5: expected 23 to equal 24 in module_23 while checking item 5
line 6: expected 23 to equal 24 in module_23 while checking item 6
line 7: expected 23 to equal 24 in module_23 while checking item 7
line 8: expected 23 to equal 24 in module_23 while checking item 8
line 9: expected 23 to equal 24 in module_23 while checking item 9
line 10: expected 23 to equal 24 in module_23 while checking item 10
line 11: expected 23 to equal 24 in module_23 while checking item 11
````

</details>

The list of 8 flaky tests was left out to fit GitHub's size limit. The table counts them.

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
