//! The blueprint, `gdd/blueprint.md`: its YAML front matter, read as the contract's template writes it.

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Blueprint {
    pub front: Value,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub id: String,
    /// `machine`, `reviewer` or `person`.
    pub kind: String,
    pub command: Option<String>,
    /// `each round` or `before review`; a check without `runs` runs each round.
    pub runs: String,
    pub when: Vec<String>,
    pub cases: Option<String>,
    pub passes: Option<String>,
    pub breaks: Option<String>,
}

impl Check {
    pub fn before_review(&self) -> bool {
        self.runs == "before review"
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub id: String,
    pub color: String,
    pub paths: Vec<String>,
    pub gates: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gate {
    pub id: String,
    pub spec: Option<String>,
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextItem {
    /// `file` or `mcp`.
    pub kind: String,
    pub name: String,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Models {
    pub launch: Option<String>,
    pub light: Option<String>,
    pub standard: Option<String>,
    pub deep: Option<String>,
}

impl Models {
    pub fn of(&self, level: &str) -> Option<&str> {
        match level {
            "light" => self.light.as_deref(),
            "standard" => self.standard.as_deref(),
            "deep" => self.deep.as_deref(),
            _ => None,
        }
    }
}

/// One agents' environment of `levels`, named by its command.
#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    pub name: String,
    pub models: Models,
    pub reviewer: Option<Models>,
}

/// The cap keys the counter knows, in the order it prints them.
pub const CAP_KEYS: [&str; 7] = [
    "rounds_per_change",
    "fix_rounds_per_gate",
    "minutes_per_change",
    "runs_before_review",
    "tokens_per_change",
    "seconds_each_round",
    "seconds_before_review",
];

fn s(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn list(v: &Value) -> Vec<String> {
    match v {
        Value::Array(a) => a.iter().filter_map(s).collect(),
        Value::String(x) => vec![x.clone()],
        _ => vec![],
    }
}

/// A cap as the counter reads it: a whole number, never a placeholder.
fn whole(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64))
            .filter(|n| *n >= 0),
        _ => None,
    }
}

/// Split a Markdown file into its YAML front matter and its body. The front matter starts on the
/// first line, `---`, and ends at the next line that is `---`.
pub fn split_front_matter(text: &str) -> Option<(&str, &str)> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

/// Read YAML into JSON values, failing loudly with the reader's own message.
pub fn read_yaml(text: &str) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Ok(Value::Object(Default::default()));
    }
    serde_saphyr::from_str::<Value>(text).map_err(|e| e.to_string())
}

impl Blueprint {
    pub fn parse(text: &str) -> Result<Blueprint, String> {
        let (front, body) = split_front_matter(text)
            .ok_or("it has no front matter between two lines of `---` at its start")?;
        let front = read_yaml(front)?;
        if !front.is_object() {
            return Err("its front matter is not a mapping".into());
        }
        Ok(Blueprint {
            front,
            body: body.to_string(),
        })
    }

    pub fn contract(&self) -> Option<String> {
        s(&self.front["contract"])
    }

    pub fn guarantee(&self) -> Option<String> {
        s(&self.front["guarantee"])
    }

    /// The command that gives a fresh copy of the repository what the checks need, such as the
    /// project's dependencies. The gates run it in their throwaway worktree before any check.
    pub fn install(&self) -> Option<String> {
        s(&self.front["install"]).filter(|c| !c.trim().is_empty())
    }

    pub fn color_names(&self) -> Vec<String> {
        match &self.front["colors"] {
            Value::Object(m) => m.keys().cloned().collect(),
            _ => vec![],
        }
    }

    pub fn color_checks(&self, color: &str) -> Vec<String> {
        list(&self.front["colors"][color]["checks"])
    }

    pub fn color_asks(&self, color: &str, check: &str) -> bool {
        self.color_checks(color).iter().any(|c| c == check)
    }

    pub fn color_level(&self, color: &str) -> Option<String> {
        s(&self.front["colors"][color]["level"])
    }

    /// A cap that holds for every color, written directly under `limits`.
    pub fn cap_all(&self, key: &str) -> Option<i64> {
        whole(&self.front["limits"][key])
    }

