//! What a launched role says at the end: its `gdd-report` block, a `CANNOT:`, and for a reviewer
//! its verdict, its color, its findings and what it leaves for later.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Finding {
    pub id: String,
    pub place: String,
    pub what: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoleReport {
    pub skill: String,
    pub version: Option<String>,
    pub result: String,
    pub caught: String,
    /// The reason and what would unblock it, when the role stopped with `CANNOT:`.
    pub cannot: Option<(String, String)>,
    /// The reviewer's first word: `pass`, `fix` or `stop`.
    pub verdict: Option<String>,
    /// The color of the `COLOR:` line, and the rest of that line.
    pub color: Option<(String, String)>,
    pub findings: Vec<Finding>,
    pub later: Vec<String>,
    pub notes: String,
    pub missing: Vec<String>,
    /// The JSON list in the last `json` block before the report block: the checks role's proofs.
    pub proofs: Option<serde_json::Value>,
    /// Why that block couldn't be read, when it was there.
    pub proofs_error: Option<String>,
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
            .map_err(|e| format!("its json block isn't JSON: {e}"))
            .and_then(|v| {
                let ok = v.as_array().is_some_and(|a| {
                    a.iter().all(|p| {
                        p.get("check").is_some_and(|c| c.is_string())
                            && p.get("case").is_some_and(|c| c.is_string())
                    })
                });
                if ok {
                    Ok(v)
                } else {
                    Err("its json block isn't a list of proofs, each with check and case".into())
                }
            }),
    )
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
        if let Some((k, v)) = l.split_once(':') {
            out.push((k.trim().to_lowercase(), v.trim().to_string()));
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

/// Read what a role said. Without its `gdd-report` block the step didn't happen.
pub fn parse(text: &str) -> Result<RoleReport, String> {
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
    let mut in_block = false;
    for l in &lines {
        if l.trim().starts_with("```") {
            in_block = !in_block;
            continue;
        }
        if in_block {
            continue;
        }
        if let Some(f) = finding(l)
            && !r.findings.iter().any(|x| x.id == f.id)
        {
            r.findings.push(f);
        }
    }
    r.later = section(
        text,
        "LATER:",
        &["NOTES FOR THE BUILDER:", "MISSING:", "COLOR:"],
    );
    r.notes = section(text, "NOTES FOR THE BUILDER:", &["LATER:", "MISSING:"]).join("\n");
    r.missing = section(text, "MISSING:", &["LATER:", "NOTES FOR THE BUILDER:"]);
    match json_block(text) {
        Some(Ok(v)) => r.proofs = Some(v),
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
