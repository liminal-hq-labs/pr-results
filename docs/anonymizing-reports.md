# Anonymizing reports

A report from a private project cannot go into this public repository as it is: its test names, class names, paths, messages and output name the project. `scripts/anonymize-junit.sh` rewrites a JUnit file so that it keeps its shape and none of its words, so the readers can be tuned against the real thing (decision D63).

## What it keeps and what it replaces

**Every word becomes a token.** A word is a run of letters, digits and underscores in any script. It becomes `w` and ten hex digits, a salted hash of the word, in names, class names, `file` attributes, messages, failure text and captured output. One word is one token wherever it occurs, so a class name, a `file` attribute and a stack frame still agree, and two different words do not share a token.

**The structure stays.** Elements and attribute names, the counts (`tests`, `failures`, `errors`, `skipped`, `assertions`), the times, the line and column numbers, short numbers (up to four digits), and all punctuation, whitespace and layout around the words. A stack frame is still `at wA.wB (/wC/wD/wE.wF:12:5)`, an indented diff is still indented, and a path still has the same number of components and the same dots.

**A few words are kept**, because the readers look for them and they belong to the tools, not to the project: `at`, `async`, `new`, `node`, `internal`, `file`, `anonymous`, `node_modules`, `ATTACHMENT`, `Retry` and `panicked`. So are the root `name` that says which tool wrote the file (`vitest tests`, `bun test`, `jest tests`, `nextest-run`), the `type` of a failure when it is one of the language's own (`Error`, `AssertionError`, `TypeError`, `RangeError`, `ReferenceError`, `SyntaxError`, `EvalError`, `URIError`, `FAILURE`, `ERROR`, `test failure`) and a Playwright project named `chromium`, `firefox` or `webkit`.

**Other changes.** Every `timestamp` becomes `2026-01-01T00:00:00.000Z`. Comments and processing instructions are dropped. CDATA sections are written as ordinary text. Characters XML 1.0 forbids become U+FFFD. A file that is not well formed, has a document type or an entity, declares an encoding other than UTF-8 or is over 50 MiB is refused, as the parser refuses it.

## What it does not do

**It does not encrypt.** A token hides a word from a reader. Someone who can guess the word, such as a product name, can hash it the same way and compare. With the default (empty) salt the tokens of the same word are the same in every report, which also lets two reports be compared. Give `--salt` a private string and keep it out of the repository: the output is still deterministic for that salt, and nobody without the salt can test a guess.

**It does not read meaning.** A number longer than four digits in prose is replaced, but a short one (a port, an index) is kept. A symbol, an emoji or punctuation is kept. Read the output before sharing it, and share only what a fixture needs: one run's file, trimmed to a few suites.

## The procedure

1. Run the project's tests with the JUnit reporter as CI does, and take the file: for example `reports/junit/app.xml`.
2. Anonymize it, with a salt that is not written down in the repository:

   ```sh
   SALT="$(openssl rand -hex 16)"
   scripts/anonymize-junit.sh --salt "$SALT" reports/junit/app.xml app-anon.xml
   ```

   The script runs `cargo run --locked --release -p pr-results-core --example anonymize_junit`, so it needs a Rust toolchain. The same command in the shared image, when there is none:

   ```sh
   docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/w" -w /w -e CARGO_HOME=/w/.cargo-home -e CARGO_TARGET_DIR=/w/.target ghcr.io/liminal-hq/ci-rust@sha256:0ef31efa5876407765cd5572a23323932183a5357eaac92eee67d79657d45c02 scripts/anonymize-junit.sh --salt "$SALT" reports/junit/app.xml app-anon.xml
   ```

   Delete `.cargo-home` and `.target` afterwards; they are build output.
3. Open `app-anon.xml` and check it: no name you recognize, no path, no message, no address, no token. Look at the failure text and `system-out` in particular.
4. Trim it to what is needed for a fixture: a few suites, the failing cases with their stacks, a skip, a retry. Keep the root and the first suite's attributes.
5. Hand it over as `tests/fixtures/<flavour>/anonymized-<what>.xml`, or send it to whoever tunes the readers. Keep the original private.

For a large report the whole file can stay local: the ignored test in `crates/pr-results-core/tests/js_scale.rs` is the timing and memory check, and it generates its own 5 200 cases.

## How it is tested

`crates/pr-results-core/tests/anonymize.rs` runs it over every fixture in the repository and checks that the output is the same for the same input and salt, that no word of the input survives (apart from the kept words above and numbers), that the parsed structure of every fixture is unchanged (the same suites, statuses, times, lines, attempts and number of path components), that 20 000 distinct words give 20 000 tokens, and that odd and hostile input is refused or written as well-formed XML. A made-up report with a company name, an address, a secret and a private package shows what a reader is left with.