    /// A cap of one color, under `colors.<color>.limits`.
    pub fn cap_own(&self, color: &str, key: &str) -> Option<i64> {
        whole(&self.front["colors"][color]["limits"][key])
    }

    /// The cap of a color, its own before the one for every color.
    pub fn cap(&self, color: &str, key: &str) -> Option<i64> {
        if !color.is_empty()
            && let Some(c) = self.cap_own(color, key)
        {
            return Some(c);
        }
        self.cap_all(key)
    }

    pub fn checks(&self) -> Vec<Check> {
        let Value::Array(items) = &self.front["checks"] else {
            return vec![];
        };
        items
            .iter()
            .filter_map(|c| {
                Some(Check {
                    id: s(&c["id"])?,
                    kind: s(&c["type"]).unwrap_or_default(),
                    command: s(&c["command"]),
                    runs: s(&c["runs"])
                        .map(|r| r.trim().to_string())
                        .unwrap_or_else(|| "each round".into()),
                    when: list(&c["when"]),
                    cases: s(&c["cases"]),
                    passes: s(&c["passes"]),
                    breaks: s(&c["breaks"]),
                })
            })
            .collect()
    }

    pub fn check(&self, id: &str) -> Option<Check> {
        self.checks().into_iter().find(|c| c.id == id)
    }

    pub fn zones(&self) -> Vec<Zone> {
        let Value::Array(items) = &self.front["zones"] else {
            return vec![];
        };
        items
            .iter()
            .filter_map(|z| {
                Some(Zone {
                    id: s(&z["id"])?,
                    color: s(&z["color"]).unwrap_or_default(),
                    paths: list(&z["paths"]),
                    gates: s(&z["gates"]),
                })
            })
            .collect()
    }

    pub fn gates(&self) -> Vec<Gate> {
        let Value::Array(items) = &self.front["gates"] else {
            return vec![];
        };
        items
            .iter()
            .filter_map(|g| {
                Some(Gate {
                    id: s(&g["id"])?,
                    spec: s(&g["spec"]),
                    checks: list(&g["checks"]),
                })
            })
            .collect()
    }

    pub fn context(&self) -> Vec<ContextItem> {
        let Value::Array(items) = &self.front["context"] else {
            return vec![];
        };
        items
            .iter()
            .filter_map(|c| {
                let (kind, name) = if let Some(f) = s(&c["file"]) {
                    ("file", f)
                } else {
                    ("mcp", s(&c["mcp"])?)
                };
                Some(ContextItem {
                    kind: kind.into(),
                    name,
                    targets: list(&c["for"]),
                })
            })
            .collect()
    }

    /// Each agents' environment of `levels`. A blueprint from before 0.4, with the three levels
    /// directly under `levels`, has one environment, named `default`.
    pub fn environments(&self) -> Vec<Environment> {
        let Value::Object(levels) = &self.front["levels"] else {
            return vec![];
        };
        let models = |v: &Value| Models {
            launch: s(&v["launch"]),
            light: s(&v["light"]),
            standard: s(&v["standard"]),
            deep: s(&v["deep"]),
        };
        if ["light", "standard", "deep"]
            .iter()
            .any(|k| levels.get(*k).is_some_and(|v| v.is_string()))
        {
            return vec![Environment {
                name: "default".into(),
                models: models(&self.front["levels"]),
                reviewer: None,
            }];
        }
        levels
            .iter()
            .filter(|(_, v)| v.is_object())
            .map(|(name, v)| Environment {
                name: name.clone(),
                models: models(v),
                reviewer: v.get("reviewer").filter(|r| r.is_object()).map(models),
            })
            .collect()
    }

    /// A blueprint from before 0.5 may say that what no zone covers counts as red. That is prose of
    /// its body, not a field, so it is read from the words.
    pub fn uncovered_is_red(&self) -> bool {
        let body = self.body.to_lowercase();
        body.contains("no zone covers counts as red")
            || body.contains("no zone covers is red")
            || body.contains("no zone covers count as red")
    }

