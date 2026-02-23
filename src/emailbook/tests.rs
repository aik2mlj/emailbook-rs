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
