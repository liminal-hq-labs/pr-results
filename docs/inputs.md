# Inputs

What `pr-results` reads, where each reporter writes it, and what the parser expects. The normalized model, the detection rules and the step-results contract are in [SPEC.md](../SPEC.md#inputs). Fixtures for each row live in `tests/fixtures/` once the parsers land.

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
| Python | `unittest-xml-reporting` or `pytest --junitxml` | any | `junit`, flavour `python` | `classname` is the dotted module path. `file` from pytest |
| Generic JUnit | anything else (Gradle, GUT, cargo2junit) | any | `junit`, flavour `generic` | Surefire-style attributes only |
| Generic step JSON (could) | a script | any | `steps-json` | The schema is below |

Detection with `flavour = "auto"` picks by the root's `name` attribute (`nextest-run`, `vitest tests`, `bun test`), by `assertions` attributes (bun) and by `[[ATTACHMENT|` (Playwright), and falls back to `generic`.

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
