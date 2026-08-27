# Working on kaiten-cli

Facts and rules that are not derivable from the code. Keep it short; when a
rule changes, change it here in the same PR.

## Layout

- `crates/kaiten-client` — the published library (`kaiten-client` on crates.io): HTTP client, models, per-resource facades (`client.cards()`, `client.files()`, …).
- `crates/kaiten` — the `kaiten` binary (`kaiten-cli` on crates.io): the CLI and the MCP stdio server (`kaiten mcp serve`, rmcp). Both share `config::resolve()` and the same auth.
- One workspace version for both crates (`Cargo.toml` `[workspace.package]` + the `kaiten-client` pin in `crates/kaiten/Cargo.toml`).

## Toolchain and gates

- Build with the stable toolchain. If the default toolchain is nightly without cargo, use `cargo +stable …` (or `RUSTUP_TOOLCHAIN=stable` for tools that shell out to cargo).
- Before every commit (not only before a push): `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` (clippy pedantic is on), `cargo test --workspace`. A failing gate blocks the commit — never commit or push code that does not compile or has red tests, even on a feature branch; if a script drives the edit, check its exit status before `git commit`.
- `kaiten-client` changes: `cargo semver-checks check-release -p kaiten-client --baseline-rev v<last tag>` (`cargo install cargo-semver-checks --locked`). CI runs it on every pull request against the PR's base commit; the check against the last release is the local one before tagging.
- Bugs are fixed test-first: write the failing test that reproduces the bug, watch it fail, then fix. New behaviour likewise gets its tests before the implementation.
- Get an independent review of the whole diff before asking the owner to merge; fix Important findings, note Minor ones.
- Commits, pushes, merges and releases happen only when the owner explicitly asks. No `Co-Authored-By` trailers. Commit messages follow `type(scope): summary` (`feat`, `fix`, `test`, `docs`, `ci`, `chore`; `!` for breaking).

## Compatibility and versioning

- `kaiten-client` response models and `KaitenError` are `#[non_exhaustive]`; request types are built with `CreateCard::new(board_id, title)`, `UpdateCard::default()`, `CardFilter::default()` and field assignment. Adding fields and variants is therefore a MINOR change; removals, renames and type changes are MAJOR.
- **Two surfaces, two rules.** The `kaiten` binary — CLI subcommands, flags, positional arguments, *accepted flag values*, exit codes, `--json` shapes, `config.toml` keys, env vars, MCP tool names, params, *accepted param values* and result keys — is a contract that is only ever extended. Nothing that is accepted today stops being accepted, and nothing changes meaning; a form that has become redundant stays accepted as a no-op (e.g. `--include external_links` after links moved into the card response). Supersede with a deprecation (help annotation + one stderr line) and keep the old form working indefinitely; removal happens only in a CLI major release decided by the owner. A breaking release of `kaiten-client` does **not** license any CLI/MCP/config change. Human-readable text (tables, messages) is not a contract, its meaning is. When unsure whether something is a contract, treat it as one and ask the owner.
- Versions: new functionality → MINOR (`0.5` → `0.6`), fixes and dependency bumps → PATCH. A breaking change of `kaiten-client` ships in the next `0.y` and the PR carries the `breaking` label (CI then runs `cargo semver-checks --release-type major`; the label is read when the job runs, so re-run after adding it). `cargo semver-checks` does not report a removed derive (e.g. `Default`) — list such breaks in the PR yourself. Version numbers are decided at release time — do not write them into issues.
- Release: bump the workspace version and the `kaiten-client` pin, run the tests, commit `chore(release): X.Y.Z`, `cargo publish --dry-run -p kaiten-client`, tag `vX.Y.Z`, push master and the tag. `release.yml` builds the binaries and the GitHub release, `publish.yml` publishes both crates (Trusted Publishing). Yank a wrong version rather than re-using it.

## Tests

