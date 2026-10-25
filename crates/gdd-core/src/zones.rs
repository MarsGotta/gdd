//! The zone map: which paths belong to which color.

use serde::Deserialize;

use crate::config::{Color, Error};

/// Whether `path` matches `pattern`. `**` crosses folders, `*` and `?` stay inside one.
/// `docs/**` matches everything inside `docs`; `**/x` also matches `x` at the root.
pub fn glob_matches(pattern: &str, path: &str) -> bool {
    let tokens = tokenize(pattern);
    let text: Vec<char> = path.chars().collect();
    match_tokens(&tokens, &text)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Literal(char),
    /// `?`: one character that is not `/`.
    One,
    /// `*`: any run of characters without `/`.
    Star,
    /// `**/`: any number of folders, possibly none.
    Folders,
    /// `**`: any run of characters, `/` included.
    Deep,
}

fn tokenize(pattern: &str) -> Vec<Token> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i..].starts_with(&['*', '*', '/']) {
            tokens.push(Token::Folders);
            i += 3;
        } else if chars[i..].starts_with(&['*', '*']) {
            tokens.push(Token::Deep);
            i += 2;
        } else {
            tokens.push(match chars[i] {
                '*' => Token::Star,
                '?' => Token::One,
                c => Token::Literal(c),
            });
            i += 1;
        }
    }
    tokens
}

fn match_tokens(tokens: &[Token], text: &[char]) -> bool {
    let Some((token, rest)) = tokens.split_first() else {
        return text.is_empty();
    };
    match token {
        Token::Literal(c) => text.first() == Some(c) && match_tokens(rest, &text[1..]),
        Token::One => text.first().is_some_and(|&c| c != '/') && match_tokens(rest, &text[1..]),
        Token::Star => {
            let run = text.iter().take_while(|&&c| c != '/').count();
            (0..=run).any(|n| match_tokens(rest, &text[n..]))
        }
        Token::Folders => {
            match_tokens(rest, text)
                || text
                    .iter()
                    .enumerate()
                    .any(|(i, &c)| c == '/' && match_tokens(rest, &text[i + 1..]))
        }
        Token::Deep => (0..=text.len()).any(|n| match_tokens(rest, &text[n..])),
    }
}

/// One zone: a color for a set of path patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    /// Color of the paths this zone matches.
    pub color: Color,
    /// Glob patterns, see [`glob_matches`].
    pub paths: Vec<String>,
    /// Why these paths have this color (empty when the map does not say).
    pub reason: String,
}

/// The zone map: a default color and the zones that override it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneMap {
    /// Color of a path that no zone matches.
    pub default: Color,
    /// Zones, in file order.
    pub zones: Vec<Zone>,
}

#[derive(Deserialize)]
struct RawMap {
    default: Option<String>,
    #[serde(default)]
    zone: Vec<RawZone>,
}

#[derive(Deserialize)]
struct RawZone {
    color: Option<String>,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    reason: String,
}

impl ZoneMap {
    /// Reads a zone map from TOML text (`default` and `[[zone]]` tables).
    /// A missing default is yellow; a zone without color takes the default.
    /// An unknown color, in the default or in a zone, is an error.
    pub fn from_toml(text: &str) -> Result<ZoneMap, Error> {
        let raw: RawMap = toml::from_str(text).map_err(|e| Error::Toml(e.to_string()))?;
        let default = match raw.default {
            Some(name) => name.parse()?,
            None => Color::Yellow,
        };
        let zones = raw
            .zone
            .into_iter()
            .map(|z| {
                let color = match z.color {
                    Some(name) => name.parse()?,
                    None => default,
                };
                Ok(Zone {
                    color,
                    paths: z.paths,
                    reason: z.reason,
                })
            })
            .collect::<Result<_, Error>>()?;
        Ok(ZoneMap { default, zones })
    }

    /// The zone that decides the color of one path: among the zones that match it, the first
    /// one with the highest color. `None` when no zone matches.
    pub fn zone_of(&self, path: &str) -> Option<&Zone> {
        self.zones
            .iter()
            .filter(|z| z.paths.iter().any(|p| glob_matches(p, path)))
            .rev()
            .max_by_key(|z| z.color)
    }

