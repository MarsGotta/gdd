//! The backlog, `gdd/backlog.md`: items, each with its expected color and its state, `todo`,
//! `briefed` or `done`. The contract writes it as a Markdown table; the reader also takes what an
//! agent may write instead: tables under headings of any level (a heading `Phase N` gives the
//! phase of the items under it), several tables, columns renamed or added, ids in backticks or
//! bold, or a list of items. What it can't read it names, line by line, instead of skipping it.
//! Only the line of the item that changes is rewritten.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    pub line: usize,
    pub id: String,
    pub what: String,
    pub color: String,
    pub phase: String,
    pub brief: String,
    pub state: String,
    /// How its line is written: a table's row (the field of each column), or a list's item.
    pub form: Form,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Form {
    Table(Vec<Option<&'static str>>),
    #[default]
    List,
}

/// A line of the backlog the reader can't take as an item, and why. Line 0 is the whole file.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub line: usize,
    pub what: String,
}

impl Problem {
    pub fn say(&self) -> String {
        if self.line == 0 {
            format!("gdd/backlog.md: {}", self.what)
        } else {
            format!("gdd/backlog.md, line {}: {}", self.line, self.what)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Backlog {
    pub items: Vec<Item>,
    pub problems: Vec<Problem>,
}

pub const STATES: [&str; 3] = ["todo", "briefed", "done"];

/// Each field of an item, and the names its column goes by.
const FIELDS: [(&str, &[&str]); 6] = [
    ("id", &["id", "item", "item id", "key"]),
    (
        "what",
        &[
            "what",
            "description",
            "title",
            "summary",
            "qué",
            "que",
            "descripción",
            "descripcion",
        ],
    ),
    ("color", &["color", "colour", "expected color"]),
    ("phase", &["phase", "fase"]),
    ("brief", &["brief"]),
    ("state", &["state", "status", "estado"]),
];

const SHAPE: &str = "the contract's table has the columns Id, What, Color, Phase, Brief and State";

fn cells(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    if !t.starts_with('|') || !t.ends_with('|') || t.len() < 2 {
        return None;
    }
    Some(
        t[1..t.len() - 1]
            .split('|')
            .map(|c| c.trim().to_string())
            .collect(),
    )
}

fn separator(row: &[String]) -> bool {
    row.iter()
        .all(|c| c.chars().all(|x| x == '-' || x == ':' || x == ' '))
}

/// A cell or a word without its decoration: backticks, bold, italics, a link's target.
fn clean(s: &str) -> String {
    let mut t = s.trim().to_string();
    if let (Some(0), Some(b)) = (t.find('['), t.find("]("))
        && t.ends_with(')')
    {
        t = t[1..b].to_string();
    }
    loop {
        let before = t.clone();
        for m in ["**", "__", "`", "*", "_"] {
            if t.len() >= 2 * m.len() && t.starts_with(m) && t.ends_with(m) {
                t = t[m.len()..t.len() - m.len()].trim().to_string();
            }
        }
        if t == before {
            return t;
        }
    }
}

fn field_of(header: &str) -> Option<&'static str> {
    let h = clean(header).to_lowercase();
    FIELDS
        .iter()
        .find(|(_, names)| names.contains(&h.as_str()))
        .map(|(f, _)| *f)
}

/// The field of each column of a table's header, when it names an id and a state.
fn columns(row: &[String]) -> Option<Vec<Option<&'static str>>> {
    let mut seen = vec![];
    let cols: Vec<Option<&'static str>> = row
        .iter()
        .map(|c| {
            let f = field_of(c).filter(|f| !seen.contains(f));
            if let Some(f) = f {
                seen.push(f);
            }
            f
        })
        .collect();
    (seen.contains(&"id") && seen.contains(&"state")).then_some(cols)
}

/// The phase a heading gives the items under it: `Phase 1`, `Fase 2: ...`.
fn heading_phase(line: &str) -> Option<Option<String>> {
    let t = line.trim_start();
    if !t.starts_with('#') {
        return None;
    }
    let words: Vec<String> = t
        .trim_start_matches('#')
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let phase = words
        .windows(2)
        .find(|w| (w[0] == "phase" || w[0] == "fase") && w[1].chars().all(|c| c.is_ascii_digit()))
        .map(|w| w[1].clone());
    Some(phase)
}

/// A phase as a number when it holds one: `Phase 1` and `1` are both `1`.
fn phase_of(cell: &str) -> String {
    let c = clean(cell);
    let digits: String = c
        .chars()
        .skip_while(|x| !x.is_ascii_digit())
        .take_while(|x| x.is_ascii_digit())
        .collect();
    if digits.is_empty() { c } else { digits }
}

/// Check an item's id and state, and take it or name what is wrong with it.
fn take(b: &mut Backlog, mut item: Item) {
    if !crate::valid_id(&item.id) {
        b.problems.push(Problem {
            line: item.line,
            what: if item.id.is_empty() {
                "an item with no id: each one starts with its id, lowercase letters, digits and hyphens".into()
            } else {
                format!(
                    "\"{}\" is not an id: lowercase letters, digits and hyphens",
                    item.id
                )
            },
        });
        return;
    }
    item.state = item.state.to_lowercase();
    if !STATES.contains(&item.state.as_str()) {
        b.problems.push(Problem {
            line: item.line,
            what: if item.state.is_empty() {
                format!("{} has no state: todo, briefed or done", item.id)
            } else {
                format!(
                    "{}'s state \"{}\" is not todo, briefed or done",
                    item.id, item.state
                )
            },
        });
        return;
    }
    if let Some(first) = b.items.iter().find(|i| i.id == item.id) {
        b.problems.push(Problem {
            line: item.line,
            what: format!("{} is an item already, at line {}", item.id, first.line),
        });
        return;
    }
    b.items.push(item);
}

/// A list's item: `- [ ] `id`: what (color, phase N, state, brief <id>)`. The id comes first, in
/// backticks or bold or followed by a colon; the parentheses at the end hold its fields.
struct ListLine {
    /// The text up to the parentheses of its fields, as written.
    head: String,
    checkbox: Option<bool>,
    id: String,
    what: String,
    fields: Vec<String>,
}

fn list_line(line: &str) -> Option<Result<ListLine, String>> {
    let t = line.trim_start();
    let rest = ["- ", "* ", "+ "]
        .iter()
        .find_map(|b| t.strip_prefix(b))
        .or_else(|| {
            let n = t.chars().take_while(|c| c.is_ascii_digit()).count();
            (n > 0)
                .then(|| t[n..].strip_prefix(". ").or(t[n..].strip_prefix(") ")))
                .flatten()
        })?;
    let (checkbox, rest) = match rest.get(..4) {
        Some("[ ] ") => (Some(false), &rest[4..]),
        Some("[x] ") | Some("[X] ") => (Some(true), &rest[4..]),
        _ => (None, rest),
    };
    let rest = rest.trim_start();
    let (id, after, marked) = if let Some(r) = rest.strip_prefix('`') {
        let end = r.find('`')?;
        (r[..end].to_string(), &r[end + 1..], true)
    } else if let Some(r) = rest.strip_prefix("**") {
        let end = r.find("**")?;
        (clean(&r[..end]), &r[end + 2..], true)
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ':')
            .unwrap_or(rest.len());
        let colon = rest[end..].starts_with(':');
        (rest[..end].to_string(), &rest[end..], colon)
    };
    if !marked {
        // Prose, unless it is a task: a checkbox with no id is an item that can't be read.
        return checkbox.map(|_| {
            Err("a task with no id: an item starts with its id, as `id`: what (color, phase, state)".into())
        });
    }
    let after = after
        .trim_start()
        .trim_start_matches([':', '-', '\u{2014}', '\u{2013}'])
        .trim();
    let (what, fields) = match (after.rfind('('), after.ends_with(')')) {
        (Some(open), true) => (
            after[..open].trim().to_string(),
            after[open + 1..after.len() - 1]
                .split([',', ';'])
                .map(|f| f.trim().to_string())
                .filter(|f| !f.is_empty())
                .collect(),
        ),
        _ => (after.to_string(), vec![]),
    };
    let head = match line.rfind('(') {
        Some(open) if line.trim_end().ends_with(')') && !fields.is_empty() => {
            line[..open].trim_end().to_string()
        }
        _ => line.trim_end().to_string(),
    };
    Some(Ok(ListLine {
        head,
        checkbox,
        id,
        what,
        fields,
    }))
}

