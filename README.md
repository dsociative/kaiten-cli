# kaiten

Command-line client and MCP server for the [Kaiten](https://kaiten.ru) tracker,
in the spirit of `gh` / `glab`.

- Browse spaces, boards and cards from the terminal
- Create, edit, move and archive cards; manage members, tags, comments and checklists
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
kaiten auth status   # shows domain, current user and where the token came from
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
kaiten card view 67089469 --comments     # a full card URL works too
kaiten card create --board 456 --title "Fix the flaky test" --description "..."
kaiten card edit 67089469 --title "New title" --asap true
kaiten card move 67089469 --column 6308511
kaiten card archive 67089469

kaiten card member add 67089469 user@example.com   # user id or email
kaiten card comment add 67089469 --body "Done, please review"
kaiten card checklist add 67089469 --name "Release steps"
kaiten card checklist item add 67089469 91011 --text "Bump version"
kaiten card checklist item check 67089469 91011 121314
kaiten card tag add 67089469 backend

kaiten tag list
kaiten card-type list

kaiten api GET "/cards?query=deploy&limit=5"                    # raw API access
kaiten api POST /cards --data '{"board_id":456,"title":"Raw"}'
```

Add `--json` to any command to print the raw JSON of the API response.

## Shell completion

### zsh

Simplest — one line in `~/.zshrc` (requires `compinit` to be enabled, as in
most zsh setups):

```sh
eval "$(kaiten completion zsh)"
```

Or install a static completion file (faster shell startup):

```sh
mkdir -p ~/.zfunc
kaiten completion zsh > ~/.zfunc/_kaiten
```

then make sure `~/.zshrc` adds the directory to `fpath` **before** `compinit`:

```sh
fpath+=(~/.zfunc)
autoload -U compinit && compinit
```

Homebrew users can drop the file into
`$(brew --prefix)/share/zsh/site-functions/_kaiten` instead — that directory
is already in `fpath`.

After installing or updating the file, rebuild the completion cache once and
open a new shell:

```sh
rm -f ~/.zcompdump*
```

Avoid `kaiten completion zsh > "${fpath[1]}/_kaiten"`: `fpath` order inside a
running shell usually differs from a fresh one, so the file lands in a
directory new shells never look at.

### bash

Requires the `bash-completion` (v2) package:

```sh
mkdir -p ~/.local/share/bash-completion/completions
kaiten completion bash > ~/.local/share/bash-completion/completions/kaiten
```

### fish

```sh
kaiten completion fish > ~/.config/fish/completions/kaiten.fish
```

## MCP server

The same binary is an MCP server (stdio transport, 16 tools mirroring the CLI).

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
`KAITEN_DOMAIN` / `KAITEN_TOKEN` in the client configuration. Logs go to stderr
only — stdout carries the MCP protocol.

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
