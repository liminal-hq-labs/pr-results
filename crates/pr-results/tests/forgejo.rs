// `pr-results publish` for Forgejo: the kind, the API address, the token, the CA file and the bot login.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Forgejo has no `workflow_run` trigger, so the publish flow itself is tested at the library
//! level (`pr-results-forge/tests/forgejo.rs`). What the command line owns is everything decided
//! before the first request, and those refusals are exit status 2: a runner of the wrong kind, an
//! API address that is not the runner's own host or not `https`, a CA file that cannot be used, a
//! token that belongs to another account. Each is tested to make no request and to mask no token.

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::{fixture, invoke, publisher_env, scratch, Factory, REPO, SHA, TOKEN};
use pr_results_forge::{FakeForgejo, FakePull, HostKind};

fn pr_event() -> String {
	fixture("events/workflow_run-pr.json")
}

fn forgejo_world() -> FakeForgejo {
	let fake = FakeForgejo::new(REPO);
	fake.add_pull(FakePull::same_repo(7, REPO, SHA, "feature"));
	fake
}

/// The environment of a Forgejo runner that has been handed a `workflow_run` event: its own host
/// for the server and the API, and a token in `FORGEJO_TOKEN`.
fn runner_env(dir: &std::path::Path) -> BTreeMap<String, String> {
	let mut v = publisher_env(dir, &pr_event());
	v.insert("GITHUB_SERVER_URL".into(), "https://git.lan".into());
	v.insert("GITHUB_API_URL".into(), "https://git.lan/api/v1".into());
	v.insert("FORGEJO_SERVER_URL".into(), "https://git.lan".into());
	v.remove("GITHUB_TOKEN");
	v.insert("FORGEJO_TOKEN".into(), TOKEN.into());
	v
}

fn write_config(dir: &std::path::Path, name: &str, text: &str) -> String {
	let path = dir.join(name);
	std::fs::write(&path, text).unwrap();
	path.display().to_string()
}

fn refused(
	vars: &BTreeMap<String, String>,
	args: &[&str],
	fake: &FakeForgejo,
	why: &str,
) -> String {
	let factory = Factory::new(&fake.forge());
	let o = invoke(args, vars, &factory);
	assert_eq!(o.code, 2, "{why}: {}", o.err);
	assert!(fake.requests().is_empty(), "{why}: it made a request");
	assert!(
		factory.asked.borrow().is_empty(),
		"{why}: it built a client"
	);
	assert!(
		!o.out.contains("add-mask"),
		"{why}: it masked a token it never used"
	);
	o.err
}

const FORGEJO: &str = "[forge]\nkind = \"forgejo\"\n";

// ---- the kind ------------------------------------------------------------------------------

#[test]
fn a_forgejo_runner_on_the_github_kind_is_refused() {
	let dir = scratch("fj-kind");
	let fake = forgejo_world();
	let vars = runner_env(&dir);
	let err = refused(&vars, &["publish"], &fake, "default kind");
	assert!(err.contains("FORGEJO_SERVER_URL"), "{err}");

	let cfg = write_config(&dir, "github.toml", "[forge]\nkind = \"github\"\n");
	refused(
		&vars,
		&["publish", "--config", &cfg],
		&fake,
		"kind = github",
	);
	let cfg = write_config(&dir, "forgejo.toml", FORGEJO);
	refused(
		&vars,
		&["publish", "--config", &cfg, "--forge", "github"],
		&fake,
		"--forge github overrides the file",
	);
}

#[test]
fn the_forgejo_kind_on_github_com_is_refused() {
	let dir = scratch("fj-on-github");
	let fake = forgejo_world();
	let vars = publisher_env(&dir, &pr_event());
	let err = refused(
		&vars,
		&["publish", "--forge", "forgejo"],
		&fake,
		"github.com",
	);
	assert!(err.contains("github.com"), "{err}");
}

#[test]
fn the_flag_overrides_the_file_and_the_kind_reaches_the_factory() {
	let dir = scratch("fj-flag");
	let fake = forgejo_world();
	let vars = runner_env(&dir);
	let cfg = write_config(&dir, "github.toml", "[forge]\nkind = \"github\"\n");
	let factory = Factory::new(&fake.forge());
	let o = invoke(
		&["publish", "--config", &cfg, "--forge", "forgejo"],
		&vars,
		&factory,
	);
	// The flow itself stops at the artefact listing: Forgejo has none.
	assert_eq!(o.code, 1, "{}", o.err);
	assert!(o.err.contains("listing artefacts"), "{}", o.err);
	let setups = factory.setups.borrow();
	assert_eq!(setups.len(), 1);
	assert_eq!(setups[0].kind, HostKind::Forgejo);
	assert_eq!(setups[0].api_base, "https://git.lan/api/v1");
	assert!(fake.writes().is_empty());
}

