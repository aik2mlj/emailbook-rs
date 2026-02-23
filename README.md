# emailbook (Rust)

A minimalistic address book for e-mails only.

This is a Rust rewrite of [emailbook-hare](https://git.sr.ht/~maxgyver83/emailbook-hare).

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

## Installation

Install [Rust](https://www.rust-lang.org/tools/install) if necessary.

```sh
cargo build --release
```

The binary will be at `target/release/emailbook`.

Copy it to a directory in your `PATH` (e.g. `$HOME/.local/bin/` or `/usr/local/bin/`).

## Usage

### Command line

#### Add a new entry:

```sh
emailbook ~/emailbook.txt --add 'jd : John Doe <john.doe@example.com>'
```

Entries should look like `MAILBOX` or `ALIAS : MAILBOX`.

- `ALIAS` is your personal abbreviation (optional).
- See [How a mailbox looks like](#how-a-mailbox-looks-like).

#### Search a recipient:

```sh
emailbook ~/emailbook.txt --search 'john'
emailbook ~/emailbook.txt --key 'jd'
emailbook ~/emailbook.txt --value 'john'
```

`--search` looks both at keys (=aliases) and values. `--key` and `--value`
limit the search accordingly.

#### Add all senders/recipients from an e-mail to your address book:

```sh
cat email.txt | emailbook ~/emailbook.txt --parse --all
```

This skips e.g. `"John Doe" <jd@example.com>` when the address book already
contains the same entry without double quotes (and vice versa).

Plain e-mail addresses are wrapped in angle brackets automatically.

E-mails like `noreply@example.com` are always ignored.

### aerc

_emailbook_ might work for other e-mail clients but it was tested with
[aerc](https://sr.ht/~rjarry/aerc/).

#### Use emailbook for autocompletion:

Add this line to `~/.config/aerc/aerc.conf`, `[compose]` section:

```conf
address-book-cmd=emailbook /home/user/emailbook.txt --search "%s"
```

#### Add binding to add all senders/recipients:

Add this line to `~/.config/aerc/binds.conf`, `[view]` section:

```conf
aa = :pipe -m emailbook /home/user/emailbook.txt --parse --all<Enter>
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

## Running tests

```sh
cargo test
```
