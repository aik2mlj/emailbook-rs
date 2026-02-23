use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;
use std::sync::OnceLock;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use encoding_rs::Encoding;
use regex::Regex;

const MAX_HEADER_SIZE: usize = 20_000;

static RE_ENCODED_WORD: OnceLock<Regex> = OnceLock::new();

/// Represents an in-memory address book backed by a file.
pub struct EmailBook {
    pub lines: Vec<String>,
    path: PathBuf,
}

impl EmailBook {
    /// Opens or creates the address book file and loads its contents.
    pub fn open(path: &str) -> io::Result<Self> {
        let path = PathBuf::from(path);

        // Create the file if it doesn't exist
        if !path.exists() {
            fs::write(&path, "")?;
        }

        let content = fs::read_to_string(&path)?;
        let lines: Vec<String> = if content.trim().is_empty() {
            Vec::new()
        } else {
            content.trim().lines().map(String::from).collect()
        };

        Ok(EmailBook { lines, path })
    }

    /// Appends a line to the address book file.
    fn append_to_file(&self, line: &str) -> io::Result<()> {
        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    /// Checks if a key (alias) already exists in the address book.
    pub fn key_exists(&self, key: &str) -> bool {
        let prefix = format!("{key} :");
        self.lines.iter().any(|line| line.starts_with(&prefix))
    }

    /// Adds an entry to the address book. Returns true if added, false if skipped.
    pub fn add(&mut self, key: Option<&str>, value: &str) -> bool {
        let exists = match key {
            Some(k) => self.key_exists(k),
            None => {
                mailbox_in_list(value, &self.lines)
                    || mailbox_in_list(&toggle_quotes(value), &self.lines)
            }
        };

        if exists {
            let display = key.unwrap_or(value);
            println!("! {display} (skipped)");
            return false;
        }

        let line = match key {
            Some(k) => format!("{k} : {value}"),
            None => value.to_string(),
        };

        println!("+ {line}");
        self.lines.push(line.clone());
        if let Err(e) = self.append_to_file(&line) {
            eprintln!("Warning: failed to write to file: {e}");
        }
        true
    }

    /// Searches by alias (key) and prints matching values.
    pub fn search_by_alias(&self, query: &str) {
        let query_lower = query.to_lowercase();
        for line in &self.lines {
            if let Some(value) = check_alias(&query_lower, line) {
                println!("{value}");
            }
        }
    }

    /// Searches by value and prints matching values.
    pub fn search_by_value(&self, query: &str) {
        let query_lower = query.to_lowercase();
        for line in &self.lines {
            if let Some(value) = check_value(&query_lower, line) {
                println!("{value}");
            }
        }
    }

    /// Searches by value, excluding results that also match by alias.
    pub fn search_by_value_only(&self, query: &str) {
        let query_lower = query.to_lowercase();
        for line in &self.lines {
            if check_alias(&query_lower, line).is_some() {
                continue;
            }
            if let Some(value) = check_value(&query_lower, line) {
                println!("{value}");
            }
        }
    }

    /// Searches by both alias and value (alias matches first, then value-only).
    pub fn search_all(&self, query: &str) {
        self.search_by_alias(query);
        self.search_by_value_only(query);
    }

    /// Parses email headers from stdin and adds found addresses.
    pub fn parse_stdin(&mut self, fields: &[&str]) -> io::Result<()> {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;
        self.parse_content(&content, fields);
        Ok(())
    }

    /// Parses email headers from a file and adds found addresses.
    pub fn parse_file(&mut self, path: &str, fields: &[&str]) -> io::Result<()> {
        let file = fs::File::open(path)?;
        let mut reader = file.take(MAX_HEADER_SIZE as u64);
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        self.parse_content(&content, fields);
        Ok(())
    }

    /// Reads filenames from stdin, opens each, and parses their email headers.
    pub fn parse_files(&mut self, fields: &[&str]) -> io::Result<()> {
        let stdin = io::stdin();
        let filenames: Vec<String> = stdin.lock().lines().collect::<Result<_, _>>()?;
        for filename in &filenames {
            if !filename.is_empty() {
                if let Err(e) = self.parse_file(filename, fields) {
                    eprintln!("Warning: could not parse {filename}: {e}");
                }
            }
        }
        Ok(())
    }

    /// Core header parsing logic shared by parse_stdin and parse_file.
    fn parse_content(&mut self, content: &str, fields: &[&str]) {
        let mut continuation = false;
        let mut joined_lines = String::new();

        for text_line in content.lines() {
            // Empty line marks end of headers
            if text_line.is_empty() {
                break;
            }

            if let Some(idx) = starts_with_any(text_line, fields) {
                if !joined_lines.is_empty() {
                    self.process_line(&joined_lines);
                    joined_lines.clear();
                }
                continuation = true;
                joined_lines = text_line[fields[idx].len()..].trim_start().to_string();
            } else if continuation && (text_line.starts_with(' ') || text_line.starts_with('\t')) {
                joined_lines.push_str(text_line);
            } else {
                continuation = false;
            }
        }

        if !joined_lines.is_empty() {
            self.process_line(&joined_lines);
        }
    }

    /// Processes a joined header line by splitting at unquoted commas and
    /// matching each part as a mailbox.
    fn process_line(&mut self, line: &str) {
        let line = line.replace('\n', "");
        let parts = split_at_unquoted_commas(&line);
        for part in parts {
            let mailbox = part.trim();
            self.match_mailbox(mailbox);
        }
    }

    /// Validates, normalizes, and adds a mailbox to the address book.
    fn match_mailbox(&mut self, line: &str) {
        if !line.contains('@') {
            return;
        }

        // Filter out noreply addresses (case-insensitive, unlike the original)
        let lower = line.to_lowercase();
        if lower.contains("noreply")
            || lower.contains("no_reply")
            || lower.contains("no-reply")
            || lower.contains("not-reply")
            || lower.contains("not_reply")
            || lower.contains("do-not-reply")
            || lower.contains("do_not_reply")
            || lower.contains("donotreply")
            || lower.contains("donotrespond")
            || lower.contains("do-not-respond")
        {
            return;
        }

        // Decode MIME encoded words
        let line = decode_encoded_words(line);

        // Sanitize the mailbox
        let line = sanitize_mailbox(&line);

        // Bare address → wrap in angle brackets
        let line = if !line.contains(' ') && !line.starts_with('<') {
            format!("<{line}>")
        } else {
            line
        };

        // Ensure space before '<'
        let line = if let Some(pos) = line.find('<') {
            if pos > 0 && line.as_bytes()[pos - 1] != b' ' {
                format!("{} {}", &line[..pos], &line[pos..])
            } else {
                line
            }
        } else {
            line
        };

        // Check for duplicates (also with toggled quotes)
        if mailbox_in_list(&line, &self.lines)
            || mailbox_in_list(&toggle_quotes(&line), &self.lines)
        {
            println!("! {line}");
        } else {
            println!("+ {line}");
            self.lines.push(line.clone());
            if let Err(e) = self.append_to_file(&line) {
                eprintln!("Warning: failed to write to file: {e}");
            }
        }
    }
}

/// Checks if the query matches the alias (key) portion of a line.
/// Returns the value portion if matched.
pub fn check_alias(query_lower: &str, line: &str) -> Option<String> {
    let pos = line.find(':')?;
    if pos + 2 > line.len() {
        return None;
    }
    let alias = &line[..pos];
    if alias.to_lowercase().contains(query_lower) {
        Some(line[pos + 2..].to_string())
    } else {
        None
    }
}

/// Checks if the query matches the value portion of a line.
/// Returns the value portion if matched.
pub fn check_value(query_lower: &str, line: &str) -> Option<String> {
    let start = match line.find(':') {
        Some(pos) => pos + 2,
        None => 0,
    };
    if start >= line.len() {
        return None;
    }
    let value = &line[start..];
    if value.to_lowercase().contains(query_lower) {
        Some(value.to_string())
    } else {
        None
    }
}

/// Returns the index of the first matching prefix, if any.
fn starts_with_any(s: &str, prefixes: &[&str]) -> Option<usize> {
    prefixes.iter().position(|p| s.starts_with(p))
}

/// Checks if the mailbox already exists in the list (exact match).
fn mailbox_in_list(mailbox: &str, list: &[String]) -> bool {
    list.iter().any(|entry| entry == mailbox)
}

/// Toggles double quotes around the display name of a mailbox.
///
/// `John Doe <x>` becomes `"John Doe" <x>` and vice versa.
pub fn toggle_quotes(s: &str) -> String {
    if s.contains('"') {
        s.replace('"', "")
    } else if let Some(pos) = s.find(" <") {
        format!("\"{}\" <{}", &s[..pos], &s[pos + 2..])
    } else {
        s.to_string()
    }
}

/// Decodes a Quoted-Printable encoded string (UTF-8 charset).
pub fn decode_q_encoded_string(s: &str) -> String {
    decode_q_encoded_string_charset(s, "utf-8")
}

/// Decodes a Quoted-Printable encoded string for an arbitrary charset.
///
/// Uses `encoding_rs` for proper charset conversion instead of hardcoded
/// character replacements (fixes incomplete ISO-8859 handling in original).
pub fn decode_q_encoded_string_charset(s: &str, charset: &str) -> String {
    let bytes = s.as_bytes();
    let mut raw = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'=' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            match u8::from_str_radix(hex, 16) {
                Ok(b) => raw.push(b),
                Err(_) => raw.push(b'?'),
            }
            i += 3;
        } else if bytes[i] == b'_' {
            raw.push(b' ');
            i += 1;
        } else {
            raw.push(bytes[i]);
            i += 1;
        }
    }

    // Use encoding_rs for proper charset conversion
    if let Some(encoding) = Encoding::for_label(charset.as_bytes()) {
        let (decoded, _, _) = encoding.decode(&raw);
        decoded.into_owned()
    } else {
        String::from_utf8(raw).unwrap_or_else(|_| s.to_string())
    }
}