/// The field a list item's word or `key: value` gives: its color, phase, state or brief.
fn list_field(f: &str) -> Option<(&'static str, String)> {
    let low = clean(f).to_lowercase();
    if let Some((k, v)) = low.split_once(':').or_else(|| low.split_once(' ')) {
        let v = clean(v);
        return match k.trim() {
            "color" | "colour" => Some(("color", v)),
            "phase" | "fase" => Some(("phase", phase_of(&v))),
            "state" | "status" | "estado" => Some(("state", v)),
            "brief" => Some(("brief", v)),
            _ => None,
        };
    }
    if crate::color_rank(&low).is_some() {
        return Some(("color", low));
    }
    if STATES.contains(&low.as_str()) {
        return Some(("state", low));
    }
    None
}

fn list_item(line: usize, l: &ListLine, phase: &Option<String>) -> Item {
    let mut item = Item {
        line,
        id: l.id.clone(),
        what: l.what.clone(),
        phase: phase.clone().unwrap_or_default(),
        form: Form::List,
        ..Item::default()
    };
    for f in &l.fields {
        match list_field(f) {
            Some(("color", v)) => item.color = v,
            Some(("phase", v)) => item.phase = v,
            Some(("state", v)) => item.state = v,
            Some(("brief", v)) => item.brief = v,
            _ => {}
        }
    }
    if item.state.is_empty() {
        item.state = match l.checkbox {
            Some(true) => "done".into(),
            Some(false) => "todo".into(),
            None => String::new(),
        };
    }
    item
}

