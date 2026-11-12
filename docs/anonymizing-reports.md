# Anonymizing reports

A report from a private project cannot go into this public repository as it is: its test names, class names, paths, messages and output name the project. `scripts/anonymize-junit.sh` rewrites a JUnit file so that it keeps its shape and none of its words, so the readers can be tuned against the real thing (decision D63).

## What it keeps and what it replaces

**Every word and number becomes a token.** A word is a run of letters, digits and underscores in any script. A token is a one-letter prefix and the first six bytes of an HMAC-SHA-256 of the word, keyed by the salt, as twelve lowercase hex digits (48 bits): `w` for a word, `n` for a number, `x` for an element or attribute name. It replaces the word in names, class names, `file` attributes, messages, failure text and captured output. One word is one token wherever it occurs, so a class name, a `file` attribute and a stack frame still agree, and two different words do not share a token (48 bits: collisions are not expected below millions of distinct words).

**Names.** Only the JUnit vocabulary the readers use is kept as it is: the elements `testsuites`, `testsuite`, `testcase`, `failure`, `error`, `skipped`, `system-out`, `system-err`, `properties`, `property`, `rerunFailure`, `flakyFailure`, `rerunError`, `flakyError` and `stack-trace`, and the attributes `name`, `classname`, `file`, `line`, `time`, `tests`, `failures`, `errors`, `skipped`, `disabled`, `timestamp`, `hostname`, `id`, `message`, `type`, `package`, `status`, `assertions` and `xmlns`. Any other element or attribute name, and any namespace prefix, becomes an `x` token that is still a valid XML name; an `xmlns:prefix` declaration keeps its meaning with the token as the prefix.

**Numbers.** A count or a line in `tests`, `failures`, `errors`, `skipped`, `disabled`, `assertions` or `line` stays when it has up to four digits, and a `time` stays when it is a decimal of up to four digits before the point. The line and column that end a stack frame (`at fn (src/a.js:12:5)`), a panic location (`panicked at src/lib.rs:12:5:`) and each part of a Playwright heading (`[chromium] › a.spec.js:24:4 › titles`) stay when they are up to four digits and the text before the colon looks like a file (a dot, then letters). Every other number is replaced: an address (`10.0.0.12`), a port (`:5432`, `localhost:3000`), an identifier, a count in prose, and any number of five digits or more.

**A few words are kept**, because the readers look for them and they belong to the tools, not to the project: `at`, `async`, `new`, `node`, `internal`, `file`, `anonymous`, `node_modules`, `ATTACHMENT`, `Retry` and `panicked`. So are the root `name` that says which tool wrote the file (`vitest tests`, `bun test`, `jest tests`, `nextest-run`), the `type` of a failure when it is one of the language's own (`Error`, `AssertionError`, `TypeError`, `RangeError`, `ReferenceError`, `SyntaxError`, `EvalError`, `URIError`, `FAILURE`, `ERROR`, `test failure`) and a Playwright project named `chromium`, `firefox` or `webkit`.

**Other changes.** Every `timestamp` becomes `2026-01-01T00:00:00.000Z`. Comments and processing instructions are dropped. CDATA sections are written as ordinary text. Characters XML 1.0 forbids become U+FFFD. A file that is not well formed, has a document type or an entity, declares an encoding other than UTF-8 or is over 50 MiB is refused, as the parser refuses it.

## What it does not hide

The output is not a description of the report's meaning, but it is a picture of its shape, and some of the shape is the project's:

- **Punctuation, spacing, indentation, line breaks and layout** stay as they were: a path is still `a/b/c.d`, so the **depth** of a path and the number of dots in a file name show; a message still has its commas, quotes and brackets.
- **The number of words** in every name, message and line shows, and so does the number of lines in a failure. A word's **length** does not: every token is the same length.
- **Which words repeat**, and where, shows: the same word is the same token, so a reader can see that a name occurs in forty test names and in three stack frames, and can compare the frequencies with those of a guessed vocabulary.
- **The tree and the totals** stay: how many suites, cases, failures, skips and attempts, which cases fail, their order and their times.
- **Short numbers** that are counts or lines stay as written, and the total of cases per suite is therefore known.
- **A kept word** (above) shows that the report had a stack frame, a `node_modules` path or an attachment.
- A port that follows a host name with a dot at the end of a frame (`at f (api.example.com:8443)`) looks like a position and stays; a bare host (`localhost:3000`) and an address do not.

