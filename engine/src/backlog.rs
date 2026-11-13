//! The backlog, `gdd/backlog.md`: a Markdown table of items, each with its expected color and its
//! state, `todo`, `briefed` or `done`. Only the line of the item that changes is rewritten.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    pub line: usize,
    pub id: String,
    pub what: String,
    pub color: String,
    pub phase: String,
    pub brief: String,
    pub state: String,
}

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

/// The columns of the table, by name, and the line of its header.
fn header(text: &str) -> Option<(usize, Vec<String>)> {
    for (i, l) in text.lines().enumerate() {
        if let Some(c) = cells(l) {
            let lower: Vec<String> = c.iter().map(|x| x.to_lowercase()).collect();
            if lower.iter().any(|x| x == "id") && lower.iter().any(|x| x == "state") {
                return Some((i, lower));
            }
        }
    }
    None
}

pub fn items(text: &str) -> Vec<Item> {
    let Some((h, cols)) = header(text) else {
        return vec![];
    };
    let at = |row: &[String], name: &str| {
        cols.iter()
            .position(|c| c == name)
            .and_then(|i| row.get(i))
            .cloned()
            .unwrap_or_default()
    };
    let mut out = vec![];
    for (i, l) in text.lines().enumerate().skip(h + 1) {
        let Some(row) = cells(l) else {
            if out.is_empty() && l.trim().is_empty() {
                continue;
            }
            break;
        };
        if row
            .iter()
            .all(|c| c.chars().all(|x| x == '-' || x == ':' || x == ' '))
        {
            continue;
        }
        out.push(Item {
            line: i + 1,
            id: at(&row, "id"),
            what: at(&row, "what"),
            color: at(&row, "color"),
            phase: at(&row, "phase"),
            brief: at(&row, "brief"),
            state: at(&row, "state"),
        });
    }
    out
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

/// Rewrite one cell of one item's line, keeping every other line as it was.
pub fn set(text: &str, id: &str, column: &str, value: &str) -> Result<String, String> {
    let (_, cols) = header(text).ok_or("the backlog has no table with Id and State")?;
    let col = cols
        .iter()
        .position(|c| c == column)
        .ok_or(format!("the backlog has no column {column}"))?;
    let item = items(text)
        .into_iter()
        .find(|i| i.id == id)
        .ok_or(format!("the backlog has no item {id}"))?;
    let mut out = String::new();
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i + 1 == item.line {
            let mut row = cells(l).unwrap_or_default();
            while row.len() < cols.len() {
                row.push(String::new());
            }
            row[col] = value.to_string();
            let end = if l.ends_with('\n') { "\n" } else { "" };
            out.push_str(&format!("{}{end}", render(&row)));
        } else {
            out.push_str(l);
        }
    }
    Ok(out)
}

/// Add an item after the last line of the table.
pub fn add(text: &str, item: &Item) -> Result<String, String> {
    let (h, cols) = header(text).ok_or("the backlog has no table with Id and State")?;
    if items(text).iter().any(|i| i.id == item.id) {
        return Err(format!("the backlog already has an item {}", item.id));
    }
    let last = items(text).last().map(|i| i.line).unwrap_or(h + 2);
    let row: Vec<String> = cols
        .iter()
        .map(|c| match c.as_str() {
            "id" => item.id.clone(),
            "what" => item.what.replace('|', "/"),
            "color" => item.color.clone(),
            "phase" => item.phase.clone(),
            "brief" => item.brief.clone(),
            "state" => item.state.clone(),
            _ => String::new(),
        })
        .collect();
    let new_line = render(&row);
    let mut out = String::new();
    let mut placed = false;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        out.push_str(l);
        if i + 1 == last {
            if !l.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&new_line);
            out.push('\n');
            placed = true;
        }
    }
    if !placed {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&new_line);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: &str = "# Backlog\n\nWhat to build.\n\n| Id | What | Color | Phase | Brief | State |\n|---|---|---|---|---|---|\n| standing-checks | Set up the checks | red | 0 | | done |\n| empty-cart | A message | yellow | 1 | | todo |\n| pages | Pages | yellow | 1 | | todo |\n\nAfter the table.\n";

    #[test]
    fn reads_and_writes() {
        let i = items(B);
        assert_eq!(i.len(), 3);
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
}