/// Read the backlog: its items, and each line it can't take as one.
pub fn read(text: &str) -> Backlog {
    let mut b = Backlog::default();
    let mut phase: Option<String> = None;
    let mut table: Option<Vec<Option<&'static str>>> = None;
    let mut unknown: Option<(usize, Vec<String>)> = None;
    let mut in_unknown = false;
    let mut tables = 0;
    for (i, raw) in text.lines().enumerate() {
        let line = i + 1;
        if let Some(p) = heading_phase(raw) {
            phase = p;
            table = None;
            in_unknown = false;
            continue;
        }
        let Some(row) = cells(raw) else {
            table = None;
            in_unknown = false;
            continue;
        };
        if let Some(cols) = &table {
            if separator(&row) || row.iter().all(String::is_empty) {
                continue;
            }
            let get = |f: &str| {
                cols.iter()
                    .position(|c| *c == Some(f))
                    .and_then(|k| row.get(k))
                    .map(|c| clean(c))
                    .unwrap_or_default()
            };
            let p = get("phase");
            let item = Item {
                line,
                id: get("id"),
                what: get("what"),
                color: get("color").to_lowercase(),
                phase: if p.is_empty() {
                    phase.clone().unwrap_or_default()
                } else {
                    phase_of(&p)
                },
                brief: get("brief"),
                state: get("state"),
                form: Form::Table(cols.clone()),
            };
            take(&mut b, item);
            continue;
        }
        if in_unknown {
            continue;
        }
        match columns(&row) {
            Some(cols) => {
                table = Some(cols);
                tables += 1;
            }
            None => {
                in_unknown = true;
                if unknown.is_none() && !separator(&row) {
                    unknown = Some((line, row));
                }
            }
        }
    }
    if tables == 0 {
        // No table of items: a list of them, if there is one.
        let mut phase: Option<String> = None;
        for (i, raw) in text.lines().enumerate() {
            if let Some(p) = heading_phase(raw) {
                phase = p;
                continue;
            }
            match list_line(raw) {
                Some(Ok(l)) => take(&mut b, list_item(i + 1, &l, &phase)),
                Some(Err(why)) => b.problems.push(Problem {
                    line: i + 1,
                    what: why,
                }),
                None => {}
            }
        }
    }
    if tables == 0 && b.items.is_empty() {
        b.problems.push(match unknown {
            Some((line, row)) => Problem {
                line,
                what: format!(
                    "a table whose columns name no id and no state ({}): {SHAPE}",
                    row.join(", ")
                ),
            },
            None if b.problems.is_empty() => Problem {
                line: 0,
                what: format!("no table of items and no list of them: {SHAPE}"),
            },
            None => return b,
        });
    }
    b
}