    /// What in the front matter the layer can't use as the contract says, a line each.
    pub fn problems(&self) -> Vec<String> {
        let mut p = vec![];
        match self.contract().as_deref() {
            Some(crate::CONTRACT) => {}
            Some(other) => p.push(format!("its contract is {other}, not {}", crate::CONTRACT)),
            None => p.push("it names no contract".into()),
        }
        let colors = self.color_names();
        for c in &colors {
            if crate::color_rank(c).is_none() {
                p.push(format!(
                    "colors has {c:?}, which is not green, yellow or red"
                ));
            }
        }
        let ids: Vec<String> = self.checks().into_iter().map(|c| c.id).collect();
        for color in &colors {
            for c in self.color_checks(color) {
                if !ids.contains(&c) {
                    p.push(format!(
                        "color {color} asks for {c:?}, which checks doesn't list"
                    ));
                }
            }
        }
        for check in self.checks() {
            if check.kind == "machine" && check.command.is_none() {
                p.push(format!("the machine check {} has no command", check.id));
            }
            if check.runs != "each round" && check.runs != "before review" {
                p.push(format!(
                    "the check {} runs {:?}: it is `each round` or `before review`",
                    check.id, check.runs
                ));
            }
        }
        for z in self.zones() {
            if crate::color_rank(&z.color).is_none() {
                p.push(format!("the zone {} has the color {:?}", z.id, z.color));
            }
            if z.paths.is_empty() {
                p.push(format!("the zone {} has no paths", z.id));
            }
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../skills/gdd-prepare/assets/blueprint.md"),
        )
        .unwrap()
    }

    #[test]
    fn reads_the_contract_template() {
        let b = Blueprint::parse(&template()).unwrap();
        assert_eq!(b.contract().as_deref(), Some("gdd/1"));
        assert_eq!(b.cap_all("lines_per_change"), Some(400));
        assert_eq!(
            b.cap_all("tokens_per_change"),
            None,
            "a placeholder is no cap"
        );
        assert_eq!(b.cap("yellow", "minutes_per_change"), Some(45));
        assert_eq!(b.cap("green", "seconds_each_round"), Some(120));
        assert_eq!(b.cap("", "minutes_per_change"), None);
        let seal = b.check("seal").unwrap();
        assert!(seal.command.unwrap().contains("':(exclude)gdd/log'"));
        assert_eq!(seal.runs, "each round");
        assert_eq!(
            b.color_checks("red"),
            vec![
                "brief-approval",
                "seal",
                "tests",
                "lint",
                "secrets",
                "review",
                "person-review"
            ]
        );
        assert_eq!(b.color_level("green").as_deref(), Some("light"));
        assert!(b.color_asks("yellow", "brief-approval"));
        assert!(!b.color_asks("green", "brief-approval"));
        assert_eq!(
            b.zones()[0].paths,
            vec!["':(glob)<pattern>'".replace('\'', "")]
        );
        assert_eq!(b.environments().len(), 1);
        assert!(!b.uncovered_is_red());
    }

    #[test]
    fn fails_loudly() {
        assert!(Blueprint::parse("no front matter").is_err());
        assert!(Blueprint::parse("---\na: b: c\n---\n").is_err());
        assert!(Blueprint::parse("---\na: 1\na: 2\n---\n").is_err());
        assert!(Blueprint::parse("---\nlevels: [unclosed\n---\n").is_err());
    }

    #[test]
    fn old_levels_and_old_limits() {
        let b = Blueprint::parse(
            "---\nlevels:\n  light: a\n  standard: b\n  deep: c\nlimits:\n  rounds_per_change: 6\n---\nAnything no zone covers counts as red until the person classifies it.\n",
        )
        .unwrap();
        let e = b.environments();
        assert_eq!(e[0].name, "default");
        assert_eq!(e[0].models.of("deep"), Some("c"));
        assert_eq!(b.cap("yellow", "rounds_per_change"), Some(6));
        assert!(b.uncovered_is_red());
    }

    #[test]
    fn a_check_runs_each_round_unless_it_says() {
        let b = Blueprint::parse(
            "---\nchecks:\n  - id: e2e\n    type: machine\n    command: x\n    runs: before review  # once the rest pass\n    when: [':(glob)src/texts/**']\n  - id: t\n    type: machine\n    command: y\n---\n",
        )
        .unwrap();
        let c = b.checks();
        assert!(c[0].before_review());
        assert_eq!(c[0].when, vec![":(glob)src/texts/**"]);
        assert_eq!(c[1].runs, "each round");
    }
}
