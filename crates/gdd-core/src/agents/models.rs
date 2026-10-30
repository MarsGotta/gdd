//! The model of each role, from `gdd/models.toml`: the Builder by the color of the change, the
//! Builder in test mode, the Reviewer and the Orchestrator. And the family of a model: the
//! Reviewer must not be of the Builder's family, or it would share its blind spots, unless the
//! human accepted it at the Setup ([`same_family_accepted`]). And whether the Builder's calls on
//! a change share one session ([`Session`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use toml::Spanned;

use crate::agents::sheets;
use crate::config::{self, Color};

/// Where the models of a project live, from its root.
pub const PATH: &str = "gdd/models.toml";

/// The key of the Reviewer's table that records the day (`YYYY-MM-DD`) the human accepted a
/// Reviewer of the Builder's model family.
pub const SAME_FAMILY_ACCEPTED: &str = "same_family_accepted";

/// Why a models file is refused when `[session] builder` is neither of its values.
const SESSION_VALUES: &str = "session.builder must be separate or shared";

/// A model as `gdd/models.toml` writes it: its table as it is (`integration`, `model`, and what
/// else it says, such as `effort`). Empty when the file says nothing about it.
pub type Model = Map<String, Value>;

/// How the Builder's calls on a change run: `[session] builder` of `gdd/models.toml`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Session {
    /// Each call starts a session of its own: build mode reads what test mode wrote. The default.
    #[default]
    Separate,
    /// Each call continues the session of the change's Builder call before it, in either mode.
    Shared,
}

impl Session {
    /// The value of `[session]`, a table as it is: `builder` missing is separate. Every reader of
    /// a models file refuses it with this error (see [`crate::agents::invoke::Settings`]).
    pub(crate) fn of(table: &Map<String, Value>) -> Result<Session, config::Error> {
        match table.get("builder") {
            None => Ok(Session::Separate),
            Some(Value::String(word)) if word == "separate" => Ok(Session::Separate),
            Some(Value::String(word)) if word == "shared" => Ok(Session::Shared),
            Some(other) => {
                let why = format!("{SESSION_VALUES}, not {other}");
                Err(config::Error::Toml(why))
            }
        }
    }
}

/// Why the Builder's two modes on a change of some color cannot share a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Apart {
    /// They use different agents: the integration of test mode, then that of build mode.
    Agents(String, String),
    /// Their agent, this integration, cannot resume a session: its sheet does not say how.
    NoResume(String),
}

/// The models of a project: the model of each role, the family of each model and the Builder
/// session.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Models {
    builder: BTreeMap<String, Model>,
    builder_test: Model,
    reviewer: Model,
    orchestrator: Model,
    families: Vec<(String, String)>,
    session: Session,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Raw {
    builder: BTreeMap<String, Model>,
    builder_test: Model,
    reviewer: Model,
    orchestrator: Model,
    // Where each family is written, so they keep the order of the file.
    families: BTreeMap<String, Spanned<String>>,
    // As it is: a value of its own refuses the file with the name of its key.
    session: Map<String, Value>,
}

/// The model of each role for a change. As JSON, each one is its table.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Roles {
    /// The Builder, chosen by the color of the change.
    pub builder: Model,
    /// The Builder in test mode.
    pub builder_test: Model,
    /// The Reviewer.
    pub reviewer_model: Model,
    /// The Orchestrator.
    pub orchestrator: Model,
}

impl Models {
    /// Reads a models file: `[orchestrator]`, `[builder.<color>]`, `[builder_test]`, `[reviewer]`,
    /// `[families]`, where each key, a model name prefix or an integration, names a family, and
    /// `[session] builder`, `separate` or `shared`. What it leaves out is empty, and the session
    /// separate; the rest of the file is ignored. The families keep the order the file writes
    /// them in.
    pub fn from_toml(text: &str) -> Result<Models, config::Error> {
        let raw: Raw = toml::from_str(text).map_err(|e| config::Error::Toml(e.to_string()))?;
        let session = Session::of(&raw.session)?;
        let mut families: Vec<(String, Spanned<String>)> = raw.families.into_iter().collect();
        families.sort_by_key(|(_, family)| family.span().start);
        Ok(Models {
            builder: raw.builder,
            builder_test: raw.builder_test,
            reviewer: raw.reviewer,
            orchestrator: raw.orchestrator,
            families: families
                .into_iter()
                .map(|(key, family)| (key, family.into_inner()))
                .collect(),
            session,
        })
    }

