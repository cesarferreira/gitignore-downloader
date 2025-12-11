<h1 align="center">gitignore-downloader</h1>
<p align="center">Fetch and compose GitHub's <code>.gitignore</code> templates from your terminal.</p>
<p align="center">
  <a href="https://crates.io/crates/gitignore-downloader"><img src="https://img.shields.io/crates/v/gitignore-downloader.svg" alt="Crates.io"></a>
  <a href="https://crates.io/crates/gitignore-downloader"><img src="https://img.shields.io/crates/d/gitignore-downloader.svg" alt="Downloads"></a>
  <a href="https://github.com/cesarferreira/gitignore/blob/master/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
</p>

When no type is provided, a fuzzy picker helps you choose; when you pass a type, it downloads immediately.

[List of available templates](https://github.com/github/gitignore)

## Install

```bash
cargo install gitignore-downloader
```

This installs the `gi` binary.

## Usage

```bash
# Fuzzy pick a template
gi

# Direct download without the picker
gi rust

# List available templates (cached)
gi --list

# Show the template without writing it
gi --dry-run node

# Overwrite a custom path
gi --output other.gitignore --overwrite Rust MacOS
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

## License

MIT
