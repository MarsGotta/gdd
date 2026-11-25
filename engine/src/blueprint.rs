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
    pub cases_each_change: bool,
    /// Who a failure goes back to: `build`, the builder, or `checks`, whoever wrote the change's
    /// checks, as with a check of planted faults whose survivor is a test too weak (decision 142).
    pub fails_to: String,
    /// `blocks`, the default, or `reports`: it runs and its result is written, and a failure
    /// stops nothing, except on red (decision 145).
    pub state: String,
}

impl Check {
    pub fn before_review(&self) -> bool {
        self.runs == "before review"
    }

    /// Whether its failure goes back to whoever wrote the change's checks.
    pub fn fails_to_checks(&self) -> bool {
        self.fails_to == "checks"
    }

    /// Whether the blueprint has it report and not block. The seal always blocks.
    pub fn reports(&self) -> bool {
        self.state == "reports" && self.id != "seal"
    }

    /// Whether a quarantine can take it: never the seal, nor the tests of a change's own scenarios,
    /// which judge the change itself (decision 145).
    pub fn quarantinable(&self) -> bool {
        self.id != "seal" && !self.cases_each_change
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
pub struct ContextItem {
    /// `file` or `mcp`.
    pub kind: String,
    pub name: String,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Models {
    pub launch: Option<String>,
    /// The launch of a role that only reads, such as the reading of a brief: the same command,
    /// with what keeps it from writing or running anything.
    pub read_only: Option<String>,
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

/// The parts of a split reading of a brief, in the order they are read: what a reader took the
/// change to do, what it can break, and what it could spare.
pub const READING_PARTS: [&str; 3] = ["understood", "breaks", "spare"];

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

    /// The command that gives a fresh copy of the repository what the checks need, such as the
    /// project's dependencies. The gates run it in their fresh copy before any check.
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

    /// Whether a color has the reading of its brief split in its three parts, each with a reader
    /// of its own: its `readings` is 3. One reading is the default.
    pub fn splits_reading(&self, color: &str) -> bool {
        whole(&self.front["colors"][color]["readings"]) == Some(3)
    }

    /// How many changes may be open at once, from `parallel`; none where the blueprint doesn't say,
    /// and then the layer holds no change back for another (decision 135).
    pub fn parallel(&self) -> Option<usize> {
        whole(&self.front["parallel"])
            .filter(|n| *n >= 1)
            .map(|n| n as usize)
    }

    /// The colors whose checks, level and limits are still the template's proposal, which the person
    /// decides when a change of that color first comes (decision 141). Red is never among them.
    pub fn provisional(&self) -> Vec<String> {
        let v = &self.front["provisional"];
        let named = list(v);
        if !v.is_array() || named.iter().any(|c| crate::color_rank(c).is_none()) {
            return vec![];
        }
        named.into_iter().filter(|c| c != "red").collect()
    }

    /// How many changes a check that passed and failed on the same commit reports instead of
    /// blocking, the person's rule approved once: none where the blueprint doesn't say (decision 145).
    pub fn quarantine(&self) -> Option<usize> {
        whole(&self.front["quarantine"])
            .filter(|n| *n >= 1)
            .map(|n| n as usize)
    }

    /// A figure of `harden` that opens a proposal of hardening once it is crossed: a whole number
    /// from 1 on, or none (decision 144).
    pub fn harden_figure(&self, key: &str) -> Option<usize> {
        whole(&self.front["harden"][key])
            .filter(|n| *n >= 1)
            .map(|n| n as usize)
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
                    cases_each_change: c["cases"] == "each change",
                    fails_to: s(&c["fails_to"])
                        .filter(|v| v == "checks")
                        .unwrap_or_else(|| "build".into()),
                    state: s(&c["state"])
                        .filter(|v| v == "reports")
                        .unwrap_or_else(|| "blocks".into()),
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
            read_only: s(&v["read_only"]),
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

    /// The days in use after which a change that promised nothing closes by itself. A blueprint
    /// without them was written before version 0.16 and reads as before: such a change is closed
    /// when it ships.
    pub fn closing_days(&self) -> i64 {
        whole(&self.front["closing"]["days"]).unwrap_or(0)
    }

    /// Who gives the verdict on a change in use, when the blueprint names someone.
    pub fn closing_who(&self) -> Option<String> {
        self.front["closing"]["who"]
            .as_str()
            .filter(|w| !w.trim().is_empty() && !w.starts_with('<'))
            .map(str::to_string)
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
        let provisional = &self.front["provisional"];
        if !provisional.is_null() {
            let named = list(provisional);
            if !provisional.is_array() || named.iter().any(|c| crate::color_rank(c).is_none()) {
                p.push("provisional is not a list of colors: every color counts as decided".into());
            }
            if named.iter().any(|c| c == "red") {
                p.push("provisional names red: what can't be undone is decided with the first yes, so red counts as decided".into());
            }
        }
        if let Value::Object(figures) = &self.front["harden"] {
            for (key, v) in figures {
                if !["sent_back", "approved_unchanged", "silent_runs", "explore_after"].contains(&key.as_str()) {
                    p.push(format!("harden has {key}, which the layer doesn't count: it opens nothing"));
                } else if self.harden_figure(key).is_none() && !v.is_null() {
                    p.push(format!("harden.{key} is not a whole number from 1 on: it opens nothing"));
                }
            }
        } else if !self.front["harden"].is_null() {
            p.push("harden is not a map of figures: nothing opens a proposal by itself".into());
        }
        if !self.front["quarantine"].is_null() && self.quarantine().is_none() {
            p.push("quarantine is not a whole number of changes from 1 on: a flaky check keeps blocking".into());
        }
        let parallel = &self.front["parallel"];
        if !parallel.is_null() && self.parallel().is_none() {
            p.push("parallel is not a whole number of changes from 1 on: no change is held back for another".into());
        }
        let closing = &self.front["closing"];
        if !closing.is_null() {
            let days = &closing["days"];
            if !closing.is_object() || !days.is_null() && whole(days).is_none() {
                p.push("closing.days is not a whole number of days: what promised nothing closes when it ships".into());
            }
            if !closing["who"].is_null() && self.closing_who().is_none() {
                p.push("closing.who names nobody: the change's author gives the verdict".into());
            }
        }
        let colors = self.color_names();
        for c in &colors {
            if crate::color_rank(c).is_none() {
                p.push(format!(
                    "colors has {c:?}, which is not green, yellow or red"
                ));
            }
        }
        for color in &colors {
            let readings = &self.front["colors"][color]["readings"];
            if !readings.is_null() && !matches!(whole(readings), Some(1 | 3)) {
                p.push(format!(
                    "color {color} has readings {readings}, which is neither 1 nor 3: its brief gets one reading"
                ));
            }
        }
        if let Value::Array(items) = &self.front["checks"] {
            for c in items {
                let id = s(&c["id"]).unwrap_or_default();
                if let Some(v) = s(&c["fails_to"]).filter(|v| v != "build" && v != "checks") {
                    p.push(format!("check {id} has fails_to {v}, neither build nor checks: its failures go to the builder"));
                }
                if let Some(v) = s(&c["state"]).filter(|v| v != "blocks" && v != "reports") {
                    p.push(format!("check {id} has state {v}, neither blocks nor reports: it blocks"));
                }
                if id == "seal" && s(&c["state"]).as_deref() == Some("reports") {
                    p.push("check seal has state reports: the seal always blocks".into());
                }
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
        assert_eq!(b.provisional(), vec!["green", "yellow"]);
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

    #[test]
    fn provisional_names_the_colors_still_to_decide_and_never_red() {
        let b = |front: &str| Blueprint::parse(&format!("---\ncontract: gdd/1\n{front}\n---\n")).unwrap();
        let some = b("provisional: [green, yellow]");
        assert_eq!(some.provisional(), vec!["green", "yellow"]);
        assert!(some.problems().is_empty(), "{:?}", some.problems());
        let red = b("provisional: [yellow, red]");
        assert_eq!(red.provisional(), vec!["yellow"]);
        assert!(red.problems().iter().any(|p| p.contains("provisional names red")));
        for wrong in ["provisional: yellow", "provisional: [yellow, blue]"] {
            let w = b(wrong);
            assert!(w.provisional().is_empty(), "{wrong}");
            assert!(w.problems().iter().any(|p| p.contains("not a list of colors")), "{wrong}");
        }
        assert!(b("parallel: 2").provisional().is_empty());
    }

    #[test]
    fn harden_names_the_figures_that_open_a_proposal() {
        let b = |front: &str| Blueprint::parse(&format!("---\ncontract: gdd/1\n{front}\n---\n")).unwrap();
        let ok = b("harden:\n  sent_back: 3\n  silent_runs: 50");
        assert_eq!((ok.harden_figure("sent_back"), ok.harden_figure("silent_runs")), (Some(3), Some(50)));
        assert_eq!(ok.harden_figure("approved_unchanged"), None);
        assert!(ok.problems().is_empty(), "{:?}", ok.problems());
        let wrong = b("harden:\n  sent_back: 0\n  escapes: 2");
        assert!(wrong.problems().iter().any(|p| p.contains("harden.sent_back is not a whole number")));
        assert!(wrong.problems().iter().any(|p| p.contains("harden has escapes")));
    }
}