// ---- the API address -------------------------------------------------------------------------

#[test]
fn an_api_url_on_another_host_is_refused_before_anything_is_sent() {
	let dir = scratch("fj-host");
	let fake = forgejo_world();
	let vars = runner_env(&dir);
	for (url, why) in [
		("https://evil.example/api/v1", "another host"),
		("https://git.lan.evil.example/api/v1", "a longer host"),
		("https://git.lan:8443/api/v1", "another port"),
		("https://GIT.LAN.evil.example/api/v1", "case does not help"),
	] {
		let cfg = write_config(
			&dir,
			"host.toml",
			&format!("[forge]\nkind = \"forgejo\"\napi_url = \"{url}\"\n"),
		);
		let err = refused(&vars, &["publish", "--config", &cfg], &fake, why);
		assert!(err.contains("runner's own host"), "{why}: {err}");
	}
}

#[test]
fn an_api_url_may_only_confirm_the_runners_own_host() {
	let dir = scratch("fj-host-ok");
	let fake = forgejo_world();
	let vars = runner_env(&dir);
	for url in [
		"https://git.lan/api/v1",
		"https://GIT.LAN/api/v1",
		"https://git.lan:443/git/api/v1",
	] {
		let cfg = write_config(
			&dir,
			"host.toml",
			&format!("[forge]\nkind = \"forgejo\"\napi_url = \"{url}\"\n"),
		);
		let factory = Factory::new(&fake.forge());
		let o = invoke(&["publish", "--config", &cfg], &vars, &factory);
		assert_eq!(o.code, 1, "{url}: {}", o.err);
		assert_eq!(factory.asked.borrow().as_slice(), [url]);
	}
}

#[test]
fn the_runners_api_variable_is_held_to_the_same_rule_and_to_https() {
	let dir = scratch("fj-env");
	let fake = forgejo_world();
	let cfg = write_config(&dir, "forgejo.toml", FORGEJO);
	for (value, why) in [
		(
			"https://elsewhere.example/api/v1",
			"GITHUB_API_URL on another host",
		),
		("http://git.lan/api/v1", "plain http"),
		("ftp://git.lan/api/v1", "not https"),
	] {
		let mut vars = runner_env(&dir);
		vars.insert("GITHUB_API_URL".into(), value.into());
		refused(&vars, &["publish", "--config", &cfg], &fake, why);
	}
	let mut vars = runner_env(&dir);
	vars.remove("GITHUB_API_URL");
	refused(
		&vars,
		&["publish", "--config", &cfg],
		&fake,
		"no API address at all",
	);

	// api_url supplies it when the runner has none.
	let cfg = write_config(
		&dir,
		"api.toml",
		"[forge]\nkind = \"forgejo\"\napi_url = \"https://git.lan/api/v1\"\n",
	);
	let o = invoke(
		&["publish", "--config", &cfg],
		&vars,
		&Factory::new(&fake.forge()),
	);
	assert_eq!(o.code, 1, "{}", o.err);
}

#[test]
fn plain_http_is_refused_in_the_file_with_its_line() {
	let dir = scratch("fj-http");
	let fake = forgejo_world();
	let vars = runner_env(&dir);
	let cfg = write_config(
		&dir,
		"http.toml",
		"[forge]\nkind = \"forgejo\"\napi_url = \"http://git.lan/api/v1\"\n",
	);
	let err = refused(&vars, &["publish", "--config", &cfg], &fake, "http api_url");
	assert!(
		err.contains("line 3") && err.contains("plain http"),
		"{err}"
	);
}

// ---- the token ---------------------------------------------------------------------------------

