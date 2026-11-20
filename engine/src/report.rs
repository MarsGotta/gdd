//! What a launched role says at the end: its `gdd-report` block, a `CANNOT:`, and for a reviewer
//! its verdict, its color, its findings and what it leaves for later.

use crate::blueprint::Blueprint;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Finding {
    pub id: String,
    pub place: String,
    pub what: String,
}

/// The command a finding ends with, `| ran: <command>` in any case, and what comes before it. The
/// last `| ran:` splits them, since the command itself can hold a pipe.
pub(crate) fn ran_of(what: &str) -> Option<(&str, &str)> {
    // ASCII lowercase keeps every byte where it was.
    let lower = what.to_ascii_lowercase();
    let (bar, rest) = lower.rmatch_indices('|').find_map(|(i, _)| {
        lower[i + 1..]
            .trim_start()
            .strip_prefix("ran:")
            .map(|rest| (i, rest))
    })?;
    let command = what[what.len() - rest.len()..].trim();
    (!command.is_empty()).then_some((&what[..bar], command))
}

/// A command as a finding or a json block gives it, ready to compare: without the quotes or
/// backticks around it, a trailing stop or "(exit N)", and with single spaces.
pub(crate) fn plain_command(c: &str) -> String {
    let c = c.trim().trim_end_matches(['.', ',', ';']).trim_end();
    let c = match c.rsplit_once('(') {
        Some((before, note)) if note.starts_with("exit") && note.ends_with(')') => before,
        _ => c,
    };
    c.trim()
        .trim_end_matches(['.', ',', ';'])
        .trim()
        .trim_matches(['`', '"', '\''])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The reviewer judges whether the cited requirement is broken. The layer checks that the
/// finding itself gives a source and some words from it.
fn has_citation(what: &str, blueprint: Option<&Blueprint>) -> bool {
    // What a finding ran is no source: a command can name the scenario it greps for.
    let what = ran_of(what).map_or(what, |(rest, _)| rest);
    // Any segment after the first `|`, so that another one after the citation keeps it.
    if what.split('|').skip(1).any(|source| {
        source
            .trim()
            .strip_prefix("check ")
            .and_then(|rest| rest.split_whitespace().next())
            .map(|word| word.trim_matches(['`', '"', '(', ')', '[', ']', ',', '.', ':']))
            .is_some_and(|id| {
                crate::valid_id(id) && blueprint.is_none_or(|bp| bp.check(id).is_some())
            })
    }) {
        return true;
    }
    let mut words = what
        .split_whitespace()
        .map(|w| w.trim_matches(['`', '"', '(', ')', '[', ']', ',', '.']));
    while let Some(word) = words.next() {
        if word.strip_prefix("@id:").is_some_and(crate::valid_id) {
            return true;
        }
        let mut rest = words.clone();
        let source = match word {
            "constitution:" | "request:" => true,
            "zone" => rest
                .next()
                .and_then(|w| w.strip_suffix(':'))
                .is_some_and(crate::valid_id),
            _ => false,
        };
        if source && rest.any(|w| w.chars().any(char::is_alphanumeric)) {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoleReport {
    pub skill: String,
    pub version: Option<String>,
    pub result: String,
    pub caught: String,
    pub decisions: Option<Vec<String>>,
    /// A builder's `contested:` lines, as it gave them: `<finding> | <file>:<line> | <why>`.
    pub contested: Vec<String>,
    /// The reason and what would unblock it, when the role stopped with `CANNOT:`.
    pub cannot: Option<(String, String)>,
    /// The reviewer's first word: `pass`, `fix` or `stop`.
    pub verdict: Option<String>,
    /// The color of the `COLOR:` line, and the rest of that line.
    pub color: Option<(String, String)>,
    pub findings: Vec<Finding>,
    pub closed: Option<String>,
    pub reviewed: Option<String>,
    /// Distinct finding numbers without a citation in a fix.
    pub uncited: Vec<String>,
    pub later: Vec<String>,
    /// What a reviewer left for the person who approves the change, a line each, or why it couldn't.
    pub evidence: Vec<String>,
    pub notes: String,
    pub missing: Vec<String>,
    /// The JSON list in the last `json` block before the report block: the checks role's proofs.
    pub proofs: Option<serde_json::Value>,
    /// Why that block couldn't be read, when it was there.
    pub proofs_error: Option<String>,
    /// The same block as the role gave it, when it is a JSON list of objects: what a builder
    /// says each check gave, or the commands a reviewer ran.
    pub list: Option<serde_json::Value>,
}

fn plain(line: &str) -> String {
    line.trim()
        .trim_matches(|c| c == '*' || c == '`' || c == '_' || c == '#')
        .trim()
        .to_string()
}

/// The last `gdd-report` block of a text, as its `key: value` lines.
/// The last fenced `json` block that closes before the `gdd-report` block, read as JSON.
fn json_block(text: &str) -> Option<Result<serde_json::Value, String>> {
    let lines: Vec<&str> = text.lines().collect();
    let report = lines
        .iter()
        .rposition(|l| {
            l.trim().starts_with("```") && l.trim().trim_start_matches('`').trim() == "gdd-report"
        })
        .unwrap_or(lines.len());
    let mut found = None;
    let mut i = 0;
    while i < report {
        let l = lines[i].trim();
        if l.starts_with("```")
            && l.trim_start_matches('`')
                .trim()
                .eq_ignore_ascii_case("json")
        {
            let mut body = vec![];
            let mut j = i + 1;
            while j < report && !lines[j].trim().starts_with("```") {
                body.push(lines[j]);
                j += 1;
            }
            if j < report {
                found = Some(body.join("\n"));
            }
            i = j + 1;
            continue;
        }
        i += 1;
    }
    let body = found?;
    Some(
        serde_json::from_str::<serde_json::Value>(&body)
            .map_err(|e| format!("its json block isn't JSON: {e}")),
    )
}

/// That block as the checks role's proofs: a list, each with `check` and `case`.
fn proofs_of(v: &serde_json::Value) -> Result<serde_json::Value, String> {
    let ok = v.as_array().is_some_and(|a| {
        a.iter().all(|p| {
            p.get("check").is_some_and(|c| c.is_string())
                && p.get("case").is_some_and(|c| c.is_string())
        })
    });
    if ok {
        Ok(v.clone())
    } else {
        Err("its json block isn't a list of proofs, each with check and case".into())
    }
}

fn block(text: &str) -> Option<Vec<(String, String)>> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().rposition(|l| {
        l.trim().trim_start_matches('`').trim() == "gdd-report" && l.trim().starts_with("```")
    })?;
    let mut out = vec![];
    for l in &lines[start + 1..] {
        if l.trim().starts_with("```") {
            return Some(out);
        }
        if let Some((k, mut value)) = l.split_once(':') {
            let mut key = k.trim().to_lowercase();
            let numbered = key
                .split_once('.')
                .filter(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
            let label = numbered
                .map_or(key.as_str(), |(_, rest)| rest)
                .trim_matches(['-', '+', '*', '_', ' ']);
            if matches!(label, "decision" | "decisions") {
                // A bold label may enclose its colon, as in **decision:**.
                if let Some(prefix) = key.strip_suffix(label)
                    && let Some(mark) = ["**", "__"].into_iter().find(|m| prefix.ends_with(m))
                {
                    value = value.strip_prefix(mark).unwrap_or(value);
                }
                key = "decision".into();
            }
            out.push((key, value.trim().to_string()));
        }
    }
    // A block cut off before its closing fence still says what it says.
    Some(out)
}

fn section(text: &str, header: &str, stops: &[&str]) -> Vec<String> {
    let mut out = vec![];
    let mut inside = false;
    for l in text.lines() {
        let p = plain(l);
        if let Some(rest) = p.strip_prefix(header) {
            inside = true;
            let rest = rest.trim();
            if !rest.is_empty() {
                out.push(rest.to_string());
            }
            continue;
        }
        if inside {
            if l.trim().starts_with("```") || stops.iter().any(|s| p.starts_with(s)) {
                break;
            }
            if p.is_empty() {
                if out.is_empty() {
                    continue;
                }
                break;
            }
            out.push(p.trim_start_matches(['-', '*', ' ']).trim().to_string());
        }
    }
    out
}

fn finding(line: &str) -> Option<Finding> {
    let p = plain(line);
    let p = p
        .trim_start_matches(['-', '*', ' '])
        .trim_start_matches('*');
    let mut chars = p.char_indices();
    let (_, f) = chars.next()?;
    if f != 'f' {
        return None;
    }
    let digits: String = p[1..].chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let id = format!("f{digits}");
    let rest = p[1 + digits.len()..]
        .trim_start_matches(['*', ':', '.', ')', '-', ' ', '\u{2013}'])
        .trim();
    if rest.is_empty() {
        return None;
    }
    let mut place = String::new();
    let mut what = rest.to_string();
    if let Some(first) = rest.split_whitespace().next() {
        let w = first.trim_matches(|c| c == '`' || c == ',' || c == ':' || c == '(' || c == ')');
        let looks = w.contains('/') || w.contains(':') || (w.contains('.') && !w.ends_with('.'));
        if looks && w.chars().any(|c| c.is_ascii_alphabetic()) {
            place = w.to_string();
            what = rest[first.len()..]
                .trim_start_matches([':', ' ', '-', '\u{2013}'])
                .trim()
                .to_string();
        }
    }
    Some(Finding { id, place, what })
}

/// The lists a report gives a line per item: what is left for later, and the evidence for the
/// person who approves the change.
const LISTS: [&str; 2] = ["LATER:", "EVIDENCE:"];

fn list_of<'a>(r: &'a mut RoleReport, header: &str) -> &'a mut Vec<String> {
    if header == "LATER:" {
        &mut r.later
    } else {
        &mut r.evidence
    }
}

/// A numbered line inside LATER belongs to the log. After a blank line it can bind the change.
/// EVIDENCE ends at a blank line or at a finding, which it would otherwise swallow.
fn read_findings(lines: &[&str], r: &mut RoleReport, blueprint: Option<&Blueprint>) {
    let mut in_block = false;
    // The list being read, and how many items it had when its header came.
    let mut open: Option<(&str, usize)> = None;
    for l in lines {
        if l.trim().starts_with("```") {
            in_block = !in_block;
            open = None;
            continue;
        }
        if in_block {
            continue;
        }
        let p = plain(l);
        if let Some((header, rest)) = LISTS
            .iter()
            .find_map(|h| p.strip_prefix(h).map(|rest| (*h, rest.trim())))
        {
            let items = list_of(r, header);
            open = Some((header, items.len()));
            if !rest.is_empty() {
                items.push(rest.to_string());
            }
            continue;
        }
        if let Some((header, start)) = open {
            let evidence = header == "EVIDENCE:";
            let items = list_of(r, header);
            if (l.trim().is_empty() && (evidence || items.len() > start))
                || (evidence && finding(l).is_some())
                || ["NOTES FOR THE BUILDER:", "MISSING:", "COLOR:"]
                    .iter()
                    .any(|h| p.starts_with(h))
            {
                open = None;
            } else {
                if !p.is_empty() {
                    items.push(p.trim_start_matches(['-', '*', ' ']).trim().to_string());
                }
                continue;
            }
        }
        if let Some(f) = finding(l) {
            if r.verdict.as_deref() == Some("fix")
                && !r.uncited.contains(&f.id)
                && !has_citation(&f.what, blueprint)
            {
                r.uncited.push(f.id.clone());
            }
            if !r.findings.iter().any(|x| x.id == f.id) {
                r.findings.push(f);
            }
        }
    }
}

/// Bound a role's lines that reach another agent or a person, the builder's product choices and
/// the reviewer's evidence, at both entry points: a new report and a list read from a log.
pub(crate) fn bounded_lines<'a>(values: impl Iterator<Item = &'a str>) -> Vec<String> {
    values
        .filter_map(|value| {
            let clean: String = value
                .trim()
                .chars()
                .filter(|c| !c.is_control() && !matches!(c, '\u{200b}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
                .take(240)
                .collect();
            let choice = clean.trim();
            (!choice.is_empty()
                && !choice
                    .strip_suffix('.')
                    .unwrap_or(choice)
                    .eq_ignore_ascii_case("none"))
            .then(|| choice.to_string())
        })
        .take(12)
        .collect()
}

/// Read what a role said. Without its `gdd-report` block the step didn't happen.
pub fn parse(text: &str) -> Result<RoleReport, String> {
    parse_with_blueprint(text, None)
}

pub(crate) fn parse_with_blueprint(
    text: &str,
    blueprint: Option<&Blueprint>,
) -> Result<RoleReport, String> {
    let Some(fields) = block(text) else {
        return Err("it has no gdd-report block: the step didn't happen".into());
    };
    let get = |k: &str| {
        fields
            .iter()
            .rev()
            .find(|(x, _)| x == k)
            .map(|(_, v)| v.clone())
    };
    let skill_line = get("skill").ok_or("its gdd-report block has no skill line")?;
    let mut words = skill_line.split_whitespace();
    let skill = words.next().unwrap_or("").to_string();
    let version = words.next().map(str::to_string);
    let mut r = RoleReport {
        skill,
        version,
        result: get("result").ok_or("its gdd-report block has no result line")?,
        caught: get("caught").unwrap_or_else(|| "none".into()),
        ..RoleReport::default()
    };
    let mut choices = fields
        .iter()
        .filter(|(k, _)| k == "decision")
        .map(|(_, v)| v.as_str())
        .peekable();
    if choices.peek().is_some() {
        r.decisions = Some(bounded_lines(choices));
    }
    r.contested = fields
        .iter()
        .filter(|(k, v)| k == "contested" && !v.is_empty() && !v.eq_ignore_ascii_case("none"))
        .map(|(_, v)| v.clone())
        .collect();
    let closed: Vec<_> = fields
        .iter()
        .filter(|(k, _)| k == "closed")
        .map(|(_, v)| v.as_str())
        .collect();
    r.closed = (!closed.is_empty()).then(|| closed.join(","));
    r.reviewed = get("reviewed")
        .map(|v| {
            v.replace('`', "")
                .trim()
                .trim_end_matches(['.', ',', ';', ':', '!', '?'])
                .trim_end()
                .to_owned()
        })
        .filter(|v| {
            v.split(|c: char| !c.is_ascii_hexdigit())
                .any(|hex| hex.len() == 40)
        });
    let lines: Vec<&str> = text.lines().collect();
    if let Some(i) = lines.iter().position(|l| plain(l).starts_with("CANNOT:")) {
        let reason = plain(lines[i])
            .trim_start_matches("CANNOT:")
            .trim()
            .to_string();
        let needs = lines[i + 1..]
            .iter()
            .map(|l| plain(l))
            .take_while(|l| !l.starts_with("```"))
            .find(|l| !l.is_empty())
            .unwrap_or_default();
        r.cannot = Some((reason, needs));
    }
    if let Some(first) = lines.iter().map(|l| plain(l)).find(|l| !l.is_empty()) {
        let w = first
            .split(|c: char| c.is_whitespace() || c == ':' || c == '.' || c == ',')
            .next()
            .unwrap_or("");
        if ["pass", "fix", "stop"].contains(&w) {
            r.verdict = Some(w.to_string());
        }
    }
    if let Some(line) = lines
        .iter()
        .map(|l| plain(l))
        .find(|l| l.starts_with("COLOR:"))
    {
        let rest = line.trim_start_matches("COLOR:").trim();
        let color = rest
            .split(|c: char| !c.is_ascii_alphabetic())
            .next()
            .unwrap_or("")
            .to_lowercase();
        if crate::color_rank(&color).is_some() {
            r.color = Some((
                color.clone(),
                rest[color.len().min(rest.len())..]
                    .trim_start_matches([',', ':', ' ', '-'])
                    .trim()
                    .to_string(),
            ));
        }
    }
    read_findings(&lines, &mut r, blueprint);
    r.evidence = bounded_lines(r.evidence.iter().map(String::as_str));
    r.notes = section(
        text,
        "NOTES FOR THE BUILDER:",
        &["LATER:", "MISSING:", "EVIDENCE:"],
    )
    .join("\n");
    r.missing = section(
        text,
        "MISSING:",
        &["LATER:", "NOTES FOR THE BUILDER:", "EVIDENCE:"],
    );
    match json_block(text) {
        Some(Ok(v)) => {
            if v.as_array()
                .is_some_and(|a| !a.is_empty() && a.iter().all(|x| x.is_object()))
            {
                r.list = Some(v.clone());
            }
            match proofs_of(&v) {
                Ok(p) => r.proofs = Some(p),
                Err(e) => r.proofs_error = Some(e),
            }
        }
        Some(Err(e)) => r.proofs_error = Some(e),
        None => {}
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REVIEW: &str = "fix\nCOLOR: yellow, the zone shop\nI ran the tests: OK.\n\nf1 src/cart.py:4 apply_code answers from a table of the tests' own numbers\n- **f2**: tests/test_cart.py:13: a test was deleted\n\nNOTES FOR THE BUILDER:\nCompute 10% for any total.\n\nLATER:\n- the same formatting is written twice\n\n```gdd-report\nskill: gdd-review 0.10.1\nresult: fix\ncaught: a table of the tests' numbers\n```\n";

    #[test]
    fn a_review() {
        let r = parse(REVIEW).unwrap();
        assert_eq!(r.skill, "gdd-review");
        assert_eq!(r.version.as_deref(), Some("0.10.1"));
        assert_eq!(r.verdict.as_deref(), Some("fix"));
        assert_eq!(r.color.as_ref().unwrap().0, "yellow");
        assert_eq!(r.findings.len(), 2);
        assert_eq!(r.findings[0].place, "src/cart.py:4");
        assert_eq!(r.findings[1].id, "f2");
        assert_eq!(r.findings[1].place, "tests/test_cart.py:13");
        assert_eq!(r.later, vec!["the same formatting is written twice"]);
        assert_eq!(r.notes, "Compute 10% for any total.");
    }

    #[test]
    fn a_citation_holds_before_what_the_finding_ran() {
        let t = "fix\nCOLOR: red\nf1 a.py:1 charges twice | check tests | ran: `sh t.sh`\nf2 a.py:2 no total | ran: grep @id:p-1 features\nf3 a.py:3 rounds up | @id:p-2\n```gdd-report\nskill: gdd-review 1\nresult: fix\ncaught: none\n```\n";
        let r = parse(t).unwrap();
        assert_eq!(r.uncited, vec!["f2"]);
        assert_eq!(
            plain_command(ran_of(&r.findings[0].what).unwrap().1),
            "sh t.sh"
        );
        assert!(ran_of(&r.findings[2].what).is_none());
    }

    #[test]
    fn a_run_reads_the_same_whatever_its_quotes_case_or_pipes() {
        for what in [
            "it fails | @id:p-1 | ran: \"sh t.sh | tail -3\"",
            "it fails | @id:p-1 | ran: sh t.sh | tail -3.",
            "it fails | @id:p-1 | Ran: sh  t.sh | tail -3",
            "it fails | @id:p-1 | RAN: `sh t.sh | tail -3` (exit 1)",
        ] {
            let (rest, command) = ran_of(what).unwrap();
            assert_eq!(rest.trim(), "it fails | @id:p-1", "{what}");
            assert_eq!(plain_command(command), "sh t.sh | tail -3", "{what}");
        }
    }

    #[test]
    fn evidence_ends_at_a_blank_line_or_a_finding() {
        let t = "fix\nCOLOR: red\nEVIDENCE: none, no browser here\nf1 a.py:1 charges twice | @id:p-1\nEVIDENCE:\n- before.png the cart\n\nI ran it.\n```gdd-report\nskill: gdd-review 1\nresult: fix\ncaught: none\n```\n";
        let r = parse(t).unwrap();
        assert_eq!(r.findings.len(), 1);
        assert_eq!(
            r.evidence,
            vec!["none, no browser here", "before.png the cart"]
        );
    }

    #[test]
    fn evidence_is_a_list_like_later() {
        let t = "pass\nCOLOR: red\nEVIDENCE:\n- /p/.gdd/evidence/x/before.png the cart at <start>\n- /p/.gdd/evidence/x/after.png the cart now\nLATER:\n- a duplicated helper\n\nNOTES FOR THE BUILDER:\nnone\n```gdd-report\nskill: gdd-review 1\nresult: pass\ncaught: none\n```\n";
        let r = parse(t).unwrap();
        assert_eq!(r.evidence.len(), 2);
        assert!(r.evidence[1].ends_with("the cart now"));
        assert_eq!(r.later, vec!["a duplicated helper"]);
        let why = parse(&t.replace(
            "EVIDENCE:\n- /p/.gdd/evidence/x/before.png the cart at <start>\n- /p/.gdd/evidence/x/after.png the cart now\n",
            "MISSING:\n- a gap\nEVIDENCE: none, no browser here\n",
        ))
        .unwrap();
        assert_eq!(why.evidence, vec!["none, no browser here"]);
        assert_eq!(why.missing, vec!["a gap"]);
    }

    #[test]
    fn proofs_in_a_json_block() {
        let t = "Sealed.\n```json\n[{\"check\":\"tests\",\"case\":\"rejects\",\"command\":\"make test\",\"exit\":1,\"tail\":\"FAILED\"}]\n```\nseen rejecting, not seen passing\n```gdd-report\nskill: gdd-checks 0.11.0\nresult: sealed\ncaught: none\n```\n";
        let r = parse(t).unwrap();
        assert_eq!(r.proofs.unwrap()[0]["exit"], 1);
        let bad = t.replace("[{", "{[");
        assert!(parse(&bad).unwrap().proofs_error.is_some());
        let none =
            parse("```gdd-report\nskill: gdd-checks 1\nresult: sealed\ncaught: none\n```").unwrap();
        assert!(none.proofs.is_none() && none.proofs_error.is_none());
    }

    #[test]
    fn what_a_builder_or_a_reviewer_ran_in_a_json_block() {
        let t = "pass\nCOLOR: green\n```json\n[{\"command\":\"npm test\",\"exit\":0,\"tail\":\"22 passed\"}]\n```\n```gdd-report\nskill: gdd-review 0.11.2\nresult: pass\ncaught: none\n```\n";
        let r = parse(t).unwrap();
        assert_eq!(r.list.unwrap()[0]["command"], "npm test");
        // It isn't a list of proofs, and for a role that owes none that is no error to act on.
        assert!(r.proofs.is_none());
    }

    #[test]
    fn a_cannot_and_a_missing_block() {
        let t = "CANNOT: src/texts/empty-cart.txt is not in my paths\nAdd it to the brief.\n```gdd-report\nskill: gdd-build 0.10.1\nresult: stopped\ncaught: none\n```";
        let r = parse(t).unwrap();
        assert_eq!(
            r.cannot.as_ref().unwrap().0,
            "src/texts/empty-cart.txt is not in my paths"
        );
        assert_eq!(r.cannot.as_ref().unwrap().1, "Add it to the brief.");
        assert!(r.verdict.is_none());
        assert!(parse("all done!").is_err());
    }
}