/// Decodes MIME encoded-word tokens (`=?charset?encoding?text?=`) in a string.
pub fn decode_encoded_words(line: &str) -> String {
    let re = RE_ENCODED_WORD
        .get_or_init(|| Regex::new(r"=\?([A-Za-z0-9-]+)\?([bBqQ])\?([^?]+)\?=").unwrap());
    let mut result = line.to_string();

    // Loop because there may be multiple encoded words
    while result.contains("=?") {
        let Some(caps) = re.captures(&result) else {
            break;
        };

        let full_match = caps.get(0).unwrap().as_str().to_string();
        let charset = caps[1].to_lowercase();
        let encoding = caps[2].to_lowercase();
        let encoded_text = &caps[3];

        let decoded = match (charset.as_str(), encoding.as_str()) {
            (_, "b") => {
                // Base64 decoding
                match BASE64.decode(encoded_text) {
                    Ok(bytes) => {
                        if let Some(enc) = Encoding::for_label(charset.as_bytes()) {
                            let (s, _, _) = enc.decode(&bytes);
                            s.into_owned()
                        } else {
                            String::from_utf8(bytes).unwrap_or_else(|_| full_match.clone())
                        }
                    }
                    Err(_) => break,
                }
            }
            ("utf-8", "q") => decode_q_encoded_string(encoded_text),
            (cs, "q") => decode_q_encoded_string_charset(encoded_text, cs),
            _ => break,
        };

        result = result.replace(&full_match, &decoded);
    }

    result
}

