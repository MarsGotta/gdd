//! The gates a blueprint declares, as data. The core defines no gate: it reads the ones a
//! blueprint declares ([`Declared::from_toml`]) and refuses, as it reads them, a declaration it
//! could not run well. Then it tells which gate claims each command of a stage
//! ([`Declared::claim`]), which commands are optional and the name a level switches each one on
//! with ([`Declared::optional`]), which gate each named check belongs to ([`Declared::named`]),
//! and what one run of a stage proved for each gate ([`Key`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::config;
use crate::gates::{Gate, Report};

/// What every refusal of a declaration starts with.
const REFUSED: &str = "gate declarations";

/// The format of a declaration this core reads.
const FORMAT: i64 = 1;

/// What a gate id is made of.
const ID_SHAPE: &str = "lowercase ASCII letters, digits, - and _, starting with a letter";

/// Why a declaration has exactly one gate with `rest = true`.
const REST: &str = "one gate, and only one, takes the commands no gate claims";

/// The id of a gate a blueprint declares: lowercase ASCII letters, digits, `-` and `_`,
/// starting with a letter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct GateId(String);

impl GateId {
    /// The id `id`, when it is shaped as one; otherwise an error that names it.
    pub fn new(id: &str) -> Result<GateId, config::Error> {
        let mut chars = id.chars();
        let starts = chars.next().is_some_and(|c| c.is_ascii_lowercase());
        if starts && chars.all(is_id_char) {
            return Ok(GateId(id.to_string()));
        }
        Err(refused(format!("{id:?} is not a gate id: {ID_SHAPE}")))
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GateId {
    /// The id.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether `c` may follow the first letter of a gate id.
fn is_id_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'
}

/// A gate a blueprint declares. The core defines none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateDecl {
    /// Its id.
    pub id: GateId,
    /// What it checks: one sentence a person reads.
    pub checks: String,
    /// The kinds of command it claims (see [`kind`]).
    pub claims: Vec<String>,
    /// Whether it takes the commands no gate claims: exactly one gate of a declaration does.
    pub rest: bool,
    /// Its optional commands: by the name a level switches them on with, the patterns of their
    /// names (`name` for that name only, `prefix*` for every name that starts with `prefix`).
    pub optional: BTreeMap<String, Vec<String>>,
    /// The checks it names that are not commands, by stage.
    pub named: BTreeMap<Gate, Vec<String>>,
}

/// The gates of a blueprint, checked when read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The gates, in declaration order.
    gates: Vec<GateDecl>,
    /// The place in `gates` of the one that takes the rest.
    rest: usize,
}

/// A declaration as its file holds it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeclared {
    format: i64,
    #[serde(default)]
    gate: Vec<RawGate>,
}

/// A gate as a declaration holds it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGate {
    id: String,
    checks: String,
    #[serde(default)]
    claims: Vec<String>,
    #[serde(default)]
    rest: bool,
    #[serde(default)]
    optional: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    named: BTreeMap<String, Vec<String>>,
}

impl Declared {
    /// Reads a declaration: `format = 1` and one `[[gate]]` table per gate, with its `id` and
    /// `checks` and, when it has them, `claims`, `rest`, `optional` and `named` (by stage, as
    /// [`Gate::of_stage`] reads it). It refuses, naming the gate or the key: text that is not
    /// TOML or has a key it does not know, another format, no gate, an id not shaped as one (see
    /// [`GateId::new`]) or declared twice, `checks` blank, a kind claimed twice, not exactly one
    /// gate with `rest = true`, a stage it does not know, a check named twice in one stage, and
    /// an optional pattern that is empty or has a `*` anywhere but at its end.
    pub fn from_toml(text: &str) -> Result<Declared, config::Error> {
        let raw: RawDeclared = toml::from_str(text).map_err(|e| refused(e.to_string()))?;
        let format = raw.format;
        if format != FORMAT {
            let why = format!("format = {format} is not one this core reads: only {FORMAT}");
            return Err(refused(why));
        }
        if raw.gate.is_empty() {
            let why = "no [[gate]]: a blueprint declares at least one gate";
            return Err(refused(why.to_string()));
        }
        let mut seen = Seen::default();
        let gates = raw.gate.into_iter().map(|gate| seen.gate(gate));
        let gates = gates.collect::<Result<Vec<GateDecl>, config::Error>>()?;
        let rest = the_rest(&gates)?;
        Ok(Declared { gates, rest })
    }