#[test]
fn the_token_is_github_token_then_forgejo_token_or_the_named_variable() {
	let dir = scratch("fj-token");
	let fake = forgejo_world();
	let cfg = write_config(&dir, "forgejo.toml", FORGEJO);

	// FORGEJO_TOKEN alone is enough for Forgejo.
	let vars = runner_env(&dir);
	let o = invoke(
		&["publish", "--config", &cfg],
		&vars,
		&Factory::new(&fake.forge()),
	);
	assert_eq!(o.code, 1, "{}", o.err);
	assert!(o.out.contains("::add-mask::ghs_testtoken"), "{}", o.out);

	// With neither, it is a usage error.
	let mut none = runner_env(&dir);
	none.remove("FORGEJO_TOKEN");
	refused(&none, &["publish", "--config", &cfg], &fake, "no token");

	// A named variable is used and the fallback is not.
	let mut named = runner_env(&dir);
	named.insert("MY_TOKEN".into(), "other-token-value".into());
	let o = invoke(
		&["publish", "--config", &cfg, "--token-env", "MY_TOKEN"],
		&named,
		&Factory::new(&fake.forge()),
	);
	assert!(o.out.contains("::add-mask::other-token-value"), "{}", o.out);
	assert!(!o.out.contains(TOKEN));
	let mut missing = runner_env(&dir);
	missing.insert("FORGEJO_TOKEN".into(), TOKEN.into());
	refused(
		&missing,
		&["publish", "--config", &cfg, "--token-env", "MY_TOKEN"],
		&fake,
		"a named variable is not replaced by FORGEJO_TOKEN",
	);
}

#[test]
fn a_forgejo_token_is_never_used_on_github() {
	let dir = scratch("fj-token-github");
	let fake = forgejo_world();
	let mut vars = publisher_env(&dir, &pr_event());
	vars.remove("GITHUB_TOKEN");
	vars.insert("FORGEJO_TOKEN".into(), TOKEN.into());
	refused(
		&vars,
		&["publish"],
		&fake,
		"FORGEJO_TOKEN on the github kind",
	);
}

// ---- the CA file --------------------------------------------------------------------------------

fn tls_fixture(name: &str) -> Vec<u8> {
	std::fs::read(
		PathBuf::from(env!("CARGO_MANIFEST_DIR"))
			.join("../pr-results-forge/tests/fixtures/tls")
			.join(name),
	)
	.unwrap()
}

fn with_ca(dir: &std::path::Path, ca: &std::path::Path) -> String {
	write_config(
		dir,
		"ca.toml",
		&format!(
			"[forge]\nkind = \"forgejo\"\nca_file = \"{}\"\n",
			ca.display()
		),
	)
}

#[test]
fn a_good_ca_file_reaches_the_factory() {
	let dir = scratch("fj-ca");
	let fake = forgejo_world();
	let vars = runner_env(&dir);
	let ca = dir.join("lan-ca.pem");
	std::fs::write(&ca, tls_fixture("test-ca.pem")).unwrap();
	let cfg = with_ca(&dir, &ca);
	let factory = Factory::new(&fake.forge());
	let o = invoke(&["publish", "--config", &cfg], &vars, &factory);
	assert_eq!(o.code, 1, "{}", o.err);
	let setups = factory.setups.borrow();
	assert_eq!(setups[0].ca.as_ref().map(|c| c.len()), Some(1));
}

#[test]
fn a_ca_file_that_cannot_be_used_is_refused_with_exit_two() {
	let dir = scratch("fj-ca-bad");
	let fake = forgejo_world();
	let vars = runner_env(&dir);

	let missing = dir.join("missing.pem");
	let err = refused(
		&vars,
		&["publish", "--config", &with_ca(&dir, &missing)],
		&fake,
		"missing",
	);
	assert!(err.contains("cannot be opened"), "{err}");

	let mut files: Vec<(&str, Vec<u8>, &str)> = vec![
		("empty", Vec::new(), "no PEM certificate"),
		(
			"garbage",
			b"not a certificate at all\n".to_vec(),
			"no PEM certificate",
		),
		("binary", vec![0xff, 0x00, 0x13, 0x37], "no PEM certificate"),
		(
			"damaged",
			b"-----BEGIN CERTIFICATE-----\n!!!!\n-----END CERTIFICATE-----\n".to_vec(),
			"damaged",
		),
		("key", tls_fixture("leaf.key"), "private key"),
	];
	let mut big = tls_fixture("test-ca.pem");
	big.resize(2 * 1024 * 1024, b'\n');
	files.push(("2 MiB", big, "larger than 1 MiB"));
	for (name, bytes, why) in files {
		let ca = dir.join("ca.pem");
		std::fs::write(&ca, &bytes).unwrap();
		let err = refused(
			&vars,
			&["publish", "--config", &with_ca(&dir, &ca)],
			&fake,
			name,
		);
		assert!(err.contains(why), "{name}: {err}");
		assert!(err.contains("ca_file"), "{name}: {err}");
	}
}