/// Sanitizes a mailbox string by normalizing whitespace, handling quotes, and
/// removing redundant display names.
pub fn sanitize_mailbox(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut within_quotes = false;
    let mut quoted_start = 0usize;
    let mut quoted_end = 0usize;
    let mut email_start = 0usize;
    let mut email_end = 0usize;

    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                out.push(bytes[i]);
                if i + 1 < bytes.len() {
                    i += 1;
                    out.push(bytes[i]);
                }
            }
            b' ' => {
                out.push(b' ');
                // Skip consecutive spaces
                while i + 1 < bytes.len() && bytes[i + 1] == b' ' {
                    i += 1;
                }
            }
            b'\t' => {
                out.push(b' ');
            }
            b'"' => {
                if within_quotes {
                    quoted_end = out.len();
                } else {
                    quoted_start = out.len() + 1;
                }
                within_quotes = !within_quotes;
                out.push(b'"');
            }
            b'\'' => {
                // Strip single quotes at specific positions (beginning, before
                // double-quote, before ' <')
                if i == 0
                    || (i > 0 && bytes[i - 1] == b'"')
                    || (i + 1 < bytes.len() && bytes[i + 1] == b'"')
                    || (i + 2 < bytes.len() && bytes[i + 1] == b' ' && bytes[i + 2] == b'<')
                {
                    i += 1;
                    continue;
                }
                out.push(bytes[i]);
            }
            b'<' => {
                if !within_quotes {
                    email_start = out.len();
                }
                out.push(b'<');
            }
            b'>' => {
                if !within_quotes {
                    email_end = out.len();
                }
                out.push(b'>');
            }
            _ => {
                out.push(bytes[i]);
            }
        }
        i += 1;
    }

    // Remove display name if it duplicates the email address
    if quoted_end > 0 && email_end > 0 && quoted_start <= quoted_end && email_start < email_end {
        if let (Ok(display_name), Ok(email_addr)) = (
            std::str::from_utf8(&out[quoted_start..quoted_end]),
            std::str::from_utf8(&out[email_start + 1..email_end]),
        ) {
            if display_name == email_addr {
                let trimmed = &out[email_start..];
                return String::from_utf8_lossy(trimmed).to_string();
            }
            // Also check if display name includes the angle brackets
            if let Ok(email_with_brackets) = std::str::from_utf8(&out[email_start..email_end + 1]) {
                if display_name == email_with_brackets {
                    let trimmed = &out[email_start..];
                    return String::from_utf8_lossy(trimmed).to_string();
                }
            }
        }
    }

    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).to_string())
}

