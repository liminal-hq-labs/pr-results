# Inputs

What `pr-results` reads, where each reporter writes it, and what the parser expects. The normalized model, the detection rules and the step-results contract are in [SPEC.md](../SPEC.md#inputs). Fixtures for each producer live in `tests/fixtures/`.

Everything here is untrusted input and is handled by the rules in [security.md](security.md).

## Reporters and flavours

| Input | Produced by | Where it is usually found | Format id | Notes for the parser |
| --- | --- | --- | --- | --- |
| nextest JUnit | `cargo nextest run --profile ci` with `junit = { path = "junit.xml" }` | artefact `rust-test-results`, file `target/nextest/ci/junit.xml` | `junit`, flavour `nextest` | Root `<testsuites name="nextest-run">`. One `<testsuite>` per test binary. `classname` is the binary id and `name` is the test path (`tests::names_itself`). No `file`. Retries appear as `<flakyFailure>` (passed in the end) and `<rerunFailure>`. `<system-out>` and `<system-err>` hold output when configured |
| Step results | a product's converter, such as Desk's `tools/steps-junit` | artefact `steps-test-results`, file `steps-junit.xml` | `junit`, flavour `steps` | The contract is in [SPEC.md](../SPEC.md#the-step-results-contract) |
| `CHECK_TIMES` (could) | `CHECK_TIMES=FILE tools/check.sh` in Desk | artefact `steps-test-results`, file `check-times.tsv` | `steps-times` | Lines `start\t<ms>\t<group>\t<title>` and one `end\t<ms>\t<status>`. The failing step is the last `start` before a non-zero `end` |
| vitest JUnit | `vitest run --reporter=default --reporter=junit --outputFile.junit=…` | `frontend-test-results`, `js-test-results`, `test-results/**/junit.xml` | `junit`, flavour `vitest` | Root `<testsuites name="vitest tests">`. One suite per file, named for the file relative to the vitest root. `classname` is the same path and `name` joins the describe blocks and the title with ` > `. `hostname` is the machine's and is never shown. Todo and skip are both `<skipped/>`. A test that kept failing under `retry` has one `<failure>` per attempt; one that failed and then passed leaves no trace. Paths are relative to the vitest root, so a monorepo needs `path_prefix` |
| bun test JUnit | `bun test --reporter=junit --reporter-outfile=…` | `language-test-results`, `mcp-test-results` | `junit`, flavour `bun` | Root `<testsuites name="bun test">` with `assertions`. A `<testsuite>` per file, then a nested one per `describe`, all with `file`, and `line` on the describe blocks. A case has `name` (the title only), `classname` (the describe blocks, innermost first), `file`, `line` (where the test starts) and `assertions`. Todo is `<skipped message="TODO"/>`. Retries are not recorded. Paths are relative to where `bun test` ran |
| jest-junit | `jest --reporters=jest-junit` | `reports/junit.xml` | `junit`, flavour `jest` | Root `<testsuites name="jest tests">` unless `suiteName` is set. A suite per test file, named for its first describe block. `classname` and `name` are both the whole title, joined by spaces. `file` only with `addFileAttribute: "true"`. A todo test is a plain passing case. Retries are not recorded. The failure body has absolute paths and the framework's own frames |
| Playwright JUnit | `reporter: [['junit', { outputFile }]]`, often sharded | one file per shard | `junit`, flavour `playwright` | Root `<testsuites id="" name="">`. A suite per project and spec file: `name` is the spec relative to `testDir` and `hostname` is the project. `classname` is the spec, `name` is `describe › title`. A failed `expect` is `<failure>` and a thrown error is `<error>`. The text holds every attempt as `Retry #n` sections. A flaky test is a plain pass. Attachments appear as `[[ATTACHMENT\|…]]` in `system-out` |
| Python (planned) | `unittest-xml-reporting` or `pytest --junitxml` | any | `junit`, flavour `python` | `classname` is the dotted module path. `file` from pytest. Not a flavour yet: read it as `generic` |
| Generic JUnit | anything else (Gradle, GUT, cargo2junit) | any | `junit`, flavour `generic` | Surefire-style attributes only |
| Generic step JSON (could) | a script | any | `steps-json` | The schema is below |

Detection with `flavour = "auto"` picks by the root's `name` attribute (`nextest-run`, `vitest tests`, `bun test`, `jest tests`), by `assertions` attributes (bun), by an empty `name` beside an `id` on the root (Playwright) and by `[[ATTACHMENT|` (Playwright), and falls back to `generic`. A jest-junit file whose `suiteName` was changed reads as `generic` until the suite sets `flavour = "jest"`.

## What the JUnit parser accepts and rejects

One streaming reader serves every flavour, and the flavour only decides how names and locations are read. The code is `parse_junit` in `pr-results-core`.

**Accepted shapes.** A `<testsuites>` root with `<testsuite>` children, a bare `<testsuite>` root, nested `<testsuite>` elements (flattened into their top-level suite), and an empty `<testsuites/>` (no suites). Inside a `<testcase>`: `<failure>`, `<error>`, `<skipped>`, `<flakyFailure>`, `<flakyError>`, `<rerunFailure>`, `<rerunError>` (with the text in the element or in a `<stack-trace>` child, as nextest writes it), `<system-out>` and `<system-err>`. Text may be plain or CDATA. Anything else, such as `<properties>`, is skipped. A leading byte-order mark is ignored and invalid UTF-8 is replaced with U+FFFD.

**Rejected, with an error that names the problem and quotes nothing from the file.**

| Input | Error |
| --- | --- |
| No content, or only whitespace | the input is empty |
| More than 50 MiB (configurable), as given or after invalid UTF-8 is replaced | too large |
| Ends before the root closes, or inside a tag, comment or CDATA section | truncated |
| A `<!DOCTYPE>`, whatever it declares | not accepted |
| An entity other than `&lt;` `&gt;` `&amp;` `&apos;` `&quot;` and numeric references, or `&#0;` | not accepted |
| An `encoding` other than UTF-8, or UTF-16 content | not accepted |
| A NUL byte | not well formed |
| Mismatched or duplicate tags and attributes, a second root, text outside the root | not well formed |
| A root other than `testsuites` or `testsuite` | unexpected root |
| A `<testcase>` directly under `<testsuites>` | structure |
| More than 64 levels of elements, 10 000 suites, 200 000 cases or 256 attributes on one element | over the limit |
| More than 64 MiB of kept text and per-item overhead (40 bytes per piece of text, 64 per case, 64 per attempt), counted after invalid UTF-8 is replaced | over the limit |

Names, class names, paths, messages and bodies longer than their caps (1 KiB, 1 KiB and 16 KiB) are cut while streaming, and the number of characters lost is kept and shown. Retries past 100 per case are not kept, and their number is kept and shown too. A suite's `file` is stored once and shared by its cases. A `time` or `line` that is not plain digits is read as zero or absent rather than failing the file. A `time` is read as seconds with no floating-point maths, rounded to the nearest millisecond.

**Status.** `<failure>` gives failed, `<error>` gives errored and `<skipped>` gives skipped, and a failure wins over a skip. A case with no failure, error or skip and at least one `<flakyFailure>` or `<flakyError>` is flaky. `<rerunFailure>` and `<rerunError>` are recorded as attempts of a case that stays failed, and so is every `<failure>` or `<error>` after the first in the same case, which is how vitest writes a test that kept failing under `retry`. A suite with no `time` takes the sum of its cases.

**Locations.** `file` and `line` attributes are read, and a suite's `file` is inherited by its cases. When there is none: nextest reads `panicked at <path>:<line>:<col>` from the failure (the current and the older panic formats); vitest takes the class name, then the suite name; Playwright takes the class name before ` › `, then the suite name. For the four JavaScript flavours a failure's text is then searched for a stack frame, and the first one that names a place in the repository gives the file and line, in place of the file the case started in and the line an attribute gave. See [Stack frames](#stack-frames). Every path must be relative, with no `..` (except as below), drive letter, URL or control character, or it is dropped. A suite's `path_prefix` is put in front of every path a report names, whichever way it was found.

**Flavours as implemented.** `nextest`, `steps`, `vitest`, `jest`, `bun`, `playwright` and `generic`; `auto` settles on one as described above, and never picks `steps`. The four JavaScript readers were tuned against reports the reporters wrote for the sample projects in `tests/sample-projects` (see [What the reporters write](#what-the-reporters-write)).

## Stack frames

A frame is a line that starts with the word `at` or the marker `❯` and ends in a place: `at fn (path:line:col)`, `at path:line:col`, `at path:line` (Playwright) and vitest's `❯ fn path:line:col`. The first frame that names a place in the repository is the location of the failure.

| Frame | Result |
| --- | --- |
| `src/a.js:4:5`, `./src/a.js:4:5`, `src\a.js:4:5` | used |
| a path inside `node_modules`, `node:internal/…`, `<anonymous>`, a name with no extension | skipped |
| `../x.js:4:5`, `a/../b.js:4:5` | skipped, unless a `path_prefix` is set and the `..` leads the path: it climbs out of the prefix and never past the repository root (`../src/x.js` with `e2e/` is `src/x.js`) |
| an absolute path, `file://…`, `C:\…`, a UNC path | skipped, unless it ends with `/` and the case's own file: jest and Playwright write absolute paths, the runner's checkout is not known, but the case's file is (from `addFileAttribute` or the class name), so the line is kept for that file |
| a line of 0, over 4 294 967 295, or not a number | skipped |
| text that only looks like a frame (`expected at src/a.js:4:5`, `- at …`) | not a frame: the line must start with the marker |

Playwright starts a failure with `[project] › path:line:col › titles`, the place the test is declared. It is used only when no frame was usable.

## What the reporters write

Everything here was read from real output: the sample projects under `tests/sample-projects` (a pass, a failure with a stack, a throw from a source file, nested describes, skip, todo, a test that passes on a retry, one that never does and one outside any describe) run by `scripts/gen-js-fixtures.sh` in pinned containers: vitest 5.1.0 and jest 30.5.2 with jest-junit 17.0.0 on Node 22.23.5, bun 1.4.4 and Playwright 1.65.0. The files are the `real-*.xml` fixtures, and `tests/models/` holds what each reads as. Other versions may differ.

### vitest

- **Names.** `classname` is the file relative to the vitest root, the same text as the suite's `name`. `name` is the describe blocks and the title joined by ` > `. `addFileAttribute: true` adds `file`, equal to `classname`, so it changes nothing the reader needs.
- **Skip and todo** are both `<skipped/>`: they cannot be told apart.
- **Retries.** With `retry: 2` a test that failed three times has three `<failure>` elements in one `<testcase>`, and the reader keeps the first as the failure and the others as attempts. A test that failed and then passed is a plain passing case with no mark: vitest's JUnit cannot show a flaky test.
- **Projects.** In `projects` mode nothing in the XML names the project (the suites are named for their files). Two projects that run the same file would give two suites with one name, which read as one.
- **Failure text.** `message` is the assertion's message and `type` the error's name. The body is the message, the diff, and frames written as `❯ path:line:col` or `❯ fn path:line:col`, relative to the vitest root, and already without `node_modules`. The first frame is where the assertion or the throw happened, so a throw from a source file locates that file.
- **Times** are seconds with up to nine decimals.

### bun

- **Structure.** A `<testsuite>` per file, then one nested `<testsuite>` per `describe`, each with `file`; `line` is where the describe starts. The reader flattens them into the file's suite. Cases have `file`, `line` (where the test starts) and `assertions`.
- **Names.** `name` is the title alone and `classname` the describe blocks, innermost first (`rounding > division > math`). The reader turns them round (`math > division > rounding`). A test outside any describe has an empty `classname`.
- **Failures.** The `message` attribute holds the whole assertion, newlines as `&#10;`, and the body repeats it with frames `at fn (path:line:col)` or `at path:line:col`, relative to where bun ran. A failure with no message (`throw new Error('')`) has no `message` attribute and a body that starts with `Error`; the first line of the body is its message. The reader shows only the first line of a multi-line message.
- **Skip and todo.** `<skipped />` and `<skipped message="TODO" />`.
- **Retries.** `--retry 2` leaves a test that failed and then passed as a plain pass (its `assertions` count the passing attempt), and a test that never passes as one `<failure>`. Nothing records the retry. `--rerun-each 2` repeats every case in the file under the same names, each with its own result; the reader keeps them as separate cases.
- **`hostname`** is on every suite and never shown.

### jest-junit

- **Names.** A suite per test file, named for its first describe block (`{title}`, the template's default), not for the file. `classname` and `name` are both the whole title joined by spaces, so a title shows once (the reader empties a `classname` equal to the name). A test outside any describe starts with a space, which the reader trims.
- **Files.** There is no `file` unless `addFileAttribute: "true"` is set, and then it is relative to jest's root with no line. Set it: without it the only path in a report is absolute.
- **Failures.** A bare `<failure>` with no attributes; the body is the message and a stack with absolute paths (`/work/test/a.test.js:28:23`) and many frames of `jest-circus` and `jest-runner` under `node_modules`. The first line of the body is the message. An absolute frame is used only when it ends with the case's own `file`, so with `addFileAttribute` a failure has its line, and without it a failure has no location.
- **Todo** is a plain passing case with no `<skipped/>`; **skip** is `<skipped/>`.
- **Retries.** `jest.retryTimes(2)` leaves a test that failed and then passed as a plain pass, and a test that never passes as one `<failure>`.
- **Templates.** `suiteName` replaces the root's name (`jest tests`), so detection reads the file as `generic`; set `flavour = "jest"`. The `classNameTemplate` and `titleTemplate` options move text between `classname` and `name`, and both are shown.

### Playwright

- **Structure.** `<testsuites id="" name="">`, then a `<testsuite>` for every project and spec file: `name` is the spec relative to `testDir` and `hostname` is the **project** (`chromium`). The same spec in two projects is two suites with the same name. The reader keeps the project as part of the suite (`math.spec.js [chromium]`) and merges only suites of the same project, so shards of one project become one suite and projects stay apart.
- **Names.** `classname` is the spec file and `name` is `describe › title`. There is no `file` or `line` attribute.
- **Skip and fixme** are `<skipped>` with a `<properties>` block that names the kind (`skip`, `fixme`), and have no `time`.
- **Failures.** A failed `expect` is `<failure type="expect.toBe">` and a thrown error is `<error type="Error">`, so they read as failed and errored. The body is CDATA: a heading `[chromium] › math.spec.js:24:4 › titles ───`, the error, a code excerpt and a frame. Frames are `at /abs/test/math.spec.js:26:23` (absolute) or `at ../src/math.js:12` (relative to `testDir`, with no column), so set `path_prefix` to the repository path of `testDir` and `../src/math.js` becomes a repository path.
- **Retries.** With `retries: 2` a test that kept failing has one `<failure>` or `<error>` whose text repeats the error under `Retry #1` and `Retry #2` headings, and `system-out` lists an attachment for every attempt. The reader does not split the text into attempts. A test that failed and then passed is a plain passing case; its only mark is an `error-context.md` attachment in `system-out`, which is a trace of the version, not a contract, and is not read. Playwright's JUnit cannot show a flaky test.
- **Attachments** are paths relative to the XML file, not to the repository.
- **Shards.** `--shard=1/2` splits the tests across projects and files, so a project's suites can arrive in different files; same-named suites of the same project merge.

## How report text is shown

Names, paths and failure text from a report reach a comment only through `Safe`, and the renderer puts them in `Safe::code_span` (inline, one line, the delimiter adapts to backticks) or `Safe::code_block` (adaptive fence), because GitHub does not look for mentions or issue references in code. `Safe::text` is a fallback for the fixed words around them: it writes `@` and `#` as the fullwidth U+FF20 and U+FF03, and a hyphen between a letter or digit and a digit (`GH-123`) as U+FF0D, which are visible and, in one sandbox, produced no mention or reference (Q9, D43). Real teams, cross-repository references and notifications are untested.

## Event and baseline inputs

| Input | Produced by | Where it is found | Trust |
| --- | --- | --- | --- |
| `workflow_run` event | Actions | `GITHUB_EVENT_PATH` of the publisher | Trusted: `workflow_run.id`, `head_sha`, `head_branch`, `head_repository.full_name`, `event`, `pull_requests[]` (empty for forks), `artifacts_url`, `html_url` and `conclusion` |
| Uploaded pull request event file | `actions/upload-artifact` of `${{ github.event_path }}` in the CI run | artefact `event-file`, file `event.json` | **Untrusted.** A fork controls it. Used only as a tie-breaker: its `pull_request.number` (an `event.json` of at most 1 MiB) chooses among pull requests the trusted event and the API already matched to the run, and only when there is more than one |
| Base-branch results (should) | the CI run on the base branch | that run's result artefacts | Trusted only when the run's `event` is `push`, its `head_repository` is the base repository and its branch is the base branch |
| Configuration | the repository | `.github/pr-results.toml` on the default branch | Trusted. Never read from the pull request head |

## Artefact names

Without a configuration file, suites are inferred from artefacts that match `*-test-results*`, one suite per artefact: `rust-test-results`, `frontend-test-results` or `js-test-results`, `language-test-results`, `mcp-test-results`. An `event-file` artefact holds `${{ github.event_path }}`. Repositories that already upload artefacts under these names need no change to their CI.

## Generic step JSON (planned)

```json
{
	"schema": "pr-results-steps/1",
	"suite": "check.sh",
	"steps": [
		{ "group": "base", "title": "Licence headers", "status": "passed", "duration_ms": 412 },
		{ "group": "dos", "title": "DESK.EXE in headless DOSBox", "status": "failed", "duration_ms": 18234, "exit_status": 1, "log_tail": "…" }
	]
}
```

`status` is `passed`, `failed`, `skipped` or `cancelled`. Unknown fields are ignored. Unknown `schema` values are an input error.

## Notes on real files

- JUnit files carry the runner's hostname (`hostname="…"` in vitest and bun output). `pr-results` never prints `hostname`, `timestamp` or properties by default. Playwright's `hostname` is its project, which is shown beside the spec in brackets.
- Per-case `detail` is capped at 16 KiB and `message` at 1 KiB at parse time, whatever the file holds.
- Annotations need repository-relative paths. The JavaScript reporters write paths relative to where they ran (the vitest root, the directory `bun test` ran in, jest's root, Playwright's `testDir`), so a suite's `path_prefix` maps them onto the repository: `path_prefix = "apps/web/"` turns `src/a.ts` into `apps/web/src/a.ts`.
- To share a report from a private project as a fixture, run it through the anonymizer first: [anonymizing-reports.md](anonymizing-reports.md).