// ---- the bot login -----------------------------------------------------------------------------

#[test]
fn a_token_that_belongs_to_another_account_than_the_bot_login_is_exit_two() {
	let dir = scratch("fj-bot");
	let fake = forgejo_world();
	fake.set_token_user("someone-else", 12);
	let vars = runner_env(&dir);
	let cfg = write_config(&dir, "forgejo.toml", FORGEJO);
	let o = invoke(
		&["publish", "--config", &cfg, "--bot-login", "ci-bot"],
		&vars,
		&Factory::new(&fake.forge()),
	);
	assert_eq!(o.code, 2, "{}", o.err);
	assert!(o.err.contains("another account"), "{}", o.err);
	// The one request is the read that found out; nothing was written.
	let all: Vec<String> = fake.requests().iter().map(|r| r.target()).collect();
	assert_eq!(all, ["/user"]);
	assert!(fake.writes().is_empty());
}

#[test]
fn the_bot_login_of_the_file_is_used_and_the_flag_overrides_it() {
	let dir = scratch("fj-bot-file");
	let fake = forgejo_world();
	fake.set_token_user("ci-bot", 7);
	let vars = runner_env(&dir);
	let cfg = write_config(
		&dir,
		"bot.toml",
		"[forge]\nkind = \"forgejo\"\nbot_login = \"ci-bot\"\n",
	);
	let o = invoke(
		&["publish", "--config", &cfg],
		&vars,
		&Factory::new(&fake.forge()),
	);
	// Past the account check, and stopped at the artefact listing as ever.
	assert_eq!(o.code, 1, "{}", o.err);
	assert!(fake.requests().iter().any(|r| r.target() == "/user"));

	let o = invoke(
		&["publish", "--config", &cfg, "--bot-login", "someone-else"],
		&vars,
		&Factory::new(&fake.forge()),
	);
	assert_eq!(o.code, 2, "{}", o.err);

	let o = invoke(
		&["publish", "--config", &cfg, "--bot-login", "not a login"],
		&vars,
		&Factory::new(&fake.forge()),
	);
	assert_eq!(o.code, 2, "{}", o.err);
}

// ---- blob hosts --------------------------------------------------------------------------------

#[test]
fn blob_hosts_are_for_github_and_refused_for_forgejo() {
	let dir = scratch("fj-blob");
	let fake = forgejo_world();
	let vars = runner_env(&dir);
	let cfg = write_config(
		&dir,
		"blob.toml",
		"[forge]\nblob_hosts = [\"storage.git.lan\"]\n",
	);
	let err = refused(
		&vars,
		&["publish", "--config", &cfg, "--forge", "forgejo"],
		&fake,
		"forgejo",
	);
	assert!(err.contains("blob_hosts"), "{err}");

	// On GitHub they reach the factory, lower-cased.
	let mut gh = publisher_env(&dir, &pr_event());
	gh.insert(
		"GITHUB_API_URL".into(),
		"https://ghes.example/api/v3".into(),
	);
	gh.insert("GITHUB_SERVER_URL".into(), "https://ghes.example".into());
	let cfg = write_config(
		&dir,
		"ghes.toml",
		"[forge]\nblob_hosts = [\"Storage.Example.com\"]\napi_url = \"https://ghes.example/api/v3\"\n",
	);
	let github = common::github_with_pr();
	let factory = Factory::new(&github);
	let o = invoke(&["publish", "--config", &cfg], &gh, &factory);
	assert_eq!(o.code, 0, "{}", o.err);
	let setups = factory.setups.borrow();
	assert_eq!(setups[0].kind, HostKind::GitHub);
	assert_eq!(
		setups[0].blob_hosts.as_deref(),
		Some(["storage.example.com".to_owned()].as_slice())
	);
}

#[test]
fn on_github_an_api_url_must_be_on_the_servers_host_too() {
	let dir = scratch("fj-ghes-host");
	let github = common::github_with_pr();
	let vars = publisher_env(&dir, &pr_event());
	// github.com's API host is api.github.com, so the setting cannot confirm it: it is refused
	// rather than allowed to name another host.
	for url in ["https://api.github.com", "https://evil.example/api/v3"] {
		let cfg = write_config(&dir, "api.toml", &format!("[forge]\napi_url = \"{url}\"\n"));
		let factory = Factory::new(&github);
		let o = invoke(&["publish", "--config", &cfg], &vars, &factory);
		assert_eq!(o.code, 2, "{url}: {}", o.err);
		assert!(github.requests().is_empty());
		assert!(factory.asked.borrow().is_empty());
	}
}