pub fn items(text: &str) -> Vec<Item> {
    read(text).items
}

/// The next item to build: the first `todo`, by phase and then by order.
pub fn next(items: &[Item]) -> Option<&Item> {
    let mut todo: Vec<&Item> = items.iter().filter(|i| i.state == "todo").collect();
    todo.sort_by_key(|i| (i.phase.parse::<i64>().unwrap_or(i64::MAX), i.line));
    todo.first().copied()
}

fn render(row: &[String]) -> String {
    let mut s = String::new();
    for c in row {
        if c.is_empty() {
            s.push_str("| ");
        } else {
            s.push_str(&format!("| {c} "));
        }
    }
    s.push('|');
    s
}

/// A list item's line with one field set: the state also checks or clears its checkbox.
fn set_list(line: &str, column: &str, value: &str) -> Result<String, String> {
    let Some(Ok(mut l)) = list_line(line) else {
        return Err("the item's line can't be read".into());
    };
    if column == "state" && l.checkbox.is_some() {
        let mark = if value == "done" { "[x]" } else { "[ ]" };
        for old in ["[ ]", "[x]", "[X]"] {
            if let Some(at) = l.head.find(old) {
                l.head.replace_range(at..at + 3, mark);
                break;
            }
        }
    }
    let key = |f: &str| list_field(f).map(|(k, _)| k);
    let written = match column {
        "state" => value.to_string(),
        "brief" => format!("brief {value}"),
        "color" => value.to_string(),
        "phase" => format!("phase {value}"),
        _ => return Err(format!("a list item has no {column}")),
    };
    match l.fields.iter().position(|f| key(f) == Some(column)) {
        Some(at) => l.fields[at] = written,
        None if column == "state" && l.checkbox.is_some() && value != "briefed" => {}
        None => l.fields.push(written),
    }
    // A checkbox says done or not: briefed is said in words, and a word it contradicts goes.
    if column == "state" && l.checkbox.is_some() && value != "briefed" {
        l.fields.retain(|f| key(f) != Some("state"));
    }
    Ok(if l.fields.is_empty() {
        l.head
    } else {
        format!("{} ({})", l.head, l.fields.join(", "))
    })
}

/// Rewrite one field of one item's line, keeping every other line as it was.
pub fn set(text: &str, id: &str, column: &str, value: &str) -> Result<String, String> {
    let b = read(text);
    let item = b
        .items
        .into_iter()
        .find(|i| i.id == id)
        .ok_or(format!("the backlog has no item {id}"))?;
    let mut out = String::new();
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i + 1 != item.line {
            out.push_str(l);
            continue;
        }
        let end = if l.ends_with('\n') { "\n" } else { "" };
        let body = l.trim_end_matches(['\n', '\r']);
        let new = match &item.form {
            Form::Table(cols) => {
                let col = cols
                    .iter()
                    .position(|c| *c == Some(column))
                    .ok_or(format!("the backlog's table has no column for {column}"))?;
                let mut row = cells(body).unwrap_or_default();
                while row.len() < cols.len() {
                    row.push(String::new());
                }
                row[col] = value.to_string();
                render(&row)
            }
            Form::List => set_list(body, column, value)?,
        };
        out.push_str(&format!("{new}{end}"));
    }
    Ok(out)
}