    /// The gates, in declaration order.
    pub fn gates(&self) -> &[GateDecl] {
        &self.gates
    }

    /// The gate that claims the command `command`, the first that fits: the gate with an
    /// optional pattern its name matches, the gate that claims its kind (see [`kind`]), the gate
    /// that takes the rest.
    pub fn claim(&self, command: &str) -> &GateDecl {
        if let Some((gate, _)) = self.optional_of(command) {
            return gate;
        }
        let kind = kind(command);
        let claims = |gate: &&GateDecl| gate.claims.iter().any(|k| k == kind);
        let claimed = self.gates.iter().find(claims);
        claimed.unwrap_or(&self.gates[self.rest])
    }

    /// The name a level switches the command `command` on with, when it is optional: the name of
    /// the first optional pattern its name matches, over every gate in declaration order.
    pub fn optional(&self, command: &str) -> Option<&str> {
        self.optional_of(command).map(|(_, on)| on)
    }

    /// The gate with the first optional pattern the name `command` matches, and the name that
    /// switches it on.
    fn optional_of(&self, command: &str) -> Option<(&GateDecl, &str)> {
        let fits = |patterns: &[String]| patterns.iter().any(|p| matches(p, command));
        self.gates.iter().find_map(|gate| {
            let mut optional = gate.optional.iter();
            let found = optional.find(|(_, patterns)| fits(patterns.as_slice()));
            found.map(|(on, _)| (gate, on.as_str()))
        })
    }

    /// The gate that names the check `check` in `stage`.
    pub fn named(&self, stage: Gate, check: &str) -> Option<&GateDecl> {
        self.gates.iter().find(|gate| {
            let named = gate.named.get(&stage);
            named.is_some_and(|list| list.iter().any(|c| c == check))
        })
    }

    /// The key of each gate, in declaration order, for a run of `stage` whose commands are
    /// `commands` and whose report is `report`. A gate's checks are the commands of the stage it
    /// claims, in name order, then the checks it names in the stage; a gate without any gets no
    /// key. Each check is, the first that fits: failed (a failure of its name), passed (see
    /// [`is_check`]), skipped, owed (known debt of its name), or else not run. The engine's own
    /// checks (Path Guard, the scope, the debt file) belong to no gate.
    pub fn keys(
        &self,
        stage: Gate,
        commands: &BTreeMap<String, String>,
        report: &Report,
    ) -> Vec<Key> {
        let mut keys = Vec::new();
        for gate in &self.gates {
            let ours = |name: &&String| self.claim(name).id == gate.id;
            let claimed = commands.keys().filter(ours).map(String::as_str);
            let mut checks: Vec<&str> = claimed.collect();
            let named = gate.named.get(&stage).into_iter().flatten();
            checks.extend(named.map(String::as_str));
            if !checks.is_empty() {
                keys.push(Key::of(&gate.id, stage, &checks, report));
            }
        }
        keys
    }
}

/// The kind of the command `name`: its name up to its first `-`.
pub fn kind(name: &str) -> &str {
    name.split_once('-').map_or(name, |(kind, _)| kind)
}

/// Whether the name `name` matches the optional pattern `pattern`: `prefix*` matches every name
/// that starts with `prefix`, any other pattern only itself.
fn matches(pattern: &str, name: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == pattern,
    }
}

/// Whether `pattern` is an optional pattern: not empty, with a `*` at its end at most.
fn is_pattern(pattern: &str) -> bool {
    let body = pattern.strip_suffix('*').unwrap_or(pattern);
    !pattern.is_empty() && !body.contains('*')
}

/// Whether the entry `entry` of what passed is the check `check`: its name, alone or followed
/// by ` (` and what it counted.
fn is_check(entry: &str, check: &str) -> bool {
    entry == check || entry.starts_with(&format!("{check} ("))
}

/// A declaration refused for the reason `why`.
fn refused(why: String) -> config::Error {
    config::Error::Toml(format!("{REFUSED}: {why}"))
}

/// What a declaration said so far, as it is read: its ids, who claims each kind and who names
/// each check in each stage.
#[derive(Default)]
struct Seen {
    ids: BTreeSet<GateId>,
    kinds: BTreeMap<String, GateId>,
    named: BTreeMap<(Gate, String), GateId>,
}