    /// Color of one path: the highest color among the zones that match it, or the default.
    pub fn color_of(&self, path: &str) -> Color {
        self.zone_of(path).map_or(self.default, |z| z.color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_star_stays_inside_a_folder() {
        assert!(glob_matches("*.md", "readme.md"));
        assert!(!glob_matches("*.md", "notes/readme.md"));
        assert!(glob_matches("src/*/mod.rs", "src/a/mod.rs"));
        assert!(!glob_matches("src/*/mod.rs", "src/a/b/mod.rs"));
    }

    #[test]
    fn two_stars_cross_folders() {
        assert!(glob_matches("docs/**", "docs/a/b/c.txt"));
        assert!(glob_matches("docs/**", "docs/a.txt"));
        assert!(!glob_matches("docs/**", "other/docs/a.txt"));
    }

    #[test]
    fn two_stars_inside_a_name_cross_folders_and_keep_what_follows() {
        assert!(glob_matches("docs/**.md", "docs/a/b.md"));
        assert!(glob_matches("docs/**.md", "docs/.md"));
        assert!(!glob_matches("docs/**.md", "docs/a/b.txt"));
    }

    #[test]
    fn leading_double_star_folder_may_be_empty() {
        assert!(glob_matches("**/x.rs", "x.rs"));
        assert!(glob_matches("**/x.rs", "a/b/x.rs"));
        assert!(!glob_matches("**/x.rs", "a/bx.rs"));
        assert!(glob_matches("a/**/z", "a/z"));
        assert!(glob_matches("a/**/z", "a/b/c/z"));
    }

    #[test]
    fn question_mark_is_one_character_but_not_a_slash() {
        assert!(glob_matches("a?c", "abc"));
        assert!(!glob_matches("a?c", "a/c"));
        assert!(!glob_matches("a?c", "ac"));
    }

    #[test]
    fn other_characters_are_literal() {
        assert!(glob_matches("a.b", "a.b"));
        assert!(!glob_matches("a.b", "axb"));
        assert!(glob_matches("a+(b)", "a+(b)"));
    }

    #[test]
    fn the_whole_path_must_match() {
        assert!(!glob_matches("docs", "docs/a"));
        assert!(!glob_matches("*.md", "a.md.bak"));
        assert!(glob_matches("", ""));
        assert!(!glob_matches("", "a"));
    }

    #[test]
    fn highest_zone_color_wins_and_default_fills_the_rest() {
        let map = ZoneMap::from_toml(
            r#"
            default = "yellow"
            [[zone]]
            color = "green"
            paths = ["src/**"]
            [[zone]]
            color = "red"
            paths = ["src/secret/**"]
            "#,
        )
        .unwrap();
        assert_eq!(map.color_of("src/secret/key.rs"), Color::Red);
        assert_eq!(map.color_of("src/a.rs"), Color::Green);
        assert_eq!(map.color_of("other.rs"), Color::Yellow);
    }

    #[test]
    fn the_first_zone_with_the_highest_color_decides() {
        let map = ZoneMap::from_toml(
            r#"
            [[zone]]
            color = "green"
            paths = ["src/**"]
            reason = "code"
            [[zone]]
            color = "red"
            paths = ["src/a/**"]
            reason = "first red"
            [[zone]]
            color = "red"
            paths = ["src/a/b.rs"]
            reason = "second red"
            "#,
        )
        .unwrap();
        assert_eq!(map.zone_of("src/a/b.rs").unwrap().reason, "first red");
        assert_eq!(map.zone_of("src/c.rs").unwrap().reason, "code");
        assert_eq!(map.zone_of("other.rs"), None);
    }

    #[test]
    fn missing_default_is_yellow() {
        assert_eq!(ZoneMap::from_toml("").unwrap().default, Color::Yellow);
    }

    #[test]
    fn unknown_colors_are_errors() {
        let zone = "[[zone]]\ncolor = \"purple\"\npaths = []\n";
        assert_eq!(
            ZoneMap::from_toml(zone),
            Err(Error::UnknownColor("purple".into()))
        );
        assert_eq!(
            ZoneMap::from_toml("default = \"blue\""),
            Err(Error::UnknownColor("blue".into()))
        );
    }

    #[test]
    fn malformed_toml_is_an_error() {
        assert!(matches!(
            ZoneMap::from_toml("default = "),
            Err(Error::Toml(_))
        ));
    }
}