    /// The Builder session the file sets, whether or not a change can have it.
    pub fn session(&self) -> Session {
        self.session
    }

    /// These models with the Builder session `session`, which another models file sets.
    pub fn with_session(self, session: Session) -> Models {
        Models { session, ..self }
    }

    /// The Builder session a change of color `color` gets: shared when the file sets it and
    /// both modes of the change's Builder can share one (see [`Models::apart`]), else separate.
    pub fn builder_session(&self, color: Color) -> Session {
        match self.session {
            Session::Shared if self.apart(color).is_none() => Session::Shared,
            Session::Shared | Session::Separate => Session::Separate,
        }
    }

    /// Why test mode and the Builder of a change of color `color` cannot share a session: they
    /// use different agents, or their agent has no way to resume one in its sheet (see
    /// [`sheets::Sheet::resumes`]). `None` when they can.
    pub fn apart(&self, color: Color) -> Option<Apart> {
        let test = integration(&self.builder_test);
        let build = self.builder.get(&color.to_string()).map_or("", integration);
        if test != build {
            return Some(Apart::Agents(test.to_string(), build.to_string()));
        }
        let resumes = sheets::sheet(test).is_ok_and(|sheet| sheet.resumes());
        (!resumes).then(|| Apart::NoResume(test.to_string()))
    }

    /// The model of each role for a change of color `color`.
    pub fn roles(&self, color: Color) -> Roles {
        let builder = self.builder.get(&color.to_string()).cloned();
        Roles {
            builder: builder.unwrap_or_default(),
            builder_test: self.builder_test.clone(),
            reviewer_model: self.reviewer.clone(),
            orchestrator: self.orchestrator.clone(),
        }
    }

    /// The family of `model`: that of the first key of `[families]` its model name starts with
    /// or its integration equals; else its integration; `?` when it has none.
    pub fn family(&self, model: &Model) -> String {
        let text = |key| model.get(key).and_then(Value::as_str).unwrap_or_default();
        let (integration, name) = (text("integration"), text("model"));
        let fits = |key: &str| name.starts_with(key) || integration == key;
        match self.families.iter().find(|(key, _)| fits(key)) {
            Some((_, family)) => family.clone(),
            None if integration.is_empty() => "?".to_string(),
            None => integration.to_string(),
        }
    }
}

/// The day the human accepted that `reviewer`, the Reviewer's table, be of the Builder's model
/// family: its [`SAME_FAMILY_ACCEPTED`], when it is text that is not empty.
pub fn same_family_accepted(reviewer: &Model) -> Option<&str> {
    let day = reviewer.get(SAME_FAMILY_ACCEPTED).and_then(Value::as_str);
    day.filter(|day| !day.is_empty())
}

