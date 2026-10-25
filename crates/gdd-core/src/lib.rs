//! The GDD (Gate-Driven Development) method as a library.
//!
//! Work moves forward by passing gates; risk decides how many gates and how strict.
//! This crate holds the method itself and has no terminal or CLI dependencies.

pub mod config;
pub mod rules;
pub mod zones;

pub use config::{Color, Error, Lane};
pub use rules::RulesTable;
pub use zones::ZoneMap;

/// Folder of the acceptance scenarios (`.feature` files).
const SCENARIOS_DIR: &str = "features";

/// Folder inside it whose scenarios do not need approval (G1).
const REGRESSION_DIR: &str = "features/regression";

/// Version of the `gdd` tool.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One line that identifies the tool, used by `gdd --version`.
pub fn version_line() -> String {
    format!("gdd {VERSION}")
}

/// A change to classify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Paths the change touches.
    pub paths: Vec<String>,
    /// Declared minimum color: it raises the color, never lowers it.
    pub min_color: Option<Color>,
    /// Whether the change can be resumed after an interruption (a condition of the fast lane).
    pub resumable: bool,
}

/// The decision for a change: its color, lane and the gates it must pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    /// Risk color.
    pub color: Color,
    /// Lane it travels.
    pub lane: Lane,
    /// Why: one line per path, plus the declared minimum when it raised the color.
    pub reasons: Vec<String>,
    /// New or changed scenarios need approval (G1).
    pub g1: bool,
    /// Scenario files the change touches (regression scenarios not included).
    pub features: Vec<String>,
    /// Tests are written before the code.
    pub tests_first: bool,
    /// Mutation testing runs.
    pub mutation: bool,
    /// The Reviewer checks the change.
    pub reviewer: bool,
    /// The human release gate (G4) applies.
    pub g4: bool,
    /// The human approves the plan.
    pub human_plan: bool,
}

/// Whether `path` is a scenario file that needs approval (not a regression one).
fn is_scenario(path: &str) -> bool {
    path.ends_with(".feature")
        && path.starts_with(&format!("{SCENARIOS_DIR}/"))
        && !path.starts_with(&format!("{REGRESSION_DIR}/"))
}

/// Classifies a change: the highest color among its paths (or the default color when it has
/// none), raised to its declared minimum, then the lane and gates the rules table gives it.
pub fn classify(
    zones: &ZoneMap,
    table: &RulesTable,
    change: &Change,
) -> Result<Classification, Error> {
    let mut color = zones.default;
    let mut reasons = Vec::new();
    if change.paths.is_empty() {
        reasons.push("no paths: default color".to_string());
    } else {
        let colors: Vec<Color> = change.paths.iter().map(|p| zones.color_of(p)).collect();
        for (path, c) in change.paths.iter().zip(&colors) {
            reasons.push(format!("{path} -> {c}"));
        }
        color = colors.into_iter().max().unwrap_or(color);
    }
    if let Some(min) = change.min_color.filter(|&min| min > color) {
        color = min;
        reasons.push(format!("declared minimum color: {color}"));
    }

    let mut features: Vec<String> = change
        .paths
        .iter()
        .filter(|p| is_scenario(p))
        .cloned()
        .collect();
    features.sort();
    features.dedup();
    let g1 = !features.is_empty();

    let lane = if color != Color::Red && !g1 && change.resumable {
        Lane::Fast
    } else {
        Lane::Full
    };
    let rule = table.rule(color, lane)?;
    Ok(Classification {
        color,
        lane,
        reasons,
        g1,
        features,
        tests_first: rule.tests_first && (g1 || color != Color::Green),
        mutation: rule.mutation,
        reviewer: rule.reviewer,
        g4: color == Color::Red || (g1 && rule.g4_if_g1),
        human_plan: color == Color::Red,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(paths: &[&str], resumable: bool) -> Change {
        Change {
            paths: paths.iter().map(|p| p.to_string()).collect(),
            min_color: None,
            resumable,
        }
    }

    fn zones() -> ZoneMap {
        ZoneMap::from_toml(
            "default = \"yellow\"\n[[zone]]\ncolor = \"green\"\npaths = [\"docs/**\"]\n",
        )
        .unwrap()
    }

    #[test]
    fn version_line_names_the_tool() {
        assert_eq!(version_line(), format!("gdd {VERSION}"));
        assert!(version_line().starts_with("gdd "));
    }

    #[test]
    fn regression_scenarios_are_not_scenarios_to_approve() {
        assert!(is_scenario("features/a.feature"));
        assert!(is_scenario("features/sub/a.feature"));
        assert!(!is_scenario("features/regression/a.feature"));
        assert!(!is_scenario("features/a.txt"));
        assert!(!is_scenario("other/a.feature"));
    }

    #[test]
    fn green_fast_skips_everything() {
        let c = classify(
            &zones(),
            &rules::default_table(),
            &change(&["docs/a.md"], true),
        )
        .unwrap();
        assert_eq!((c.color, c.lane), (Color::Green, Lane::Fast));
        assert!(!c.tests_first && !c.mutation && !c.reviewer && !c.g1 && !c.g4);
    }

    #[test]
    fn green_with_scenarios_writes_tests_first() {
        let c = classify(
            &zones(),
            &rules::default_table(),
            &change(&["docs/a.md", "features/a.feature"], true),
        )
        .unwrap();
        assert_eq!(c.lane, Lane::Full);
        assert!(c.tests_first && c.g1 && c.g4 && c.reviewer);
        assert_eq!(c.features, vec!["features/a.feature".to_string()]);
    }

    #[test]
    fn minimum_color_never_lowers() {
        let mut ch = change(&["docs/a.md"], true);
        ch.min_color = Some(Color::Yellow);
        let c = classify(&zones(), &rules::default_table(), &ch).unwrap();
        assert_eq!(c.color, Color::Yellow);
        ch.min_color = Some(Color::Green);
        ch.paths = vec!["x.rs".into()];
        let c = classify(&zones(), &rules::default_table(), &ch).unwrap();
        assert_eq!(c.color, Color::Yellow);
    }

    #[test]
    fn a_table_without_the_rule_is_an_error() {
        let empty = RulesTable::from_toml("").unwrap();
        assert_eq!(
            classify(&zones(), &empty, &change(&["x.rs"], false)),
            Err(Error::MissingRule(Color::Yellow, Lane::Full))
        );
    }
}