/// Add an item after the last one, as the backlog writes its items: a table's row, or a list's.
pub fn add(text: &str, item: &Item) -> Result<String, String> {
    let b = read(text);
    if b.items.iter().any(|i| i.id == item.id) {
        return Err(format!("the backlog already has an item {}", item.id));
    }
    let (after, line) = match b.items.last() {
        Some(last) => (
            last.line,
            match &last.form {
                Form::Table(cols) => table_row(cols, item),
                Form::List => list_row(item),
            },
        ),
        None => {
            // A table with its header and no row yet.
            let header = text
                .lines()
                .enumerate()
                .find_map(|(i, l)| cells(l).and_then(|r| columns(&r)).map(|c| (i + 1, c)))
                .ok_or("the backlog has no table with Id and State, and no list of items")?;
            let sep = text
                .lines()
                .nth(header.0)
                .and_then(cells)
                .is_some_and(|r| separator(&r));
            (header.0 + usize::from(sep), table_row(&header.1, item))
        }
    };
    let mut out = String::new();
    let mut placed = false;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        out.push_str(l);
        if i + 1 == after {
            if !l.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&line);
            out.push('\n');
            placed = true;
        }
    }
    if !placed {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

fn table_row(cols: &[Option<&'static str>], item: &Item) -> String {
    let row: Vec<String> = cols
        .iter()
        .map(|c| match c {
            Some("id") => item.id.clone(),
            Some("what") => item.what.replace('|', "/"),
            Some("color") => item.color.clone(),
            Some("phase") => item.phase.clone(),
            Some("brief") => item.brief.clone(),
            Some("state") => item.state.clone(),
            _ => String::new(),
        })
        .collect();
    render(&row)
}

fn list_row(item: &Item) -> String {
    let mut fields = vec![];
    if !item.color.is_empty() {
        fields.push(item.color.clone());
    }
    if !item.phase.is_empty() {
        fields.push(format!("phase {}", item.phase));
    }
    if !item.brief.is_empty() {
        fields.push(format!("brief {}", item.brief));
    }
    fields.push(item.state.clone());
    format!(
        "- `{}`: {} ({})",
        item.id,
        item.what.replace(['(', ')'], ""),
        fields.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: &str = "# Backlog\n\nWhat to build.\n\n| Id | What | Color | Phase | Brief | State |\n|---|---|---|---|---|---|\n| standing-checks | Set up the checks | red | 0 | | done |\n| empty-cart | A message | yellow | 1 | | todo |\n| pages | Pages | yellow | 1 | | todo |\n\nAfter the table.\n";

    #[test]
    fn reads_and_writes() {
        let i = items(B);
        assert_eq!(i.len(), 3);
        assert!(read(B).problems.is_empty());
        assert_eq!(next(&i).unwrap().id, "empty-cart");
        let t = set(B, "empty-cart", "state", "done").unwrap();
        assert!(t.contains("| empty-cart | A message | yellow | 1 | | done |\n"));
        assert_eq!(t.lines().count(), B.lines().count());
        let t = add(
            &t,
            &Item {
                id: "x".into(),
                what: "A | B".into(),
                color: "green".into(),
                phase: "2".into(),
                state: "todo".into(),
                ..Item::default()
            },
        )
        .unwrap();
        assert!(t.contains(
            "| pages | Pages | yellow | 1 | | todo |\n| x | A / B | green | 2 | | todo |\n\nAfter"
        ));
        assert!(
            add(
                &t,
                &Item {
                    id: "x".into(),
                    ..Item::default()
                }
            )
            .is_err()
        );
        assert!(set(B, "nope", "state", "done").is_err());
    }

    const HEADINGS: &str = "# Backlog\n\n## Phase 0\n\n| ID | What | Color | Brief | Status |\n|---|---|---|---|---|\n| **standing-checks** | Set up the checks | Red | | `todo` |\n\n### Fase 1: the shop\n\n| Id | Qué | Color | Brief | Estado | Notes |\n|---|---|---|---|---|---|\n| `catalog-search` | Search | yellow | | todo | soon |\n| Classify Features | Classify | red | | todo | |\n| pages | Pages | yellow | | maybe | |\n| catalog-search | Again | yellow | | todo | |\n";

    #[test]
    fn reads_tables_under_headings_and_names_what_it_cannot() {
        let b = read(HEADINGS);
        let ids: Vec<(&str, &str, &str, &str)> = b
            .items
            .iter()
            .map(|i| {
                (
                    i.id.as_str(),
                    i.phase.as_str(),
                    i.state.as_str(),
                    i.color.as_str(),
                )
            })
            .collect();
        assert_eq!(
            ids,
            vec![
                ("standing-checks", "0", "todo", "red"),
                ("catalog-search", "1", "todo", "yellow")
            ]
        );
        let said: Vec<String> = b.problems.iter().map(Problem::say).collect();
        assert_eq!(
            said,
            vec![
                "gdd/backlog.md, line 14: \"Classify Features\" is not an id: lowercase letters, digits and hyphens",
                "gdd/backlog.md, line 15: pages's state \"maybe\" is not todo, briefed or done",
                "gdd/backlog.md, line 16: catalog-search is an item already, at line 13",
            ]
        );
        // Written back where it is, in its own table's columns.
        let t = set(HEADINGS, "catalog-search", "state", "briefed").unwrap();
        assert!(t.contains("| `catalog-search` | Search | yellow | | briefed | soon |\n"));
        let t = set(&t, "catalog-search", "brief", "catalog-search").unwrap();
        assert!(t.contains(
            "| `catalog-search` | Search | yellow | catalog-search | briefed | soon |\n"
        ));
        let t = add(
            &t,
            &Item {
                id: "y".into(),
                what: "Y".into(),
                color: "green".into(),
                state: "todo".into(),
                ..Item::default()
            },
        )
        .unwrap();
        assert!(t.contains("| `catalog-search` | Search | yellow | catalog-search | briefed | soon |\n| y | Y | green | | todo | |\n"));
    }

    const LIST: &str = "# Backlog\n\nWhat to build.\n\n- a note: prose, not an item\n- [ ] `standing-checks`: Set up the checks (red, phase 0)\n- [x] **empty-cart**: A message (yellow, phase 1)\n- pages: Pages (yellow, phase 1, briefed, brief pages)\n- [ ] something with no id\n";

    #[test]
    fn reads_a_list_and_writes_it_back() {
        let b = read(LIST);
        let ids: Vec<(&str, &str, &str, &str)> = b
            .items
            .iter()
            .map(|i| {
                (
                    i.id.as_str(),
                    i.state.as_str(),
                    i.phase.as_str(),
                    i.brief.as_str(),
                )
            })
            .collect();
        assert_eq!(
            ids,
            vec![
                ("standing-checks", "todo", "0", ""),
                ("empty-cart", "done", "1", ""),
                ("pages", "briefed", "1", "pages")
            ]
        );
        assert_eq!(b.problems.len(), 1);
        assert_eq!(b.problems[0].line, 9);
        let t = set(LIST, "standing-checks", "state", "done").unwrap();
        assert!(t.contains("- [x] `standing-checks`: Set up the checks (red, phase 0)\n"));
        let t = set(&t, "pages", "state", "done").unwrap();
        assert!(t.contains("- pages: Pages (yellow, phase 1, done, brief pages)\n"));
        let t = add(
            &t,
            &Item {
                id: "later".into(),
                what: "Later".into(),
                color: "green".into(),
                state: "todo".into(),
                ..Item::default()
            },
        )
        .unwrap();
        assert!(t.contains(
            "- pages: Pages (yellow, phase 1, done, brief pages)\n- `later`: Later (green, todo)\n"
        ));
        assert_eq!(items(&t).len(), 4);
    }

    #[test]
    fn says_when_there_is_nothing_it_can_read() {
        let b = read("# Backlog\n\n| Item | Qué | Hecho |\n|---|---|---|\n| x | y | no |\n");
        assert!(b.items.is_empty());
        assert_eq!(b.problems.len(), 1);
        assert_eq!(b.problems[0].line, 3);
        assert!(b.problems[0].what.contains("no id and no state"));
        let b = read("# Backlog\n\nNothing yet.\n");
        assert_eq!(b.problems[0].line, 0);
    }
}
