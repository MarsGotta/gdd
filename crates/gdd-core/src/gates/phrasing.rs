//! The sentences of the steps of the scenario files, with their data blanked. Test mode never
//! sees the hidden examples of a change, so it can only write their steps when they reuse the
//! sentences of the visible scenarios, with other data.

use std::collections::BTreeSet;
use std::str::Chars;

/// Words that start a step.
const KEYWORDS: [&str; 6] = ["Given", "When", "Then", "And", "But", "*"];

/// Marks that open and close a doc string.
const DOC_STRING: [&str; 2] = ["\"\"\"", "```"];

/// The step sentences of the scenario files `texts`, without their keyword and with their data
/// blanked: quoted text becomes `"…"`, an outline placeholder `<…>` and a number `#`. Doc
/// strings are left out.
pub fn phrasings<S: AsRef<str>>(texts: &[S]) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for text in texts {
        let mut in_doc_string = false;
        for line in text.as_ref().lines() {
            let trimmed = line.trim();
            if DOC_STRING.iter().any(|mark| trimmed.starts_with(mark)) {
                in_doc_string = !in_doc_string;
            } else if !in_doc_string && let Some(sentence) = sentence(line) {
                let blanked = blank_placeholders(&blank_quotes(sentence));
                found.insert(blank_numbers(&blanked));
            }
        }
    }
    found
}

/// The sentences of the scenario files `hidden` that none of the files `visible` has (see
/// [`phrasings`]), sorted.
pub fn unknown<S: AsRef<str>>(hidden: &[S], visible: &[S]) -> Vec<String> {
    let visible = phrasings(visible);
    phrasings(hidden)
        .into_iter()
        .filter(|sentence| !visible.contains(sentence))
        .collect()
}

/// The sentence of a step line: what follows its keyword and at least one space, trimmed.
fn sentence(line: &str) -> Option<&str> {
    let line = line.trim_start();
    KEYWORDS.iter().find_map(|keyword| {
        let rest = line.strip_prefix(keyword)?;
        if rest.starts_with(char::is_whitespace) {
            Some(rest.trim())
        } else {
            None
        }
    })
}

/// `text` with each quoted text, from a quote to the next one that no backslash escapes,
/// replaced by `"…"`.
fn blank_quotes(text: &str) -> String {
    let mut blanked = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        let closed = if c == '"' {
            after_quote(chars.clone())
        } else {
            None
        };
        match closed {
            Some(rest) => {
                blanked.push_str("\"…\"");
                chars = rest;
            }
            None => blanked.push(c),
        }
    }
    blanked
}

/// What follows the quoted text `chars` starts inside, right after its opening quote: what
/// comes after the first quote no backslash escapes, when one closes it.
fn after_quote(mut chars: Chars<'_>) -> Option<Chars<'_>> {
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(chars),
            '\\' => {
                chars.next();
            }
            _ => {}
        }
    }
    None
}

/// `text` with each `<placeholder>` of a scenario outline replaced by `<…>`.
fn blank_placeholders(text: &str) -> String {
    let mut blanked = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let after = &rest[start + 1..];
        match after.find('>') {
            Some(end) if end > 0 => {
                blanked.push_str(&rest[..start]);
                blanked.push_str("<…>");
                rest = &after[end + 1..];
            }
            _ => {
                blanked.push_str(&rest[..=start]);
                rest = after;
            }
        }
    }
    blanked.push_str(rest);
    blanked
}

/// `text` with each number that stands as a word of its own, with or without decimals,
/// replaced by `#`.
fn blank_numbers(text: &str) -> String {
    let words: Vec<(&str, &str)> = text.split_inclusive(|c| !is_word(&c)).map(ending).collect();
    let mut blanked = String::new();
    let mut rest = words.as_slice();
    loop {
        match rest {
            [(whole, "."), (decimals, end), after @ ..]
                if is_number(whole) && is_number(decimals) =>
            {
                blanked.push('#');
                blanked.push_str(end);
                rest = after;
            }
            [(word, end), after @ ..] => {
                blanked.push_str(if is_number(word) { "#" } else { word });
                blanked.push_str(end);
                rest = after;
            }
            [] => return blanked,
        }
    }
}

/// A piece of text that ends a word, as `split_inclusive` cuts it: the word (maybe empty) and
/// the character that ends it, when one does.
fn ending(piece: &str) -> (&str, &str) {
    match piece.char_indices().next_back() {
        Some((i, c)) if !is_word(&c) => piece.split_at(i),
        _ => (piece, ""),
    }
}

/// Whether `word` is a number: digits and nothing else.
fn is_number(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_digit())
}