## What the salt does

**The salt is the key.** HMAC-SHA-256 under a secret key is indistinguishable from random bytes to someone without the key, so a token cannot be tied to a word, and a guessed word cannot be confirmed, as long as the salt has enough entropy to resist being guessed itself: 128 bits or more, which is what `--random-salt` makes (sixteen random bytes as thirty-two hex digits). With such a salt, nobody without it can test a guess, and the 48-bit tokens would also be too short to invert if they could.

**A weak salt gives that up.** The salt is the only secret, and the words of a JUnit file are mostly known (`describe`, `expect`, `test`, a framework's messages). Someone who has the output and a short, human or reused salt can try candidate salts against those known words until a token matches, and then test any word they like. `--salt` is therefore required (there is no default), a salt under sixteen bytes draws a warning, and a salt that is a password, a project name or a date should be treated as no protection.

**Keep the salt out of the repository** and out of the report you hand over. The same salt gives the same output, which lets two reports from one run be compared; use a new salt for each project. Knowing the salt, anyone with the original can reproduce the tokens, so discard it once the fixture is made, unless you need to regenerate it.

## The procedure

1. Run the project's tests with the JUnit reporter as CI does, and take the file: for example `reports/junit/app.xml`.
2. Anonymize it, with a salt that is not written down in the repository:

   ```sh
   scripts/anonymize-junit.sh --random-salt reports/junit/app.xml app-anon.xml
   ```

   The command prints the salt it made on standard error; it is only needed to make the same output again. To use your own, `--salt` takes a string of sixteen bytes or more (and warns below that). One of the two is required.

   The script runs `cargo run --locked --release -p pr-results-core --example anonymize_junit`, so it needs a Rust toolchain. The same command in the shared image, when there is none:

   ```sh
   docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/w" -w /w -e CARGO_HOME=/w/.cargo-home -e CARGO_TARGET_DIR=/w/.target ghcr.io/liminal-hq/ci-rust@sha256:0ef31efa5876407765cd5572a23323932183a5357eaac92eee67d79657d45c02 scripts/anonymize-junit.sh --random-salt reports/junit/app.xml app-anon.xml
   ```

   Delete `.cargo-home` and `.target` afterwards; they are build output.
3. Open `app-anon.xml` and check it: no name you recognize, no path, no message, no address, no token. Look at the failure text and `system-out` in particular, and read [What it does not hide](#what-it-does-not-hide).
4. Trim it to what is needed for a fixture: a few suites, the failing cases with their stacks, a skip, a retry. Keep the root and the first suite's attributes.
5. Hand it over as `tests/fixtures/<flavour>/anonymized-<what>.xml`, or send it to whoever tunes the readers. Keep the original private.

For a large report the whole file can stay local: the ignored test in `crates/pr-results-core/tests/js_scale.rs` is the timing and memory check, and it generates its own 5 200 cases.

## How it is tested

`crates/pr-results-core/tests/anonymize.rs` runs it over every fixture in the repository and checks that the output is the same for the same input and salt, that another salt gives other tokens, that no word of the input survives (apart from the kept words above), that no element, attribute or namespace name outside the vocabulary survives, that addresses, ports and long numbers are replaced while counts and positions stay, that the parsed structure of every fixture is unchanged (the same suites, statuses, times, lines, attempts and number of path components), that 20 000 distinct words give 20 000 tokens, and that odd and hostile input is refused or written as well-formed XML. A made-up report with a company name, an address, a secret and a private package shows what a reader is left with.