impl Seen {
    /// The gate `raw`, checked against itself and the gates read before it.
    fn gate(&mut self, raw: RawGate) -> Result<GateDecl, config::Error> {
        let id = GateId::new(&raw.id)?;
        if !self.ids.insert(id.clone()) {
            return Err(refused(format!("gate \"{id}\" is declared twice")));
        }
        if raw.checks.trim().is_empty() {
            let why = format!("gate \"{id}\" has a blank checks: say in a sentence what it checks");
            return Err(refused(why));
        }
        self.claims(&id, &raw.claims)?;
        let named = self.named(&id, raw.named)?;
        patterns(&id, &raw.optional)?;
        Ok(GateDecl {
            id,
            checks: raw.checks,
            claims: raw.claims,
            rest: raw.rest,
            optional: raw.optional,
            named,
        })
    }

    /// Records that the gate `id` claims the kinds `claims`; a kind claimed before is refused.
    fn claims(&mut self, id: &GateId, claims: &[String]) -> Result<(), config::Error> {
        for kind in claims {
            if let Some(first) = self.kinds.insert(kind.clone(), id.clone()) {
                let why = format!("kind \"{kind}\" is claimed twice, by \"{first}\" and \"{id}\"");
                return Err(refused(why));
            }
        }
        Ok(())
    }

    /// The checks the gate `id` names, by stage, from `raw`, where they go by the names of their
    /// stages; a stage [`Gate::of_stage`] does not read, and a check named before in its stage,
    /// are refused.
    fn named(
        &mut self,
        id: &GateId,
        raw: BTreeMap<String, Vec<String>>,
    ) -> Result<BTreeMap<Gate, Vec<String>>, config::Error> {
        let mut named: BTreeMap<Gate, Vec<String>> = BTreeMap::new();
        for (name, checks) in raw {
            let Some(stage) = Gate::of_stage(&name) else {
                let why = format!(
                    "gate \"{id}\" names checks in {name:?}, which is not a stage: every_round \
                     or before_integrating"
                );
                return Err(refused(why));
            };
            for check in checks {
                if let Some(first) = self.named.insert((stage, check.clone()), id.clone()) {
                    let why = format!(
                        "check \"{check}\" is named twice in {}, by \"{first}\" and \"{id}\"",
                        stage.stage()
                    );
                    return Err(refused(why));
                }
                named.entry(stage).or_default().push(check);
            }
        }
        Ok(named)
    }
}

/// Refuses an optional pattern of the gate `id` that is not one (see [`is_pattern`]).
fn patterns(id: &GateId, optional: &BTreeMap<String, Vec<String>>) -> Result<(), config::Error> {
    for (on, patterns) in optional {
        if let Some(wrong) = patterns.iter().find(|p| !is_pattern(p)) {
            let why = format!(
                "gate \"{id}\" has the optional pattern {wrong:?} for \"{on}\": a pattern is not \
                 empty and has a * only at its end"
            );
            return Err(refused(why));
        }
    }
    Ok(())
}

/// The place of the one gate of `gates` that takes the rest; none, or more than one, is refused.
fn the_rest(gates: &[GateDecl]) -> Result<usize, config::Error> {
    let rest: Vec<usize> = (0..gates.len()).filter(|&i| gates[i].rest).collect();
    match rest.as_slice() {
        [one] => Ok(*one),
        [] => Err(refused(format!("no gate has rest = true: {REST}"))),
        [first, second, ..] => {
            let (first, second) = (&gates[*first].id, &gates[*second].id);
            let why = format!("gates \"{first}\" and \"{second}\" both have rest = true: {REST}");
            Err(refused(why))
        }
    }
}

/// What one run of one stage proved for one gate: each of its checks in that stage, by how it
/// ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Key {
    /// The gate.
    pub gate: GateId,
    /// The stage, by its name in a blueprint (see [`Gate::stage`]).
    pub stage: &'static str,
    /// Whether the gate holds after this run: none of its checks failed and all of them ran.
    pub ok: bool,
    /// Its checks that passed.
    pub passed: Vec<String>,
    /// Its checks that failed.
    pub failed: Vec<String>,
    /// Its commands the level of the change did not switch on.
    pub skipped: Vec<String>,
    /// Its commands that failed within their known debt.
    pub owed: Vec<String>,
    /// Its checks that did not run, since Path Guard failed before the commands ran; left out of
    /// the JSON when there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_run: Vec<String>,
}

