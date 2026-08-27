# kaiten

[![CI](https://github.com/dsociative/kaiten-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/dsociative/kaiten-cli/actions/workflows/ci.yml)
[![Security](https://github.com/dsociative/kaiten-cli/actions/workflows/security.yml/badge.svg)](https://github.com/dsociative/kaiten-cli/actions/workflows/security.yml)
[![CodeQL](https://github.com/dsociative/kaiten-cli/actions/workflows/codeql.yml/badge.svg)](https://github.com/dsociative/kaiten-cli/actions/workflows/codeql.yml)
[![Coverage](https://codecov.io/gh/dsociative/kaiten-cli/graph/badge.svg)](https://codecov.io/gh/dsociative/kaiten-cli)
[![Release](https://img.shields.io/github/v/release/dsociative/kaiten-cli)](https://github.com/dsociative/kaiten-cli/releases)
[![Crates.io](https://img.shields.io/crates/v/kaiten-cli.svg)](https://crates.io/crates/kaiten-cli)
[![Downloads](https://img.shields.io/crates/d/kaiten-cli.svg)](https://crates.io/crates/kaiten-cli)
[![License](https://img.shields.io/crates/l/kaiten-cli.svg)](https://github.com/dsociative/kaiten-cli#license)

Command-line client and MCP server for the [Kaiten](https://kaiten.ru) tracker,
in the spirit of `gh` / `glab`.

- Browse spaces, boards and cards from the terminal
- Create, edit, move and archive cards; manage members, tags, comments, checklists and external links
- `--json` output on every command for scripting
- Built-in MCP server (`kaiten mcp serve`) so coding agents can work with the tracker
- Raw API escape hatch: `kaiten api GET /users/current`

## Install

From crates.io:

```sh
cargo install kaiten-cli
```

Prebuilt binaries for Linux (x86_64/aarch64), macOS (x86_64/aarch64) and Windows
are attached to [GitHub Releases](https://github.com/dsociative/kaiten-cli/releases).

From source:

```sh
git clone https://github.com/dsociative/kaiten-cli
cd kaiten-cli
cargo install --path crates/kaiten
```

## Authentication

Create an API token in your Kaiten profile (`https://mycompany.kaiten.ru` →
user profile → API tokens), then:

```sh
kaiten auth login    # asks for the domain ("mycompany") and token, verifies them
kaiten auth status   # shows domain, current user, where the URL and token came from
```

On-premise installations have no `*.kaiten.ru` domain — log in with the full API
base URL instead (everything else, MCP included, then works unchanged):

```sh
kaiten auth login --base-url https://kaiten.corp.local/api/latest
```

Environment variables override the config file:

| Variable | Meaning |
|---|---|
| `KAITEN_TOKEN` | API token |
| `KAITEN_DOMAIN` | company domain: `mycompany` → `https://mycompany.kaiten.ru/api/latest` |
| `KAITEN_BASE_URL` | full API base URL (overrides the domain) |
| `KAITEN_CONFIG_DIR` | config directory (default: `~/.config/kaiten`) |

## Configuration

`~/.config/kaiten/config.toml` — created by `kaiten auth login` with mode 600:

```toml
domain = "mycompany"
# base_url = "https://kaiten.corp.local/api/latest"   # on-premise: instead of domain
token = "your-api-token"

[defaults]      # optional: used when --space/--board flags are omitted
space = 123
board = 456
```

`kaiten card list` (and the `list_cards` MCP tool) only return non-archived cards
unless you pass `--archived`, which flips the filter to archived-only cards.

## Usage

```sh
kaiten space list
kaiten board list --space 123
kaiten board view 456                    # columns and lanes (ids for `card move`)

kaiten card list --mine
kaiten card list --board 456 --query "deploy" --limit 20
kaiten card view 67089469 --include comments   # a full card URL works too; external links are always shown
kaiten card create --board 456 --title "Fix the flaky test" --description "..."
kaiten card edit 67089469 --title "New title" --asap true
kaiten card move 67089469 --column 6308511
kaiten card archive 67089469

kaiten card member add 67089469 user@example.com   # user id or email
kaiten card member responsible 67089469 user@example.com
kaiten card comment add 67089469 --body "Done, please review"
kaiten card external-link add 67089469 --url https://example.com/spec --description "Spec"
kaiten card external-link list 67089469          # also edit / rm
kaiten card checklist add 67089469 --name "Release steps"
kaiten card checklist item add 67089469 91011 --text "Bump version"
kaiten card checklist item check 67089469 91011 121314
kaiten card tag add 67089469 backend

kaiten card link 67089469 --blocked-by 67089500 --reason "waiting for API"
kaiten card file add 67089469 ./screenshot.png    # uploads get a PUBLIC url
kaiten card file list 67089469
kaiten card file get 67089469 62769658 -o ./downloads/   # id or uid; into an existing dir or a file path; --force to overwrite
kaiten card time add 67089469 --minutes 30 --date 2026-07-16
kaiten card list --mine --state in-progress --sort updated --desc

kaiten tag list
kaiten card-type list

kaiten api GET "/cards?query=deploy&limit=5"                    # raw API access
kaiten api POST /cards --data '{"board_id":456,"title":"Raw"}'
```

Add `--json` to any command to print the raw JSON of the API response.

`card view` shows the card's external links (they come with the card);
`--include comments` fetches the comments too (one extra request; the name matches
the MCP `get_card` `include` value; `external_links` is accepted for compatibility).
`card view --comments` still works but is deprecated — use `--include comments`.

## Shell completion

```sh
# zsh — add to ~/.zshrc (needs compinit enabled, as in most setups)
eval "$(kaiten completion zsh)"

# bash — add to ~/.bashrc
eval "$(kaiten completion bash)"

# fish — run once
kaiten completion fish > ~/.config/fish/completions/kaiten.fish
```

## MCP server

The same binary is an MCP server (stdio transport, 40 tools mirroring the CLI,
including compact card projections and a cursor-based `poll_updates` for
event-like agent workflows). `get_card` returns the card's external links and
takes an optional `include: ["comments"]` to add the comments in the same call
(one extra request; `"external_links"` is accepted for compatibility).

Claude Code:

```sh
claude mcp add kaiten -- kaiten mcp serve
```

Any other MCP client:

```json
{
  "mcpServers": {
    "kaiten": {
      "command": "kaiten",
      "args": ["mcp", "serve"]
    }
  }
}
```

Authentication is shared with the CLI: run `kaiten auth login` once, or export
`KAITEN_DOMAIN` / `KAITEN_TOKEN` (or `KAITEN_BASE_URL` for an on-premise
installation) in the client configuration. Logs go to stderr
only — stdout carries the MCP protocol.

## API coverage

What this project covers of the [Kaiten API](https://developers.kaiten.ru/),
by area (✅ covered, ◐ partial, — not covered):

| Kaiten API area | CLI | MCP server |
|---|---|---|
| Auth, current user | ✅ | ✅ |
| Spaces | ◐ list | ◐ list |
| Boards, columns, lanes | ◐ read-only (`board list/view`) | ◐ read-only |
| Cards: create / list / view / edit / move / archive | ✅ | ✅ |
| Cards: delete | ✅ (with confirmation) | — deliberately: irreversible |
| Cards: batch update, history | — | — |
| Card list filters | ✅ space/board/column/member/mine/query/tag/type/archived/state/dates/sort/offset | ✅ same + lane/owner |
| Members: add / remove / set responsible | ✅ (by id or email) | ✅ (by id; `list_users` resolves) |
| Comments: list / add / edit / delete | ✅ | ✅ |
| Checklists: create, add items, check | ✅ | ✅ |
| Tags on cards, tag list | ✅ | ✅ |
| Card types | ◐ list | ◐ list |
| Users list (id lookup) | ✅ | ✅ |
| Card links: children / parents / blockers | ✅ `card link/unlink/unblock` | ✅ `link_cards` etc. |
| Files: attach / detach / list / download | ✅ (uploads get a PUBLIC url!) | ✅ (`download_file` saves locally) |
| External links (Links (common links)): list / add / edit / remove | ✅ `card external-link`, shown by `card view` | ✅ four tools, part of `get_card` |
| Custom properties: reference + set values | ✅ `property list/values`, `--properties-json` | ✅ two tools + `properties` (a JSON object — a wrong shape is rejected before the API call; mutations echo the resulting `properties`) |
| Time logs | ✅ `card time add/list` | ✅ |
| Events: polling for changes | — | ✅ `poll_updates` (cursor-based) |
| Events: webhooks | — deliberately (needs a public URL) | — |
| Sprints, SLA, location history | — | — |
| Raw API escape hatch | ✅ `kaiten api` | — |

Not covered and currently out of scope: administration (space/board CRUD,
roles, groups, automations), service desk, documents, custom directories.
Anything missing from the typed commands is reachable via `kaiten api`.

## Debugging

- `-v` — debug logs to stderr: every HTTP request with method, path, status, duration
- `-vv` — trace logs including request/response bodies (the token is always redacted)
- `RUST_LOG=kaiten_client=trace kaiten ...` — fine-grained filtering without flags
- decode errors report the exact JSON path that failed to parse
- `kaiten api <METHOD> <path> [--data <json>]` — raw access when a typed command is not enough
- API error bodies are printed as-is together with the HTTP status

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE),
at your option.

## Versioning

`MAJOR.MINOR.PATCH`, one version for both crates. New functionality bumps
MINOR (`0.2` → `0.3`); fixes and dependency updates bump PATCH. Until 1.0 a
MINOR bump may also carry a breaking change — the release notes say so when it
does. Cargo treats `0.y` as the compatibility line, so a `"0.3"` requirement
does not pick up `0.4.0` on its own.

`kaiten-client` types are `#[non_exhaustive]` (response models and
`KaitenError`; request types are built with `CreateCard::new` /
`UpdateCard::default` / `CardFilter::default`), so following the API by adding
fields is a compatible change. CI runs `cargo semver-checks` on every pull
request against the PR's base commit: a PR must fit a MINOR release unless it
carries the `breaking` label, which declares an intentional break.