#[test]
fn forgejo_in_job_requires_from() {
	let dir = scratch("fj-event");
	let fake = forgejo_world();
	let mut vars = runner_env(&dir);
	vars.insert("GITHUB_EVENT_NAME".into(), "pull_request".into());
	let cfg = write_config(&dir, "forgejo.toml", FORGEJO);
	let err = refused(&vars, &["publish", "--config", &cfg], &fake, "pull_request");
	assert!(err.contains("--from"), "{err}");
}

#[test]
fn in_job_publishes_local_results_to_forgejo() {
	let dir = scratch("fj-in-job");
	let mut vars = runner_env(&dir);
	let event = format!(
		r#"{{"repository":{{"full_name":"{REPO}"}},"number":7,"pull_request":{{"number":7,"head":{{"sha":"{SHA}","ref":"feature","repo":{{"full_name":"{REPO}"}}}},"base":{{"repo":{{"full_name":"{REPO}"}}}}}}}}"#
	);
	std::fs::write(dir.join("event.json"), event).unwrap();
	vars.insert("GITHUB_EVENT_NAME".into(), "pull_request".into());
	vars.insert("GITHUB_RUN_ID".into(), "5000".into());
	vars.insert("GITHUB_RUN_NUMBER".into(), "42".into());
	let input = dir.join("results/rust-test-results");
	std::fs::create_dir_all(&input).unwrap();
	std::fs::write(input.join("junit.xml"), fixture("nextest/fail.xml")).unwrap();
	let fake = forgejo_world();
	let out = invoke(
		&[
			"publish",
			"--forge",
			"forgejo",
			"--from",
			dir.join("results").to_str().unwrap(),
		],
		&vars,
		&Factory::new(&fake.forge()),
	);
	assert_eq!(out.code, 0, "{}", out.err);
	assert!(out.out.contains("Commit status written"), "{}", out.out);
	assert!(out.out.contains("Comment on pull request 7"), "{}", out.out);
}

#[test]
fn http_requires_opt_in_and_stays_on_the_runner_host() {
	let dir = scratch("fj-http-opt-in");
	let fake = forgejo_world();
	let mut vars = runner_env(&dir);
	vars.insert("GITHUB_SERVER_URL".into(), "http://git.lan".into());
	vars.insert("GITHUB_API_URL".into(), "http://git.lan/api/v1".into());
	let no = write_config(&dir, "no.toml", FORGEJO);
	refused(
		&vars,
		&["publish", "--config", &no],
		&fake,
		"HTTP without opt-in",
	);
	let yes = write_config(
		&dir,
		"yes.toml",
		"[forge]\nkind = \"forgejo\"\nallow_http = true\n",
	);
	let factory = Factory::new(&fake.forge());
	let result = invoke(&["publish", "--config", &yes], &vars, &factory);
	assert_eq!(result.code, 1, "{}", result.err);
	assert!(factory.setups.borrow()[0].allow_http);
	vars.insert("GITHUB_API_URL".into(), "http://other.lan/api/v1".into());
	refused(
		&vars,
		&["publish", "--config", &yes],
		&fake,
		"another HTTP host",
	);
}

#[test]
fn http_cannot_downgrade_an_https_runner() {
	let dir = scratch("fj-http-downgrade");
	let fake = forgejo_world();
	let mut vars = runner_env(&dir);
	vars.insert("GITHUB_API_URL".into(), "http://git.lan/api/v1".into());
	let cfg = write_config(
		&dir,
		"yes.toml",
		"[forge]\nkind = \"forgejo\"\nallow_http = true\n",
	);
	refused(
		&vars,
		&["publish", "--config", &cfg],
		&fake,
		"HTTPS downgrade",
	);
}

#[test]
fn http_opt_in_does_not_change_the_api_port_or_scheme() {
	let dir = scratch("fj-http-authority");
	let cfg = write_config(
		&dir,
		"yes.toml",
		"[forge]\nkind = \"forgejo\"\nallow_http = true\n",
	);
	let mut vars = runner_env(&dir);
	vars.insert("GITHUB_SERVER_URL".into(), "http://git.lan".into());
	for api in ["http://git.lan:443/api/v1", "https://git.lan/api/v1"] {
		vars.insert("GITHUB_API_URL".into(), api.into());
		refused(&vars, &["publish", "--config", &cfg], &forgejo_world(), api);
	}
}