impl Key {
    /// The key of the gate `gate` in `stage`, whose checks are `checks`, from `report`.
    fn of(gate: &GateId, stage: Gate, checks: &[&str], report: &Report) -> Key {
        let mut key = Key {
            gate: gate.clone(),
            stage: stage.stage(),
            ok: false,
            passed: Vec::new(),
            failed: Vec::new(),
            skipped: Vec::new(),
            owed: Vec::new(),
            not_run: Vec::new(),
        };
        for &check in checks {
            let list = if report.failures.iter().any(|f| f.name == check) {
                &mut key.failed
            } else if report.passed.iter().any(|p| is_check(p, check)) {
                &mut key.passed
            } else if report.skipped.iter().any(|s| s == check) {
                &mut key.skipped
            } else if report.known_debt.iter().any(|o| o.name == check) {
                &mut key.owed
            } else {
                &mut key.not_run
            };
            list.push(check.to_string());
        }
        key.ok = key.failed.is_empty() && key.not_run.is_empty();
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Color;
    use crate::gates::debt;

    /// Three gates: `alpha` claims `fmt` and `style`, has optional commands and names checks in
    /// both stages (one by an old stage name); `beta` claims `probe`, has an optional command and
    /// names a check before integrating; `gamma` takes the rest.
    const DECLARED: &str = r#"
format = 1

[[gate]]
id = "alpha"
checks = "the code is tidy"
claims = ["fmt", "style"]
optional = { deep = ["probe-deep", "slow*"] }
named = { every_round = ["tidy"], g3 = ["count"] }

[[gate]]
id = "beta"
checks = "the probes answer"
claims = ["probe"]
optional = { wide = ["wide*"] }
named = { before_integrating = ["ping"] }

[[gate]]
id = "gamma"
checks = "the rest of the work holds"
rest = true
"#;

    /// The lines of a gate `alpha` that checks `c`, to add keys to.
    const ALPHA: &str = "id = \"alpha\"\nchecks = \"c\"\n";

    fn declared() -> Declared {
        Declared::from_toml(DECLARED).unwrap()
    }

    fn id(id: &str) -> GateId {
        GateId::new(id).unwrap()
    }

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// A declaration of `format = 1`, the lines `gate` of a gate table, then the gate `gamma`,
    /// which takes the rest.
    fn with(gate: &str) -> String {
        let rest = "[[gate]]\nid = \"gamma\"\nchecks = \"c\"\nrest = true\n";
        format!("format = 1\n[[gate]]\n{gate}\n{rest}")
    }

    /// Why reading `text` is refused: what the error says after what every refusal says first.
    fn refusal(text: &str) -> String {
        let said = Declared::from_toml(text).unwrap_err().to_string();
        let why = said.strip_prefix("invalid TOML: gate declarations: ");
        why.unwrap_or_else(|| panic!("{said}")).to_string()
    }

    #[test]
    fn a_gate_id_is_lowercase_letters_digits_and_dashes_starting_with_a_letter() {
        for good in ["a", "a1", "a-b", "a_b", "alpha"] {
            let id = GateId::new(good).unwrap();
            assert_eq!(id.as_str(), good);
            assert_eq!(id.to_string(), good);
        }
        for bad in ["", "1a", "-a", "_a", "A", "aB", "a b", "a.b", "aé"] {
            let why = GateId::new(bad).unwrap_err().to_string();
            let said = format!("invalid TOML: {REFUSED}: {bad:?} is not a gate id: {ID_SHAPE}");
            assert_eq!(why, said, "{bad:?}");
        }
    }

    #[test]
    fn a_declaration_keeps_its_gates_in_order_with_their_fields() {
        let deep = strings(&["probe-deep", "slow*"]);
        let wide = strings(&["wide*"]);
        let expected = [
            GateDecl {
                id: id("alpha"),
                checks: "the code is tidy".into(),
                claims: strings(&["fmt", "style"]),
                rest: false,
                optional: BTreeMap::from([("deep".to_string(), deep)]),
                named: BTreeMap::from([
                    (Gate::G2, strings(&["tidy"])),
                    (Gate::G3, strings(&["count"])),
                ]),
            },
            GateDecl {
                id: id("beta"),
                checks: "the probes answer".into(),
                claims: strings(&["probe"]),
                rest: false,
                optional: BTreeMap::from([("wide".to_string(), wide)]),
                named: BTreeMap::from([(Gate::G3, strings(&["ping"]))]),
            },
            GateDecl {
                id: id("gamma"),
                checks: "the rest of the work holds".into(),
                claims: vec![],
                rest: true,
                optional: BTreeMap::new(),
                named: BTreeMap::new(),
            },
        ];
        assert_eq!(declared().gates(), expected);
    }

    #[test]
    fn text_that_is_not_a_declaration_is_refused_naming_the_key() {
        refusal("format = ");
        let unknown = refusal("format = 1\nshade = 2\n");
        assert!(unknown.contains("shade"), "{unknown}");
        let unknown = refusal(&with(&format!("{ALPHA}colour = 1")));
        assert!(unknown.contains("colour"), "{unknown}");
        let missing = refusal(&with("id = \"alpha\""));
        assert!(missing.contains("checks"), "{missing}");
        let missing = refusal("[[gate]]\nid = \"a\"\nchecks = \"c\"\nrest = true\n");
        assert!(missing.contains("format"), "{missing}");
    }

    #[test]
    fn a_format_other_than_1_is_refused() {
        let text = "format = 2\n[[gate]]\nid = \"alpha\"\nchecks = \"c\"\nrest = true\n";
        let said = "format = 2 is not one this core reads: only 1";
        assert_eq!(refusal(text), said);
    }

    #[test]
    fn a_declaration_without_gates_is_refused() {
        let said = "no [[gate]]: a blueprint declares at least one gate";
        assert_eq!(refusal("format = 1\n"), said);
        assert_eq!(refusal("format = 1\ngate = []\n"), said);
    }

    #[test]
    fn an_id_not_shaped_as_one_is_refused_naming_it() {
        let said = format!("\"Alpha\" is not a gate id: {ID_SHAPE}");
        assert_eq!(refusal(&with("id = \"Alpha\"\nchecks = \"c\"")), said);
    }

    #[test]
    fn an_id_declared_twice_is_refused_naming_it() {
        let said = "gate \"gamma\" is declared twice";
        assert_eq!(refusal(&with("id = \"gamma\"\nchecks = \"c\"")), said);
    }

    #[test]
    fn a_blank_checks_is_refused_naming_the_gate() {
        let said = "gate \"alpha\" has a blank checks: say in a sentence what it checks";
        assert_eq!(refusal(&with("id = \"alpha\"\nchecks = \"  \"")), said);
    }

    #[test]
    fn a_kind_claimed_twice_is_refused_in_one_gate_or_two() {
        let one = format!("{ALPHA}claims = [\"fmt\", \"fmt\"]");
        let said = "kind \"fmt\" is claimed twice, by \"alpha\" and \"alpha\"";
        assert_eq!(refusal(&with(&one)), said);
        let beta = "[[gate]]\nid = \"beta\"\nchecks = \"c\"\nclaims = [\"probe\", \"fmt\"]";
        let two = format!("{ALPHA}claims = [\"fmt\"]\n{beta}");
        let said = "kind \"fmt\" is claimed twice, by \"alpha\" and \"beta\"";
        assert_eq!(refusal(&with(&two)), said);
    }

    #[test]
    fn a_declaration_without_a_gate_for_the_rest_is_refused() {
        let text = format!("format = 1\n[[gate]]\n{ALPHA}rest = false\n");
        let said = format!("no gate has rest = true: {REST}");
        assert_eq!(refusal(&text), said);
    }

    #[test]
    fn two_gates_for_the_rest_are_refused_naming_them() {
        let both = with(&format!("{ALPHA}rest = true"));
        let said = format!("gates \"alpha\" and \"gamma\" both have rest = true: {REST}");
        assert_eq!(refusal(&both), said);
    }

    #[test]
    fn a_stage_that_is_not_one_is_refused_naming_it() {
        let gate = format!("{ALPHA}named = {{ every-round = [\"x\"] }}");
        let said = "gate \"alpha\" names checks in \"every-round\", which is not a stage: \
                    every_round or before_integrating";
        assert_eq!(refusal(&with(&gate)), said);
    }

    #[test]
    fn a_check_named_twice_in_one_stage_is_refused_in_one_gate_or_two() {
        let one = format!("{ALPHA}named = {{ every_round = [\"x\", \"x\"] }}");
        let said = "check \"x\" is named twice in every_round, by \"alpha\" and \"alpha\"";
        assert_eq!(refusal(&with(&one)), said);
        // An old name of the stage is the same stage.
        let beta = "[[gate]]\nid = \"beta\"\nchecks = \"c\"\nnamed = { G3 = [\"x\"] }";
        let two = format!("{ALPHA}named = {{ before_integrating = [\"x\"] }}\n{beta}");
        let said = "check \"x\" is named twice in before_integrating, by \"alpha\" and \"beta\"";
        assert_eq!(refusal(&with(&two)), said);
        // In another stage it is another check.
        let other = format!("{ALPHA}named = {{ g2 = [\"x\"], g3 = [\"x\"] }}");
        assert!(Declared::from_toml(&with(&other)).is_ok());
    }

    #[test]
    fn an_optional_pattern_that_is_empty_or_has_a_star_inside_is_refused_naming_it() {
        for wrong in ["", "*a", "a*b", "a**"] {
            let gate = format!("{ALPHA}optional = {{ deep = [\"a\", {wrong:?}] }}");
            let said = format!(
                "gate \"alpha\" has the optional pattern {wrong:?} for \"deep\": a pattern is \
                 not empty and has a * only at its end"
            );
            assert_eq!(refusal(&with(&gate)), said, "{wrong:?}");
        }
        for right in ["a", "a*", "*"] {
            let gate = format!("{ALPHA}optional = {{ deep = [{right:?}] }}");
            assert!(Declared::from_toml(&with(&gate)).is_ok(), "{right:?}");
        }
    }

    #[test]
    fn the_kind_of_a_command_is_its_name_up_to_its_first_dash() {
        assert_eq!(kind("fmt"), "fmt");
        assert_eq!(kind("fmt-strict"), "fmt");
        assert_eq!(kind("fmt-a-b"), "fmt");
        assert_eq!(kind("fmt_strict"), "fmt_strict");
    }

    #[test]
    fn a_command_goes_to_its_optional_pattern_then_its_kind_then_the_rest() {
        let declared = declared();
        let claimed = |command| declared.claim(command).id.as_str();
        // Its kind sends it to beta, but an optional pattern of alpha matches it.
        assert_eq!(claimed("probe-deep"), "alpha");
        assert_eq!(claimed("probe"), "beta");
        assert_eq!(claimed("probe-x"), "beta");
        assert_eq!(claimed("fmt-strict"), "alpha");
        assert_eq!(claimed("style"), "alpha");
        // An optional pattern of beta, of a kind nobody claims.
        assert_eq!(claimed("wide-x"), "beta");
        // A kind that only starts like a claimed one, and one nobody claims, go to the rest.
        assert_eq!(claimed("fmtx"), "gamma");
        assert_eq!(claimed("probex"), "gamma");
        assert_eq!(claimed("other"), "gamma");
    }

    #[test]
    fn an_optional_command_is_one_a_pattern_of_any_gate_matches() {
        let declared = declared();
        assert_eq!(declared.optional("probe-deep"), Some("deep"));
        for name in ["slow", "slow-x", "slow_x", "slowx"] {
            assert_eq!(declared.optional(name), Some("deep"), "{name}");
        }
        assert_eq!(declared.optional("wide"), Some("wide"));
        assert_eq!(declared.optional("wide-x"), Some("wide"));
        for name in ["probe", "probe-deeper", "slo", "xslow", ""] {
            assert_eq!(declared.optional(name), None, "{name}");
        }
    }

    #[test]
    fn a_named_check_belongs_to_the_gate_that_names_it_in_its_stage() {
        let declared = declared();
        let named = |stage, check| declared.named(stage, check).map(|g| g.id.as_str());
        assert_eq!(named(Gate::G2, "tidy"), Some("alpha"));
        assert_eq!(named(Gate::G3, "count"), Some("alpha"));
        assert_eq!(named(Gate::G3, "ping"), Some("beta"));
        assert_eq!(named(Gate::G3, "tidy"), None);
        assert_eq!(named(Gate::G2, "ping"), None);
        assert_eq!(named(Gate::G2, "other"), None);
    }

    /// The commands named `names`, each with a line of shell.
    fn commands(names: &[&str]) -> BTreeMap<String, String> {
        let line = || "true".to_string();
        names.iter().map(|n| (n.to_string(), line())).collect()
    }

    fn owed(name: &str) -> debt::Owed {
        debt::Owed {
            name: name.to_string(),
            tail: "x".to_string(),
            count: None,
            known: None,
        }
    }

    #[test]
    fn each_gate_gets_a_key_with_each_of_its_checks_by_how_it_ended() {
        let mut report = Report::new(Gate::G2, Color::Yellow);
        report.passed = strings(&["fmt", "tidy", "path-guard", "scope"]);
        report.fail("fmt-strict", "x".to_string());
        report.fail("scope", "y".to_string());
        report.skipped = strings(&["probe"]);
        report.known_debt = vec![owed("probe-x")];
        let ran = commands(&["fmt", "fmt-strict", "probe", "probe-x", "zed"]);
        let keys = declared().keys(Gate::G2, &ran, &report);
        let alpha = Key {
            gate: id("alpha"),
            stage: "every_round",
            ok: false,
            passed: strings(&["fmt", "tidy"]),
            failed: strings(&["fmt-strict"]),
            skipped: vec![],
            owed: vec![],
            not_run: vec![],
        };
        let beta = Key {
            gate: id("beta"),
            ok: true,
            passed: vec![],
            failed: vec![],
            skipped: strings(&["probe"]),
            owed: strings(&["probe-x"]),
            ..alpha.clone()
        };
        let gamma = Key {
            gate: id("gamma"),
            passed: vec![],
            failed: vec![],
            not_run: strings(&["zed"]),
            ..alpha.clone()
        };
        assert_eq!(keys, [alpha, beta, gamma]);
    }

    #[test]
    fn a_key_holds_with_only_passed_checks_and_takes_a_check_with_its_count() {
        let mut report = Report::new(Gate::G3, Color::Yellow);
        report.passed = strings(&["fmt", "path-guard", "count (2)", "ping"]);
        let keys = declared().keys(Gate::G3, &commands(&["fmt"]), &report);
        let alpha = Key {
            gate: id("alpha"),
            stage: "before_integrating",
            ok: true,
            passed: strings(&["fmt", "count"]),
            failed: vec![],
            skipped: vec![],
            owed: vec![],
            not_run: vec![],
        };
        let beta = Key {
            gate: id("beta"),
            passed: strings(&["ping"]),
            ..alpha.clone()
        };
        // gamma has no check before integrating: no key.
        assert_eq!(keys, [alpha, beta]);
        // An entry that only starts like the check is not the check.
        report.passed = strings(&["fmtx", "count(2)", "ping"]);
        let keys = declared().keys(Gate::G3, &commands(&["fmt"]), &report);
        assert_eq!(keys[0].not_run, ["fmt", "count"]);
        assert!(!keys[0].ok);
    }

    #[test]
    fn a_stage_without_commands_or_named_checks_gives_no_key() {
        let report = Report::new(Gate::G2, Color::Yellow);
        let none = Declared::from_toml(&with(ALPHA)).unwrap();
        assert!(none.keys(Gate::G2, &BTreeMap::new(), &report).is_empty());
        let keys = declared().keys(Gate::G2, &BTreeMap::new(), &report);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].gate, id("alpha"));
        assert_eq!(keys[0].not_run, ["tidy"]);
    }

    #[test]
    fn as_json_a_key_names_its_gate_and_stage_and_leaves_out_an_empty_not_run() {
        let key = Key {
            gate: id("alpha"),
            stage: Gate::G2.stage(),
            ok: true,
            passed: strings(&["fmt"]),
            failed: vec![],
            skipped: vec![],
            owed: strings(&["style"]),
            not_run: vec![],
        };
        let said = concat!(
            r#"{"gate":"alpha","stage":"every_round","ok":true,"passed":["fmt"],"#,
            r#""failed":[],"skipped":[],"owed":["style"]}"#
        );
        assert_eq!(serde_json::to_string(&key).unwrap(), said);
        let not_run = Key {
            ok: false,
            not_run: strings(&["probe"]),
            ..key
        };
        let json = serde_json::to_string(&not_run).unwrap();
        let end = r#""owed":["style"],"not_run":["probe"]}"#;
        assert!(json.ends_with(end), "{json}");
    }
}
