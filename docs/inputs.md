# Inputs

What `pr-results` reads, where each reporter writes it, and what the parser expects. The normalized model, the detection rules and the step-results contract are in [SPEC.md](../SPEC.md#inputs). Fixtures for each producer live in `tests/fixtures/`.

Everything here is untrusted input and is handled by the rules in [security.md](security.md).

## Reporters and flavours

| Input | Produced by | Where it is usually found | Format id | Notes for the parser |
| --- | --- | --- | --- | --- |
| nextest JUnit | `cargo nextest run --profile ci` with `junit = { path = "junit.xml" }` | artefact `rust-test-results`, file `target/nextest/ci/junit.xml` | `junit`, flavour `nextest` | Root `<testsuites name="nextest-run">`. One `<testsuite>` per test binary. `classname` is the binary id and `name` is the test path (`tests::names_itself`). No `file`. Retries appear as `<flakyFailure>` (passed in the end) and `<rerunFailure>`. `<system-out>` and `<system-err>` hold output when configured |
| Step results | a product's converter, such as Desk's `tools/steps-junit` | artefact `steps-test-results`, file `steps-junit.xml` | `junit`, flavour `steps` | The contract is in [SPEC.md](../SPEC.md#the-step-results-contract) |
| `CHECK_TIMES` (could) | `CHECK_TIMES=FILE tools/check.sh` in Desk | artefact `steps-test-results`, file `check-times.tsv` | `steps-times` | Lines `start\t<ms>\t<group>\t<title>` and one `end\t<ms>\t<status>`. The failing step is the last `start` before a non-zero `end` |
| vitest JUnit | `vitest run --reporter=default --reporter=junit --outputFile.junit=…` | `frontend-test-results`, `js-test-results`, `test-results/**/junit.xml` | `junit`, flavour `vitest` | Root `<testsuites name="vitest tests">`. One suite per file. `classname` is the file path relative to the vitest root, not the repository. `name` joins describe blocks with ` > `. A `hostname` attribute is present and is never shown |
| bun test JUnit | `bun test --reporter=junit --reporter-outfile=…` | `language-test-results`, `mcp-test-results` | `junit`, flavour `bun` | Nested `<testsuite>` per `describe`. `file` and `line` on suites and cases. An `assertions` attribute |
| jest-junit | `jest --reporters=jest-junit` | `reports/junit.xml` | `junit`, flavour `jest` | `file` only with `addFileAttribute`. The `classname` and `title` templates vary by configuration |
| Playwright JUnit | `reporter: [['junit', { outputFile }]]`, often sharded | one file per shard | `junit`, flavour `playwright` | Suites per spec file and project. Shards with the same suite name are merged. Attachments appear as `[[ATTACHMENT\|…]]` in `system-out` |
| Python (planned) | `unittest-xml-reporting` or `pytest --junitxml` | any | `junit`, flavour `python` | `classname` is the dotted module path. `file` from pytest. Not a flavour yet: read it as `generic` |
| Generic JUnit | anything else (Gradle, GUT, cargo2junit) | any | `junit`, flavour `generic` | Surefire-style attributes only |
| Generic step JSON (could) | a script | any | `steps-json` | The schema is below |

Detection with `flavour = "auto"` picks by the root's `name` attribute (`nextest-run`, `vitest tests`, `bun test`), by `assertions` attributes (bun) and by `[[ATTACHMENT|` (Playwright), and falls back to `generic`.

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

**Status.** `<failure>` gives failed, `<error>` gives errored and `<skipped>` gives skipped, and a failure wins over a skip. A case with no failure, error or skip and at least one `<flakyFailure>` or `<flakyError>` is flaky. `<rerunFailure>` and `<rerunError>` are recorded as attempts of a case that stays failed. A suite with no `time` takes the sum of its cases.

**Locations.** `file` and `line` attributes are read, and a suite's `file` is inherited by its cases. When there is none: nextest reads `panicked at <path>:<line>:<col>` from the failure (the current and the older panic formats); vitest takes the class name, then the suite name; Playwright takes the class name before ` › `, then the suite name. Every path must be relative, with no `..`, drive letter, URL or control character, or it is dropped. JavaScript stack frames are not read yet.

**Flavours as implemented.** `nextest`, `steps`, `vitest`, `jest`, `bun`, `playwright` and `generic`; `auto` settles on one as described above, and never picks `steps`. Playwright's retry and flaky output and bun's retry output have no fixtures yet, so they read as `generic` does.

## How report text is shown

Names, paths and failure text from a report reach a comment only through `Safe`, and the renderer must put them in `Safe::code_span` (inline, one line, the delimiter adapts to backticks) or `Safe::code_block` (adaptive fence), because GitHub does not look for mentions or issue references in code. `Safe::text` is a fallback for the fixed words around them: it writes `@` and `#` as the fullwidth U+FF20 and U+FF03, and a hyphen between a letter or digit and a digit (`GH-123`) as U+FF0D, which are visible and are not verified to stop mentions and references (Q9).

## Event and baseline inputs

| Input | Produced by | Where it is found | Trust |
| --- | --- | --- | --- |
| `workflow_run` event | Actions | `GITHUB_EVENT_PATH` of the publisher | Trusted: `workflow_run.id`, `head_sha`, `head_branch`, `head_repository.full_name`, `event`, `pull_requests[]` (empty for forks), `artifacts_url`, `html_url` and `conclusion` |
| Uploaded pull request event file | `actions/upload-artifact` of `${{ github.event_path }}` in the CI run | artefact `event-file`, file `event.json` | **Untrusted.** A fork controls it. Used only as a hint for the pull request number, always verified |
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

- JUnit files carry the runner's hostname (`hostname="…"` in vitest and bun output). `pr-results` never prints `hostname`, `timestamp` or properties by default.
- Per-case `detail` is capped at 16 KiB and `message` at 1 KiB at parse time, whatever the file holds.
- Annotations need repository-relative paths. vitest paths are relative to the vitest root, so `path_prefix` maps them onto the repository.
