# emailbook (Rust)

A minimalistic address book for e-mails only.

This is a Rust rewrite of [emailbook-hare](https://git.sr.ht/~maxgyver83/emailbook-hare).

## Installation

Install [Rust](https://www.rust-lang.org/tools/install) if necessary.

```sh
cargo build --release
```

The binary will be at `target/release/emailbook`.

Copy it to a directory in your `PATH` (e.g. `$HOME/.local/bin/` or `/usr/local/bin/`).

## Usage

The address book file defaults to `emailbook.txt` in the platform data directory:

| Platform | Default path                                                            |
| -------- | ----------------------------------------------------------------------- |
| Linux    | `$XDG_DATA_HOME/emailbook.txt` (usually `~/.local/share/emailbook.txt`) |
| macOS    | `~/Library/Application Support/emailbook.txt`                           |
| Windows  | `%APPDATA%\emailbook.txt`                                               |

Override with `-f`/`--file`.

```
emailbook [OPTIONS] [COMMAND]

Commands:
  add          Add an entry
  search       Search entries
  remove       Remove entries interactively
  parse        Parse stdin for e-mail addresses and add them
  parse-files  Read filenames from stdin, parse them for e-mail addresses and add them
  completion   Generate shell completion scripts
  help         Print help

Options:
  -f, --file <FILE>  Address book file [default: <data_dir>/emailbook.txt]
```

### Add an entry

```sh
emailbook add 'John Doe <john.doe@example.com>'
emailbook add jd 'John Doe <john.doe@example.com>'   # with alias
```

Entries are either a bare mailbox (`MAILBOX`) or a keyed entry (`ALIAS : MAILBOX`).

### Search

```sh
emailbook search john          # search keys and values
emailbook search -k jd         # keys (aliases) only
emailbook search -v example    # values (e-mail addresses) only
```

### Remove entries interactively

```sh
emailbook remove john
```

For each match, you will be prompted:

```
John Doe <john.doe@example.com>
  Remove? [y/N]
```

### Parse e-mail headers

```sh
cat email.txt | emailbook parse           # all headers (From, To, Cc, Bcc)
cat email.txt | emailbook parse from      # From only
cat email.txt | emailbook parse to        # To only
```

Plain e-mail addresses are wrapped in angle brackets automatically.
Noreply addresses are always ignored.

### Parse multiple files

```sh
find ~/mail -type f | emailbook parse-files
```

### Shell completion

```sh
emailbook completion fish > ~/.config/fish/completions/emailbook.fish
emailbook completion bash > ~/.bash_completion.d/emailbook
emailbook completion zsh  > ~/.zfunc/_emailbook
```

## aerc

_emailbook_ might work for other e-mail clients but it was tested with
[aerc](https://sr.ht/~rjarry/aerc/).

#### Use emailbook for autocompletion

Add this line to `~/.config/aerc/aerc.conf`, `[compose]` section:

```conf
address-book-cmd=emailbook search "%s"
```

#### Add binding to parse all senders/recipients from a viewed message

Add this line to `~/.config/aerc/binds.conf`, `[view]` section:

```conf
aa = :pipe -m emailbook parse<Enter>
```

## How a mailbox looks like

A mailbox is an e-mail address plus optionally a display name.

- `john.doe@example.com`
- `<john.doe@example.com>`
- `John Doe <john.doe@example.com>`
- `"Doe, Joe" <john.doe@example.com>`

> Normally, a mailbox is composed of two parts: (1) an optional display
> name that indicates the name of the recipient [...] and (2) an addr-spec
> address enclosed in angle brackets ("<" and ">").

[RFC 5322, Section 3.4](https://www.rfc-editor.org/rfc/rfc5322.html#section-3.4)

## Changes from the Hare version

### Bug fixes

- **Fixed `--from`/`--to`/`--cc`/`--bcc` flags being silently ignored**: The original had
  a variable shadowing bug where the inner `fields` variable was assigned but the outer
  (empty) one was used for parsing.
- **Fixed file read bug in `parse_file`**: The original used the full buffer size regardless
  of how many bytes were actually read from the file.
- **Proper error handling**: Instead of aborting on errors (Hare's `!` operator) or silently
  ignoring OOM errors, all operations return `Result` types with meaningful error messages.

### Improvements

- **Complete charset decoding**: Instead of ~15 hardcoded ISO-8859-1 character replacements,
  uses the `encoding_rs` crate for proper conversion of all ISO-8859-1, ISO-8859-15,
  Windows-1252, and other charsets.
- **Case-insensitive noreply filtering**: The original only checked specific casings
  (`noreply`, `NoReply`). Now uses case-insensitive matching and covers more patterns
  (`donotreply`, `donotrespond`, `do-not-respond`, etc.).
- **No memory leaks**: Rust's ownership system prevents the memory leaks present in the
  original (e.g., allocated strings not freed in `match_mailbox` and `decode_encoded_words`).

## Running tests

```sh
cargo test
```