- Integration tests use wiremock; request bodies are pinned with `body_json`, request counts with `.expect(n)`; a test that must make no request asserts `received_requests()` is empty or `.expect(0)`.
- Fixtures are full, sanitized live captures of real API responses, not hand-trimmed subsets: emails → `<user>@example.com`, avatar data URLs → `null`, inbound email keys (`email_key`, `a+co-…@a.kaiten.ru`) zeroed, `share_id` → `null`. Hand overlays only for what the test account's tariff cannot produce (relations, blocking, custom properties); say so in the PR.
- The MCP tool set is pinned (`registers_exactly_N_tools_with_spec_names` in `crates/kaiten/src/mcp/mod.rs`, `EXPECTED_TOOLS` in `crates/kaiten/tests/mcp_stdio_test.rs`, the count in README). Tool-level failures are `Ok(CallToolResult::error(..))` (`try_api!` / `try_args!`), never JSON-RPC errors; params use `#[serde(deny_unknown_fields)]`.
- Insta snapshots: review the diff, then accept with `INSTA_UPDATE=always cargo test …`.

## Live smoke (test account)

- Live checks run against a Kaiten test account whose credentials live in a gitignored `.env.test` at the repo root (`KAITEN_DOMAIN`, `KAITEN_TOKEN`); ask the owner for them. Never commit or print them.
- Card 67089469 there is the **frozen fixture card** — the source of the card fixtures; do not add, remove or edit anything on it. Card 69316486 is the playground for mutating checks; clean up after yourself.
- The test tariff has no time logs, relations (children/parents), blocking or custom properties.
- Probe and debug with trace logging on — see **Debugging** below; never infer API behaviour from the `--json` output of typed commands.
- The owner's corporate tracker may be used read-only, and only when the owner says so.

## Debugging

- Always run with tracing when investigating anything that touches the API — it shows the whole exchange at once instead of guessing:
  - `kaiten -v …` → debug: method, path, status, elapsed, retries;
  - `kaiten -vv …` → trace: additionally the request body and the **full response body** of every call (stderr; redirect with `2>trace.log` when the output is processed further);
  - or `RUST_LOG=kaiten=trace,kaiten_client=trace,reqwest=debug kaiten …` when a flag cannot be passed (e.g. the MCP server launched by a client — its logs go to stderr, stdout carries the protocol).
- `kaiten api GET /path` prints the raw JSON of any endpoint. `--json` on typed commands prints the serialized *model* and silently drops every field the model does not know — it proved that `GET /cards` "does not return external_links" when it did. Read the raw body first, reason about models second.
- Tests: `cargo test … -- --nocapture` shows the CLI's stderr; wiremock's unmatched-request panic lists what was actually sent.

## Kaiten API facts worth knowing

- `GET /cards/{id}` embeds `external_links`, `files`, `members`, `tags`, `checklists`; `GET /cards` embeds `external_links` only for cards that have any (key absent otherwise) and `description` only with `additional_card_fields=description` (the only value it accepts). List cards do carry members/tags/checklists.
- External links: `GET/POST /cards/{id}/external-links`, `PATCH/DELETE …/{link_id}` (`PUT` → 404). The API accepts duplicate and invalid urls; the CLI/MCP only check that a url is absolute `http(s)://` before sending. A missing card or link answers 403, not 404.
- `properties` is `{}` for a card without custom properties (`null` for one that never had them).
- Files: the classic storage has numeric `id`, a separate `uid` and a public `https://files.kaiten.ru/<uuid>.ext` url served without auth; the newer storage (`type` 11) has a UUID string `id`, `uid: null`, a host-root-relative `/api/v1/cards/{card_uid}/files/{uuid}` url that needs the token and answers with metadata whose `url` is a short-lived signed storage link. The token is sent only to the API origin.
- Rate limit answers 429 with `Retry-After`; the client retries.

## CI

- `ci.yml`: `lint`, `test` (ubuntu, macos), `semver` (pull requests; see above), `coverage` (cargo-llvm-cov → Codecov; `CODECOV_TOKEN` is an Actions and a Dependabot secret), `build-windows`. `codeql.yml` uses `.github/codeql/codeql-config.yml`, which excludes `rust/cleartext-logging` — it models `println!` as a log file and identifiers like `user_id`/`username` as secrets, so every CLI line naming a user was a false positive. `security.yml` runs cargo-deny (advisories, licenses, bans). Dependabot opens weekly PRs for cargo and GitHub Actions.
- Release notes are generated from PR titles: write them for the user.

## Issues and pull requests

- A PR that closes an issue references it (`Closes #N`) and, when the issue is closed, leaves a short comment for the reporter: what shipped, in which release, how to use it, any deliberate deviation.
- Feature PRs update the README (usage examples, MCP tool count, the API coverage table) in the same PR.
- Deviations from a request (an endpoint that behaves differently, a feature the tariff cannot test) are stated in the PR description, not silently absorbed.
