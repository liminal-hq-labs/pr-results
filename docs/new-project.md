# Starting a new Labs project

A checklist for turning a copy of `labs-template` into a Liminal HQ Labs project. The [Labs charter](https://github.com/liminal-hq-labs/.github/blob/main/docs/labs-charter.md) explains the why. Replace `<name>` with the repository name throughout.

## 1. Create the repository from the template

- [ ] Create it, public from day one:

	```sh
	gh repo create liminal-hq-labs/<name> --public --template liminal-hq-labs/labs-template --description "<one line>"
	gh repo clone liminal-hq-labs/<name>
	```

## 2. Replace the placeholders

- [ ] On a topic branch, run the init script. The description is one line with no trailing full stop. Pass `--title` when the display name is not just the name with capitals.

	```sh
	git checkout -b chore/init-project
	scripts/init-project.sh <name> "<one-line description>"
	```

- [ ] Review the diff. The script fills `{{PROJECT_NAME}}`, `{{PROJECT_TITLE}}` and `{{PROJECT_DESCRIPTION}}` in the README, `SPEC.md`, `AGENTS.md`, `CLAUDE.md`, `docs/decisions.md`, the hero, the issue templates, `rustfmt.toml` and the workflow, and removes the template note from the README.
- [ ] Delete `scripts/init-project.sh`; it has done its job.
- [ ] Remove what the project does not need: `rustfmt.toml` for a project without Rust, the commented Rust jobs in `.github/workflows/ci.yml` for one without Rust.
- [ ] Commit with explicit paths and merge with a merge commit.

## 3. Labels

- [ ] Copy the house labels from Mailslot, then delete or reword the Mailslot-specific ones (`cli`, `ipc`, `protocol`, `sync`, `upnp`, `web-ui` and similar):

	```sh
	gh label clone liminal-hq-labs/mailslot -R liminal-hq-labs/<name>
	```

## 4. Topics

- [ ] Always `liminal-hq-labs` and `experimental`, plus the language, toolkit and domain:

	```sh
	gh repo edit liminal-hq-labs/<name> --add-topic liminal-hq-labs,experimental,<language>,<domain>
	```

## 5. Repository settings

- [ ] Merge commits only, branches deleted on merge, issues and projects on, discussions and wiki off:

	```sh
	gh repo edit liminal-hq-labs/<name> \
		--enable-merge-commit --enable-squash-merge=false --enable-rebase-merge=false \
		--delete-branch-on-merge \
		--enable-issues --enable-projects --enable-discussions=false --enable-wiki=false
	```

- [ ] Workflows get a read-only token by default:

	```sh
	gh api -X PUT repos/liminal-hq-labs/<name>/actions/permissions/workflow -f default_workflow_permissions=read
	```

- [ ] Secret scanning and push protection are on by default for public repositories. Confirm both under Settings → Code security, or turn them on:

	```sh
	gh repo edit liminal-hq-labs/<name> --enable-secret-scanning --enable-secret-scanning-push-protection
	```

## 6. Dependabot (opt-in)

Dependabot is off by default: the template ships no `.github/dependabot.yml`. To get weekly update pull requests, add one with GitHub Actions and the project's own ecosystem (`cargo`, `npm`, `pip`, `docker` and so on), and record the choice in `docs/decisions.md`:

```yaml
# Weekly dependency updates for GitHub Actions and Cargo.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

version: 2
updates:
  - package-ecosystem: 'github-actions'
    directory: '/'
    schedule:
      interval: 'weekly'
    open-pull-requests-limit: 5
    labels:
      - 'ci'

  - package-ecosystem: 'cargo'
    directory: '/'
    schedule:
      interval: 'weekly'
    open-pull-requests-limit: 5
    labels:
      - 'build'
```

## 7. README and hero

- [ ] Write the pitch, "What it's for" and "Install and run", and delete the guidance comments.
- [ ] Keep `> **Status:** experimental.` and say plainly what works today.
- [ ] Give `assets/hero.svg` a scene of its own when the project has one. The comment at the top of the file lists the rules. Check it and look at the frames:

	```sh
	bash scripts/check-banner.sh --shots /tmp/shots
	```

## 8. Checks

- [ ] Run the checks in `AGENTS.md` (headers, banner, markdownlint, shellcheck, actionlint) and fix anything they report.
- [ ] Add the project's own build, lint and test jobs to `.github/workflows/ci.yml`, one job per check.

## 9. Announce it

- [ ] Add the project under "What we're trying" in the [Labs profile](https://github.com/liminal-hq-labs/.github/blob/main/profile/README.md).