/// Whether `c` belongs to a word: a letter, a digit or `_`.
fn is_word(c: &char) -> bool {
    c.is_alphanumeric() || *c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(sentences: &[&str]) -> BTreeSet<String> {
        sentences.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_sentences_of_the_steps_lose_their_keyword() {
        let feature = "Feature: Drafts\n  Background:\n    Given a draft\n\n  Scenario: One\n    \
                       When I save it\n\tThen it is saved\n    And it is listed  \n    \
                       But it is not sent\n    * it is kept\n    # Given a comment\n    \
                       Givenx not a step\n    Given\n    Thenit is not a step\n";
        let expected = set(&[
            "a draft",
            "I save it",
            "it is saved",
            "it is listed",
            "it is not sent",
            "it is kept",
        ]);
        assert_eq!(phrasings(&[feature]), expected);
        let spaced = phrasings(&["Given  \t spaced  out "]);
        assert_eq!(spaced, set(&["spaced  out"]));
        assert_eq!(phrasings(&["Given\ttabbed"]), set(&["tabbed"]));
        assert_eq!(phrasings::<&str>(&[]), set(&[]));
    }

    #[test]
    fn doc_strings_are_left_out() {
        let feature = "Given a file:\n  \"\"\"\n  Given not a step\n  \"\"\"\nThen a step\n\
                       ```\nWhen not either\n```\nAnd again\n";
        let expected = set(&["a file:", "a step", "again"]);
        assert_eq!(phrasings(&[feature]), expected);
        let two = ["Given one\n\"\"\"\n", "Given two\n"];
        assert_eq!(phrasings(&two), set(&["one", "two"]));
    }

    #[test]
    fn quoted_data_is_blanked() {
        assert_eq!(blank_quotes(r#"I run "gdd status""#), "I run \"…\"");
        assert_eq!(blank_quotes(r#""a" and "b""#), "\"…\" and \"…\"");
        let escaped = blank_quotes(r#"it says "a \"b\" c" now"#);
        assert_eq!(escaped, "it says \"…\" now");
        assert_eq!(blank_quotes(r#"a \"b" c"#), "a \\\"…\" c");
        assert_eq!(blank_quotes(r#"an "open quote"#), "an \"open quote");
        assert_eq!(blank_quotes(r#"a "b\"#), "a \"b\\");
        assert_eq!(blank_quotes(r#"a "b\" c"#), "a \"b\\\" c");
        assert_eq!(blank_quotes(r#""""#), "\"…\"");
        assert_eq!(blank_quotes("é \"ñ\" ü"), "é \"…\" ü");
    }

    #[test]
    fn placeholders_are_blanked() {
        let outline = blank_placeholders("I have <count> <things>");
        assert_eq!(outline, "I have <…> <…>");
        assert_eq!(blank_placeholders("a <> b"), "a <> b");
        assert_eq!(blank_placeholders("a <<b> c"), "a <…> c");
        assert_eq!(blank_placeholders("a < b"), "a < b");
        assert_eq!(blank_placeholders("a <b c > d <e"), "a <…> d <e");
        assert_eq!(blank_placeholders("é<ñ>ü"), "é<…>ü");
    }

    #[test]
    fn numbers_standing_alone_are_blanked() {
        assert_eq!(blank_numbers("I have 3 drafts"), "I have # drafts");
        let numbers = blank_numbers("12 and 1.5 and 0.25,7");
        assert_eq!(numbers, "# and # and #,#");
        for same in ["v2", "3d", "a_4", "5_", "é1", "1é", "x"] {
            assert_eq!(blank_numbers(same), same);
        }
        assert_eq!(blank_numbers("1.5a and 1.2.3"), "#.5a and #.#");
        assert_eq!(blank_numbers("10."), "#.");
        assert_eq!(blank_numbers("(42)"), "(#)");
        assert_eq!(blank_numbers("7"), "#");
        assert_eq!(blank_numbers(""), "");
    }

    #[test]
    fn data_is_blanked_quotes_first() {
        let feature = "Given \"a 1 <b>\" with 2 <items>\nThen the 3rd costs 4.50\n";
        let expected = set(&["\"…\" with # <…>", "the 3rd costs #"]);
        assert_eq!(phrasings(&[feature]), expected);
    }

    #[test]
    fn hidden_sentences_must_be_visible_ones_with_other_data() {
        let visible = [
            "Given the change \"c1\" has 2 drafts\n",
            "Then it fails with a message that contains \"x\"\n",
        ];
        let hidden = "Given the change \"c9\" has 7 drafts\nWhen a brand new sentence\n\
                      Then it fails with a message that contains \"y\"\nAnd one more 3\n";
        assert_eq!(
            unknown(&[hidden], &visible),
            ["a brand new sentence", "one more #"]
        );
        assert_eq!(unknown(&[visible[0]], &visible), Vec::<String>::new());
        assert_eq!(unknown(&[], &visible), Vec::<String>::new());
    }
}
