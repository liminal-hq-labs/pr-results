# Publishing on Forgejo

Forgejo has no `workflow_run` trigger. A final job can download earlier jobs' result artefacts into a directory with one subdirectory per artefact and run `pr-results publish --from results --forge forgejo`. Run the job after all test jobs, including when tests fail, and keep it separate from the required CI status. Set `GITHUB_TOKEN` or `FORGEJO_TOKEN` to the job token; add no other secrets. Configure suites in `.forgejo/pr-results.toml`, or pass `--config` explicitly. The action can be pinned to a release commit and receives the same arguments through `args`.

This mode accepts same-repository pull requests and pushes. A fork pull request is skipped, and `pull_request_target` is refused. Forgejo ignores workflow `permissions:`, so review the runner's token access and trust the code in jobs that have it. The publisher verifies the pull request before commenting, and writes commit statuses in place of check runs. It has not been verified against a live Forgejo runner.

Build from source with `cargo install --git https://github.com/liminal-hq-labs/pr-results --locked pr-results` when a release binary does not fit the runner. In-job file collection requires Unix; Windows builds and paths have not been tested.