/// The integration of `model`, the agent that runs it; empty when it names none as text.
pub fn integration(model: &Model) -> &str {
    let integration = model.get("integration").and_then(Value::as_str);
    integration.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODELS: &str = r#"
[orchestrator]
integration = "claude"
model = "opus"

[builder.yellow]
integration = "claude"
model = "sonnet"

[builder.red]
integration = "claude"
model = "opus"

[builder_test]
integration = "claude"
model = "sonnet"

[reviewer]
integration = "codex"
model = "gpt-6-astra"
effort = "high"

[families]
"opus" = "anthropic"
"sonnet" = "anthropic"
"gpt" = "openai"
"#;

    fn model(json: Value) -> Model {
        match json {
            Value::Object(model) => model,
            other => panic!("not a table: {other}"),
        }
    }

    #[test]
    fn each_role_gets_its_table_as_it_is_and_the_builder_by_color() {
        let models = Models::from_toml(MODELS).unwrap();
        let red = models.roles(Color::Red);
        let opus = serde_json::json!({"integration": "claude", "model": "opus"});
        assert_eq!(red.builder, model(opus.clone()));
        assert_eq!(red.orchestrator, model(opus));
        let sonnet = serde_json::json!({"integration": "claude", "model": "sonnet"});
        assert_eq!(red.builder_test, model(sonnet.clone()));
        let reviewer =
            serde_json::json!({"integration": "codex", "model": "gpt-6-astra", "effort": "high"});
        assert_eq!(red.reviewer_model, model(reviewer));
        let yellow = models.roles(Color::Yellow);
        assert_eq!(yellow.builder, model(sonnet));
        assert_eq!(models.roles(Color::Green).builder, Model::new());
    }

    #[test]
    fn as_json_the_roles_are_their_tables() {
        let models = Models::from_toml(MODELS).unwrap();
        let json = serde_json::to_value(models.roles(Color::Green)).unwrap();
        assert_eq!(json["builder"], serde_json::json!({}));
        assert_eq!(json["builder_test"]["model"], "sonnet");
        assert_eq!(json["reviewer_model"]["effort"], "high");
        assert_eq!(json["orchestrator"]["model"], "opus");
        assert_eq!(json.as_object().map(Map::len), Some(4));
    }

    #[test]
    fn a_file_that_says_nothing_gives_empty_roles() {
        for text in ["", "[other]\nx = 1\n"] {
            let models = Models::from_toml(text).unwrap();
            assert_eq!(models, Models::default(), "{text:?}");
            assert_eq!(models.roles(Color::Red), Roles::default());
        }
    }

    #[test]
    fn a_file_that_is_not_valid_is_an_error() {
        for wrong in [
            "[reviewer",
            "reviewer = \"codex\"\n",
            "[builder]\nyellow = \"sonnet\"\n",
            "[families]\nopus = 1\n",
            "families = [\"opus\"]\n",
        ] {
            let result = Models::from_toml(wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
    }

    #[test]
    fn the_family_comes_from_the_model_name_or_the_integration() {
        let models = Models::from_toml(MODELS).unwrap();
        let family = |json| models.family(&model(json));
        let sonnet = serde_json::json!({"integration": "claude", "model": "sonnet-5"});
        assert_eq!(family(sonnet), "anthropic");
        let gpt = serde_json::json!({"integration": "codex", "model": "gpt-6-astra"});
        assert_eq!(family(gpt), "openai");
        let codex = "[families]\n\"codex\" = \"openai\"\n\"opus\" = \"anthropic\"\n";
        let by_integration = Models::from_toml(codex).unwrap();
        let o3 = model(serde_json::json!({"integration": "codex", "model": "o3"}));
        assert_eq!(by_integration.family(&o3), "openai");
        // No key fits: the integration, or "?" without one.
        let other = serde_json::json!({"integration": "opencode", "model": "qwen"});
        assert_eq!(family(other), "opencode");
        assert_eq!(family(serde_json::json!({"model": "qwen"})), "?");
        assert_eq!(family(serde_json::json!({})), "?");
        let numbers = serde_json::json!({"integration": 7, "model": 7});
        assert_eq!(family(numbers), "?");
        // The name must start with the key.
        let inside = serde_json::json!({"integration": "x", "model": "my-opus"});
        assert_eq!(family(inside), "x");
    }

    #[test]
    fn the_reviewer_says_when_the_human_accepted_one_family() {
        let accepted = "[reviewer]\nintegration = \"claude\"\nmodel = \"opus\"\n\
                        same_family_accepted = \"2026-10-27\"\n";
        let models = Models::from_toml(accepted).unwrap();
        let reviewer = models.roles(Color::Red).reviewer_model;
        assert_eq!(same_family_accepted(&reviewer), Some("2026-10-27"));
        assert_eq!(SAME_FAMILY_ACCEPTED, "same_family_accepted");
        for not_accepted in [
            serde_json::json!({"integration": "claude"}),
            serde_json::json!({"same_family_accepted": ""}),
            serde_json::json!({"same_family_accepted": true}),
        ] {
            let reviewer = model(not_accepted.clone());
            let said = same_family_accepted(&reviewer);
            assert_eq!(said, None, "{not_accepted}");
        }
    }

    #[test]
    fn the_first_family_that_fits_wins_in_the_order_of_the_file() {
        let json = serde_json::json!({"integration": "codex", "model": "gpt-6-codex"});
        let codex = model(json);
        let first = "[families]\n\"gpt-6\" = \"first\"\n\"gpt\" = \"second\"\n";
        assert_eq!(Models::from_toml(first).unwrap().family(&codex), "first");
        let second = "[families]\n\"gpt\" = \"second\"\n\"gpt-6\" = \"first\"\n";
        assert_eq!(Models::from_toml(second).unwrap().family(&codex), "second");
        let integration = "[families]\n\"codex\" = \"by-integration\"\n\"gpt\" = \"by-name\"\n";
        let family = Models::from_toml(integration).unwrap().family(&codex);
        assert_eq!(family, "by-integration");
    }

    /// [`MODELS`] with `[session] builder = <value>`, the value as TOML.
    fn with_session(value: &str) -> Result<Models, config::Error> {
        Models::from_toml(&format!("{MODELS}\n[session]\nbuilder = {value}\n"))
    }

    #[test]
    fn the_builder_session_is_separate_unless_the_file_shares_it() {
        let models = Models::from_toml(MODELS).unwrap();
        assert_eq!(models.session(), Session::Separate);
        let empty = Models::from_toml("[session]\nother = 1\n").unwrap();
        assert_eq!(empty.session(), Session::Separate);
        assert_eq!(empty, Models::default());
        let separate = with_session("\"separate\"").unwrap();
        assert_eq!(separate.session(), Session::Separate);
        let sharing = with_session("\"shared\"").unwrap();
        assert_eq!(sharing.session(), Session::Shared);
    }

    #[test]
    fn a_builder_session_that_is_neither_refuses_the_file() {
        let refused = "invalid TOML: session.builder must be separate or shared";
        // The value is shown as the file writes it.
        for value in ["\"joint\"", "\"Shared\"", "\"\"", "true", "3"] {
            let err = with_session(value).unwrap_err().to_string();
            assert_eq!(err, format!("{refused}, not {value}"));
        }
        assert!(Models::from_toml("session = \"shared\"\n").is_err());
    }

    /// The models of a project that shares the Builder session, whose Builder in test mode is
    /// `test` and whose yellow Builder is `yellow`, integrations both.
    fn shared(test: &str, yellow: &str) -> Models {
        let text = format!(
            "[builder_test]\nintegration = \"{test}\"\nmodel = \"m\"\n\
             [builder.yellow]\nintegration = \"{yellow}\"\nmodel = \"m\"\n\
             [session]\nbuilder = \"shared\"\n"
        );
        Models::from_toml(&text).unwrap()
    }

    #[test]
    fn a_shared_session_needs_one_agent_for_both_modes_that_can_resume() {
        let claude = shared("claude", "claude");
        assert_eq!(claude.apart(Color::Yellow), None);
        assert_eq!(claude.builder_session(Color::Yellow), Session::Shared);
        // Another agent in build mode.
        let other = shared("claude", "opencode");
        let agents = Apart::Agents("claude".into(), "opencode".into());
        assert_eq!(other.apart(Color::Yellow), Some(agents));
        assert_eq!(other.builder_session(Color::Yellow), Session::Separate);
        // A color without a Builder has no agent.
        let none = Apart::Agents("claude".into(), String::new());
        assert_eq!(claude.apart(Color::Red), Some(none));
        assert_eq!(claude.builder_session(Color::Red), Session::Separate);
        // One agent whose sheet does not say how to resume, or that has no sheet.
        for agent in ["codex", "opencode", "gemini"] {
            let one = shared(agent, agent);
            let no_resume = Some(Apart::NoResume(agent.to_string()));
            assert_eq!(one.apart(Color::Yellow), no_resume, "{agent}");
            let session = one.builder_session(Color::Yellow);
            assert_eq!(session, Session::Separate, "{agent}");
        }
    }

    #[test]
    fn a_change_never_shares_a_session_the_file_does_not_share() {
        let models = Models::from_toml(MODELS).unwrap();
        assert_eq!(models.apart(Color::Yellow), None);
        for color in [Color::Green, Color::Yellow, Color::Red] {
            let session = models.builder_session(color);
            assert_eq!(session, Session::Separate, "{color}");
        }
        let separate = with_session("\"separate\"").unwrap();
        assert_eq!(separate.builder_session(Color::Red), Session::Separate);
        let sharing = with_session("\"shared\"").unwrap();
        assert_eq!(sharing.builder_session(Color::Red), Session::Shared);
        // The session another file sets.
        let set = models.clone().with_session(Session::Shared);
        assert_eq!(set.builder_session(Color::Red), Session::Shared);
        assert_eq!(set.roles(Color::Red), models.roles(Color::Red));
        let unset = sharing.with_session(Session::Separate);
        assert_eq!(unset.builder_session(Color::Red), Session::Separate);
    }

    #[test]
    fn the_integration_of_a_model_is_its_agent() {
        let codex = model(serde_json::json!({"integration": "codex"}));
        assert_eq!(integration(&codex), "codex");
        let number = model(serde_json::json!({"integration": 7}));
        assert_eq!(integration(&number), "");
        assert_eq!(integration(&Model::new()), "");
    }
}
