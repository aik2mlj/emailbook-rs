use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use encoding_rs::Encoding;

const MAX_HEADER_SIZE: usize = 20_000;

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

    /// Returns indices of lines whose key matches the query.
    pub fn search_by_alias(&self, query: &str) -> Vec<usize> {
        let q = query.to_lowercase();
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, line)| check_alias(&q, line).is_some())
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns indices of lines whose value matches the query.
    pub fn search_by_value(&self, query: &str) -> Vec<usize> {
        let q = query.to_lowercase();
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, line)| check_value(&q, line).is_some())
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns indices of all matching lines (alias matches first, then value-only), without duplicates.
    pub fn search_all(&self, query: &str) -> Vec<usize> {
        let q = query.to_lowercase();
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for (i, line) in self.lines.iter().enumerate() {
            if check_alias(&q, line).is_some() {
                seen.insert(i);
                result.push(i);
            }
        }
        for (i, line) in self.lines.iter().enumerate() {
            if !seen.contains(&i) && check_value(&q, line).is_some() {
                result.push(i);
            }
        }
        result
    }

    /// Removes lines at the given indices and rewrites the file.
    pub fn remove_lines(&mut self, indices: &[usize]) -> io::Result<()> {
        let to_remove: std::collections::HashSet<usize> = indices.iter().copied().collect();
        let mut i = 0;
        self.lines.retain(|_| {
            let keep = !to_remove.contains(&i);
            i += 1;
            keep
        });
        let content = if self.lines.is_empty() {
            String::new()
        } else {
            self.lines.join("\n") + "\n"
        };
        fs::write(&self.path, content)
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
            if !filename.is_empty()
                && let Err(e) = self.parse_file(filename, fields)
            {
                eprintln!("Warning: could not parse {filename}: {e}");
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

/// Finds the next MIME encoded-word `=?charset?encoding?text?=` in `s` starting
/// at byte offset `start`. Returns `(match_start, charset, encoding_char, text, match_end)`.
fn find_encoded_word(s: &str) -> Option<(usize, &str, u8, &str, usize)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        // Find "=?"
        if bytes[i] != b'=' || bytes[i + 1] != b'?' {
            i += 1;
            continue;
        }
        let token_start = i;
        i += 2;
        // charset: alnum or '-'
        let charset_start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'?' || charset_start == i {
            continue;
        }
        let charset = &s[charset_start..i];
        i += 1; // skip '?'
        // encoding: b, B, q, Q
        if i >= bytes.len() || !matches!(bytes[i], b'b' | b'B' | b'q' | b'Q') {
            continue;
        }
        let enc_byte = bytes[i].to_ascii_lowercase();
        i += 1;
        if i >= bytes.len() || bytes[i] != b'?' {
            continue;
        }
        i += 1; // skip '?'
        // text: anything except '?'
        let text_start = i;
        while i < bytes.len() && bytes[i] != b'?' {
            i += 1;
        }
        if i + 1 >= bytes.len() || bytes[i] != b'?' || bytes[i + 1] != b'=' {
            continue;
        }
        let text = &s[text_start..i];
        i += 2; // skip "?="
        return Some((token_start, charset, enc_byte, text, i));
    }
    None
}

/// Decodes MIME encoded-word tokens (`=?charset?encoding?text?=`) in a string.
pub fn decode_encoded_words(line: &str) -> String {
    if !line.contains("=?") {
        return line.to_string();
    }

    let mut result = line.to_string();

    loop {
        if !result.contains("=?") {
            break;
        }
        let Some((start, charset, enc_byte, text, end)) = find_encoded_word(&result) else {
            break;
        };

        let charset_lower = charset.to_lowercase();
        let decoded = match enc_byte {
            b'b' => match BASE64.decode(text) {
                Ok(bytes) => {
                    if let Some(enc) = Encoding::for_label(charset_lower.as_bytes()) {
                        let (s, _, _) = enc.decode(&bytes);
                        s.into_owned()
                    } else {
                        String::from_utf8(bytes).unwrap_or_else(|_| result[start..end].to_string())
                    }
                }
                Err(_) => break,
            },
            b'q' if charset_lower == "utf-8" => decode_q_encoded_string(text),
            b'q' => decode_q_encoded_string_charset(text, &charset_lower),
            _ => break,
        };

        result = format!("{}{}{}", &result[..start], decoded, &result[end..]);
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
    if quoted_end > 0
        && email_end > 0
        && quoted_start <= quoted_end
        && email_start < email_end
        && let (Ok(display_name), Ok(email_addr)) = (
            std::str::from_utf8(&out[quoted_start..quoted_end]),
            std::str::from_utf8(&out[email_start + 1..email_end]),
        )
    {
        if display_name == email_addr {
            let trimmed = &out[email_start..];
            return String::from_utf8_lossy(trimmed).to_string();
        }
        // Also check if display name includes the angle brackets
        if let Ok(email_with_brackets) = std::str::from_utf8(&out[email_start..email_end + 1])
            && display_name == email_with_brackets
        {
            let trimmed = &out[email_start..];
            return String::from_utf8_lossy(trimmed).to_string();
        }
    }

    let result = String::from_utf8(out)
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).to_string());
    strip_unnecessary_display_quotes(&result).unwrap_or(result)
}

/// Strips double quotes from the display name when they are unnecessary,
/// i.e. the display name contains no RFC 5322 special characters and is pure ASCII.
fn strip_unnecessary_display_quotes(s: &str) -> Option<String> {
    if !s.starts_with('"') {
        return None;
    }
    // Find closing quote, honouring backslash escapes
    let bytes = s.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => break,
            _ => i += 1,
        }
    }
    if i >= bytes.len() {
        return None; // no closing quote
    }
    let display = &s[1..i];
    let after = &s[i + 1..]; // everything after the closing quote

    // Must be followed by " <..." to be a mailbox display name
    if !after.starts_with(' ') || !after[1..].starts_with('<') {
        return None;
    }
    if display.is_empty() {
        return None;
    }
    // RFC 5322 specials — any of these in the display name requires quoting
    const SPECIALS: &[u8] = b"()<>[]:;@\\,.\"";
    if display
        .bytes()
        .any(|b| SPECIALS.contains(&b) || !(0x20..=0x7e).contains(&b))
    {
        return None;
    }
    Some(format!("{display}{after}"))
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
mod tests;
