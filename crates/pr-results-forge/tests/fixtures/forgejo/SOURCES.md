# Sources of the Forgejo fixtures

Every file here is one response of a route the Forgejo host uses. They were built, not captured: nothing ran against a live Forgejo. Each takes its shape from Forgejo's own definitions, and its values are made up (`forgejo.example.invalid`, `alice`, `bob`, `ci-bot`, ids and times that mean nothing).

- **Shape.** Every property of the Swagger definition is present, with a value of its type: <https://code.forgejo.org/swagger.v1.json> (version 17.0.0-26). Properties the Go struct writes as pointers without `omitempty` are `null` where nothing is set (`milestone`, `assignee`, `merged_by`, `merged_at`, `repo_transfer`, `parent`), and the ones it marks `omitempty` are left out when empty (`external_tracker`, `external_wiki`). The structs are in `modules/structs/pull.go`, `repo.go` and `issue_comment.go` of the v17.0.1 source, <https://code.forgejo.org/forgejo/forgejo/src/tag/v17.0.1>.
- **Behaviour.** What each route does is read from the same tag, as `SRC/` below: `routers/api/v1/repo/status.go`, `pull.go`, `commits.go` and `issue_comment.go`, `services/repository/files/tree.go` and `models/user/user_system.go`.

| Fixture | Route | Definition | Source of the behaviour |
| --- | --- | --- | --- |
| `user.json` | `GET /user` | `User` | Swagger `userGetCurrent`. A bot account's personal token; the job token's account is `forgejo-actions` with id -2 (`models/user/user_system.go`, `ActionsUserID`) |
| `pull.json` | `GET /repos/{o}/{r}/pulls/{index}` | `PullRequest` | Swagger `repoGetPullRequest` |
| `pulls-list.json` | `GET /repos/{o}/{r}/pulls?state=open&head=<branch>` | `PullRequest` list | `ListPullRequests` in `pull.go`: `head` filters by the branch name alone, `Link` and `X-Total-Count` are set |
| `commit-pull.json` | `GET /repos/{o}/{r}/commits/{sha}/pull` | `PullRequest` | `GetCommitPullRequest` in `commits.go`: it looks the commit up with `GetPullRequestByMergedCommit`, so only a *merged* commit has one, and the pull request is closed |
| `tree-page.json` | `GET /repos/{o}/{r}/git/trees/{sha}?recursive=true&per_page=1000&page=N` | `GitTreeResponse` | `GetTreeBySHA` in `tree.go`: `truncated` is true on every page of a tree longer than a page (the last included), a page past the end is empty, and `total_count` is the length of the whole tree |
| `comments.json` | `GET /repos/{o}/{r}/issues/{index}/comments` | `Comment` list | `ListIssueComments` in `issue_comment.go`: no `page` or `limit`, every comment, `X-Total-Count` set, no `Link` |
| `comment-created.json` | `POST /repos/{o}/{r}/issues/{index}/comments` | `Comment` | Swagger `issueCreateComment`, status 201 |
| `comment-patched.json` | `PATCH /repos/{o}/{r}/issues/comments/{id}` | `Comment` | Swagger `issueEditComment`, status 200 |
| `status-created.json` | `POST /repos/{o}/{r}/statuses/{sha}` | `CommitStatus` | `NewCommitStatus` in `status.go`, status 201. The body sent is `CreateStatusOption`: `state`, `context`, `description`, `target_url` |
| `statuses-list.json` | `GET /repos/{o}/{r}/commits/{ref}/statuses` | `CommitStatus` list | `GetCommitStatusesByRef` in `status.go`: `page` and `limit`, `Link` and `X-Total-Count` set. Its status field is `status`, where the body sent says `state` |

`DELETE /repos/{o}/{r}/issues/comments/{id}` answers 204 with no body, so it has no fixture.

The files are generated from the Swagger and indented with tabs. Regenerate one by walking its definition rather than editing it by hand, so a property cannot be forgotten.
