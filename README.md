# gitignore (Rust)

Fetch and compose GitHub's `.gitignore` templates from your terminal. When no type is provided, a fuzzy picker helps you choose; when you pass a type, it downloads immediately.

[List of available templates](https://github.com/github/gitignore)

## Install

```bash
cargo install --path .
```

## Usage

```bash
# Fuzzy pick a template
gitignore

# Direct download without the picker
gitignore rust

# List available templates (cached)
gitignore --list

# Show the template without writing it
gitignore --dry-run node

# Overwrite a custom path
gitignore --output other.gitignore --overwrite Rust MacOS
```

Key flags:

- `--list` / `-l` – print all template names.
- `--output <PATH>` – where to write (default: `.gitignore`).
- `--overwrite` – replace instead of append.
- `--dry-run` – print to stdout.
- `--no-cache` – ignore cached template list.
- `--cache-ttl <MINUTES>` – cache lifetime (default 1440).

Built-in snippets: `--macos` and `--locks` append handy ignores without hitting the network.

The tool caches the template list under the XDG cache dir and will reuse it until it expires (defaults to 24h). When appending, it skips templates already present in the output.

## Development

- Build: `cargo build`
- Test: `cargo test`
