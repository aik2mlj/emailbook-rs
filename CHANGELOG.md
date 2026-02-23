# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-02-22

### Added

- Subcommand-based CLI: `add`, `search`, `remove`, `parse`, `parse-files`, `completion`
- `remove` subcommand: interactively delete entries with a per-match `y/[N]` prompt
- `completion` subcommand: generate shell completion scripts (bash, zsh, fish, elvish, powershell)
- Default file path uses the platform data directory (`emailbook.txt` under `$XDG_DATA_HOME` on Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on Windows), via the `directories` crate

### Changed

- `-k`/`-v` (key/value search filters) are now flags under the `search` and `remove` subcommands
- File path is now specified with `-f`/`--file` instead of as a positional argument

### Removed

- Positional `FILE` argument (replaced by `-f`/`--file` with a sensible default)

## [0.2.0] - 2026-02-22

### Added

- Initial Rust rewrite of [emailbook-hare](https://git.sr.ht/~maxgyver83/emailbook-hare)
- Proper charset decoding via `encoding_rs` (ISO-8859-1, ISO-8859-15, Windows-1252, and more)
- Case-insensitive noreply filtering covering additional patterns (`donotreply`, `donotrespond`, `do-not-respond`, etc.)
- Full `Result`-based error handling with meaningful messages

### Fixed

- `--from`/`--to`/`--cc`/`--bcc` flags were silently ignored due to a variable shadowing bug in the original
- File read bug in `parse_file`: read actual bytes rather than the full buffer size

[Unreleased]: https://github.com/aik2mlj/emailbook-rs/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/aik2mlj/emailbook-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/aik2mlj/emailbook-rs/releases/tag/v0.2.0