/// Splits a string at commas that are not inside double quotes.
pub fn split_at_unquoted_commas(s: &str) -> Vec<String> {
    let bytes = s.as_bytes();
    let mut result = Vec::new();
    let mut quoted = false;
    let mut start = 0;

    let mut i = 0;
    while i < bytes.len() {
        if quoted {
            if bytes[i] == b'\\' {
                i += 1; // skip next char
            } else if bytes[i] == b'"' {
                quoted = false;
            }
        } else if bytes[i] == b'"' {
            quoted = true;
        } else if bytes[i] == b',' {
            result.push(s[start..i].to_string());
            start = i + 1;
        }
        i += 1;
    }
    result.push(s[start..].to_string());
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn test_lines() -> Vec<String> {
        vec![
            "alice : Alice Z. <alice@harelang.org>".to_string(),
            "bob : Robert Y. <robert@harelang.org>".to_string(),
            "carol : <carol@sourehut.org>".to_string(),
            "Dan <daniel@sourcehut.org>".to_string(),
        ]
    }

    fn make_book(lines: Vec<String>) -> EmailBook {
        let mut tmp = NamedTempFile::new().unwrap();
        for line in &lines {
            writeln!(tmp, "{line}").unwrap();
        }
        EmailBook {
            lines,
            path: tmp.path().to_path_buf(),
        }
    }

    // ── key_exists ──────────────────────────────────────────────────

    #[test]
    fn test_key_exists() {
        let book = make_book(test_lines());
        assert!(book.key_exists("alice"));
        assert!(!book.key_exists("ali"));
        assert!(!book.key_exists("Dan"));
    }

    // ── add ─────────────────────────────────────────────────────────

    #[test]
    fn test_add_skips_existing_key() {
        let mut book = make_book(test_lines());
        assert!(!book.add(Some("bob"), "Robert X."));
    }

    #[test]
    fn test_add_new_key() {
        let mut book = make_book(test_lines());
        assert!(book.add(Some("rob"), "Robert Y. <robert@harelang.org>"));
    }

    #[test]
    fn test_add_skips_duplicate_value() {
        let mut book = make_book(test_lines());
        assert!(!book.add(None, "Dan <daniel@sourcehut.org>"));
    }

    #[test]
    fn test_add_skips_toggled_quotes_duplicate() {
        let mut book = make_book(vec!["John Doe <john.doe@example.com>".to_string()]);
        // "John Doe" with quotes is the same as without
        assert!(!book.add(None, "\"John Doe\" <john.doe@example.com>"));
    }

    #[test]
    fn test_add_new_entry() {
        let mut book = make_book(test_lines());
        assert!(book.add(None, "New Person <new@example.com>"));
        assert!(
            book.lines
                .contains(&"New Person <new@example.com>".to_string())
        );
    }

    // ── check_alias ─────────────────────────────────────────────────

    #[test]
    fn test_check_alias() {
        assert_eq!(
            check_alias("john", "john : <john@xxx>"),
            Some("<john@xxx>".to_string())
        );
        assert_eq!(
            check_alias("jo", "john : <john@xxx>"),
            Some("<john@xxx>".to_string())
        );
        assert_eq!(
            check_alias("n", "john : <john@xxx>"),
            Some("<john@xxx>".to_string())
        );
        assert_eq!(
            check_alias("me", "me : <john@xxx>"),
            Some("<john@xxx>".to_string())
        );
        assert_eq!(check_alias("xxx", "john : <john@xxx>"), None);
        assert_eq!(check_alias("xxx", "<john@xxx>"), None);
    }

    // ── check_value ─────────────────────────────────────────────────

    #[test]
    fn test_check_value() {
        assert_eq!(
            check_value("john", "john : <john@xxx>"),
            Some("<john@xxx>".to_string())
        );
        assert_eq!(
            check_value("jo", "john : <john@xxx>"),
            Some("<john@xxx>".to_string())
        );
        assert_eq!(
            check_value("n", "john : <john@xxx>"),
            Some("<john@xxx>".to_string())
        );
        assert_eq!(
            check_value("xxx", "john : <john@xxx>"),
            Some("<john@xxx>".to_string())
        );
        assert_eq!(
            check_value("xxx", "<john@xxx>"),
            Some("<john@xxx>".to_string())
        );
        assert_eq!(check_value("y", "<john@xxx>"), None);
        assert_eq!(check_value("me", "me : <john@xxx>"), None);
    }

    // ── search functions ────────────────────────────────────────────

    #[test]
    fn test_search_by_alias_logic() {
        let lines = test_lines();
        let query = "bob";
        let query_lower = query.to_lowercase();
        let results: Vec<_> = lines
            .iter()
            .filter_map(|l| check_alias(&query_lower, l))
            .collect();
        assert_eq!(results, vec!["Robert Y. <robert@harelang.org>"]);
    }

    #[test]
    fn test_search_by_alias_partial() {
        let lines = test_lines();
        let query_lower = "c";
        let results: Vec<_> = lines
            .iter()
            .filter_map(|l| check_alias(query_lower, l))
            .collect();
        assert_eq!(
            results,
            vec!["Alice Z. <alice@harelang.org>", "<carol@sourehut.org>"]
        );
    }

    #[test]
    fn test_search_by_alias_no_match_unkeyed() {
        let lines = test_lines();
        let query_lower = "dan";
        let results: Vec<_> = lines
            .iter()
            .filter_map(|l| check_alias(query_lower, l))
            .collect();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_by_value_logic() {
        let lines = test_lines();
        let query_lower = "harelang";
        let results: Vec<_> = lines
            .iter()
            .filter_map(|l| check_value(query_lower, l))
            .collect();
        assert_eq!(
            results,
            vec![
                "Alice Z. <alice@harelang.org>",
                "Robert Y. <robert@harelang.org>"
            ]
        );
    }

    #[test]
    fn test_search_by_value_only_logic() {
        let lines = test_lines();
        let query_lower = "c";
        // search_by_value_only: exclude lines where alias also matches
        let results: Vec<_> = lines
            .iter()
            .filter(|l| check_alias(query_lower, l).is_none())
            .filter_map(|l| check_value(query_lower, l))
            .collect();
        assert_eq!(results, vec!["Dan <daniel@sourcehut.org>"]);
    }

    #[test]
    fn test_search_all_logic() {
        let lines = test_lines();
        let query_lower = "c";
        // Alias matches first
        let alias_results: Vec<_> = lines
            .iter()
            .filter_map(|l| check_alias(query_lower, l))
            .collect();
        // Then value-only matches
        let value_only_results: Vec<_> = lines
            .iter()
            .filter(|l| check_alias(query_lower, l).is_none())
            .filter_map(|l| check_value(query_lower, l))
            .collect();
        let mut all = alias_results;
        all.extend(value_only_results);
        assert_eq!(
            all,
            vec![
                "Alice Z. <alice@harelang.org>",
                "<carol@sourehut.org>",
                "Dan <daniel@sourcehut.org>"
            ]
        );
    }

    // ── decode_q_encoded_string ─────────────────────────────────────

    #[test]
    fn test_decode_q_utf8() {
        assert_eq!(decode_q_encoded_string("M=C3=BCller"), "Müller");
        assert_eq!(
            decode_q_encoded_string("B=C3=A9la_Bart=C3=B3k"),
            "Béla Bartók"
        );
        assert_eq!(decode_q_encoded_string("=F0=9F=98=8E"), "\u{1f60e}");
    }

    // ── decode_q_encoded_string_charset ─────────────────────────────

    #[test]
    fn test_decode_q_iso8859() {
        assert_eq!(
            decode_q_encoded_string_charset("M=FCller", "iso-8859-1"),
            "Müller"
        );
        assert_eq!(
            decode_q_encoded_string_charset("B=E9la_Bart=F3k", "iso-8859-1"),
            "Béla Bartók"
        );
    }

    #[test]
    fn test_decode_q_windows1252() {
        assert_eq!(
            decode_q_encoded_string_charset("M=FCller", "windows-1252"),
            "Müller"
        );
    }

    // ── decode_encoded_words ────────────────────────────────────────

    #[test]
    fn test_decode_encoded_words_q() {
        assert_eq!(decode_encoded_words("=?UTF-8?Q?M=C3=B6ller?="), "Möller");
    }

    #[test]
    fn test_decode_encoded_words_b() {
        assert_eq!(
            decode_encoded_words("=?UTF-8?B?5byg5LiJ?= <zhang.san@example.com>"),
            "张三 <zhang.san@example.com>"
        );
    }

    #[test]
    fn test_decode_encoded_words_iso() {
        assert_eq!(
            decode_encoded_words("=?ISO-8859-1?Q?Max M=FCller?= <max.m@example.com>"),
            "Max Müller <max.m@example.com>"
        );
    }

    #[test]
    fn test_decode_encoded_words_no_encoding() {
        assert_eq!(decode_encoded_words("plain text"), "plain text");
    }

    // ── sanitize_mailbox ────────────────────────────────────────────

    #[test]
    fn test_sanitize_plain() {
        assert_eq!(sanitize_mailbox("john"), "john");
    }

    #[test]
    fn test_sanitize_no_space_before_bracket() {
        assert_eq!(sanitize_mailbox("john<xxx>"), "john<xxx>");
    }

    #[test]
    fn test_sanitize_with_space() {
        assert_eq!(sanitize_mailbox("john <xxx>"), "john <xxx>");
    }

    #[test]
    fn test_sanitize_double_space() {
        assert_eq!(sanitize_mailbox("john  <xxx>"), "john <xxx>");
    }

    #[test]
    fn test_sanitize_tab() {
        assert_eq!(sanitize_mailbox("john\t<xxx>"), "john <xxx>");
    }

    #[test]
    fn test_sanitize_single_quotes() {
        assert_eq!(sanitize_mailbox("'john' <xxx>"), "john <xxx>");
    }

    #[test]
    fn test_sanitize_single_inside_double_quotes() {
        assert_eq!(sanitize_mailbox("\"'john'\" <xxx>"), "\"john\" <xxx>");
    }

    #[test]
    fn test_sanitize_apostrophe_preserved() {
        assert_eq!(
            sanitize_mailbox("\"joe's garage\" <xxx>"),
            "\"joe's garage\" <xxx>"
        );
    }

    #[test]
    fn test_sanitize_redundant_display_name() {
        assert_eq!(sanitize_mailbox("\"xxx\" <xxx>"), "<xxx>");
    }

    #[test]
    fn test_sanitize_redundant_display_with_brackets() {
        assert_eq!(sanitize_mailbox("\"<xxx>\" <xxx>"), "<xxx>");
    }

    // ── toggle_quotes ───────────────────────────────────────────────

    #[test]
    fn test_toggle_quotes_add() {
        assert_eq!(
            toggle_quotes("John Doe <john@example.com>"),
            "\"John Doe\" <john@example.com>"
        );
    }

    #[test]
    fn test_toggle_quotes_remove() {
        assert_eq!(
            toggle_quotes("\"John Doe\" <john@example.com>"),
            "John Doe <john@example.com>"
        );
    }

    // ── split_at_unquoted_commas ────────────────────────────────────

    #[test]
    fn test_split_simple() {
        let result = split_at_unquoted_commas("a@x.com, b@y.com");
        assert_eq!(result, vec!["a@x.com", " b@y.com"]);
    }

    #[test]
    fn test_split_quoted_comma() {
        let result = split_at_unquoted_commas("\"Doe, John\" <j@x.com>, b@y.com");
        assert_eq!(result, vec!["\"Doe, John\" <j@x.com>", " b@y.com"]);
    }

    #[test]
    fn test_split_no_comma() {
        let result = split_at_unquoted_commas("a@x.com");
        assert_eq!(result, vec!["a@x.com"]);
    }

    // ── noreply filtering ───────────────────────────────────────────

    #[test]
    fn test_noreply_filtered() {
        let mut book = make_book(vec![]);
        // These should all be silently filtered
        book.match_mailbox("noreply@example.com");
        book.match_mailbox("NoReply@example.com");
        book.match_mailbox("no-reply@example.com");
        book.match_mailbox("do-not-reply@example.com");
        book.match_mailbox("NOREPLY@example.com");
        book.match_mailbox("donotreply@example.com");
        assert!(book.lines.is_empty());
    }

    // ── parse_content (integration-style) ───────────────────────────

    #[test]
    fn test_parse_sample1() {
        let mut book = make_book(vec![]);
        let content = "Subject: Test e-mail 1\nFrom: John Doe <john.doe@example.com>\nTo: Erika Mustermann\n <e.mustermann@example.com>,\n <max.m@example.com>,\n mario.r@example.com\nCc: =?UTF-8?B?5byg5LiJ?= <zhang.san@example.com>,\n Maria Rossi <maria.r@example.com>\n\nContent";

        let fields = vec!["From:", "To:", "Cc:", "CC:", "Bcc:"];
        book.parse_content(content, &fields);

        assert_eq!(book.lines.len(), 6);
        assert!(
            book.lines
                .contains(&"John Doe <john.doe@example.com>".to_string())
        );
        assert!(
            book.lines
                .contains(&"Erika Mustermann <e.mustermann@example.com>".to_string())
        );
        assert!(book.lines.contains(&"<max.m@example.com>".to_string()));
        assert!(book.lines.contains(&"<mario.r@example.com>".to_string()));
        assert!(
            book.lines
                .contains(&"张三 <zhang.san@example.com>".to_string())
        );
        assert!(
            book.lines
                .contains(&"Maria Rossi <maria.r@example.com>".to_string())
        );
    }

    #[test]
    fn test_parse_sample2_dedup() {
        // Start with the results from sample1
        let initial = vec![
            "John Doe <john.doe@example.com>".to_string(),
            "Erika Mustermann <e.mustermann@example.com>".to_string(),
            "<max.m@example.com>".to_string(),
            "<mario.r@example.com>".to_string(),
            "张三 <zhang.san@example.com>".to_string(),
            "Maria Rossi <maria.r@example.com>".to_string(),
        ];
        let mut book = make_book(initial);

        let content = "Subject: Test e-mail 2\nFrom: John Doe <john.doe@example.com>\nTo: \"Erika Mustermann\"<e.mustermann@example.com>,\n =?ISO-8859-1?Q?Max M=FCller?= <max.m@example.com>,\n <mario.r@example.com>\nCc: zhang.san@example.com, Maria Rossi <maria.r@example.com>\n\nContent";

        let fields = vec!["From:", "To:", "Cc:", "CC:", "Bcc:"];
        book.parse_content(content, &fields);

        // Should have added 2 new entries: Max Müller and bare zhang.san
        assert_eq!(book.lines.len(), 8);
        assert!(
            book.lines
                .contains(&"Max Müller <max.m@example.com>".to_string())
        );
        assert!(book.lines.contains(&"<zhang.san@example.com>".to_string()));
    }

    // ── mailbox_in_list ─────────────────────────────────────────────

    #[test]
    fn test_mailbox_in_list() {
        let list = vec![
            "Alice <alice@example.com>".to_string(),
            "<bob@example.com>".to_string(),
        ];
        assert!(mailbox_in_list("Alice <alice@example.com>", &list));
        assert!(mailbox_in_list("<bob@example.com>", &list));
        assert!(!mailbox_in_list("Charlie <charlie@example.com>", &list));
    }

    // ── match_mailbox edge cases ────────────────────────────────────

    #[test]
    fn test_match_mailbox_no_at() {
        let mut book = make_book(vec![]);
        book.match_mailbox("not-an-email");
        assert!(book.lines.is_empty());
    }

    #[test]
    fn test_match_mailbox_bare_address() {
        let mut book = make_book(vec![]);
        book.match_mailbox("user@example.com");
        assert_eq!(book.lines, vec!["<user@example.com>"]);
    }

    #[test]
    fn test_match_mailbox_space_before_bracket() {
        let mut book = make_book(vec![]);
        book.match_mailbox("\"John Doe\"<john@example.com>");
        // Should insert space before <
        assert_eq!(book.lines, vec!["\"John Doe\" <john@example.com>"]);
    }
}
