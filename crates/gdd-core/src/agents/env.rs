//! The environment of an agent: the one gdd runs in, without the variables whose name says they
//! hold a secret (it ends in `_API_KEY`, `_TOKEN`, `_SECRET` or `_PASSWORD`, in any case), unless
//! `gdd/models.toml` passes them on purpose in `[env] pass`.

use serde::Deserialize;

/// The ends of the names of the variables an agent never gets, in capitals.
const SECRETS: [&str; 4] = ["_API_KEY", "_TOKEN", "_SECRET", "_PASSWORD"];

/// The `[env]` table of `gdd/models.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Env {
    /// Variables the agents get even when their name says they hold a secret: the exact names.
    pub pass: Vec<String>,
}

impl Env {
    /// The variables of `env`, the environment gdd runs in, that an agent gets, in their order.
    pub fn of_agent(&self, env: &[(String, String)]) -> Vec<(String, String)> {
        let gets = |name: &String| !secret(name) || self.pass.contains(name);
        env.iter().filter(|(name, _)| gets(name)).cloned().collect()
    }
}

/// Whether the variable `name` holds a secret, by its name.
fn secret(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    SECRETS.iter().any(|end| name.ends_with(end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(names: &[&str]) -> Vec<(String, String)> {
        let var = |name: &&str| (name.to_string(), "v".to_string());
        names.iter().map(var).collect()
    }

    fn names(env: &[(String, String)]) -> Vec<&str> {
        env.iter().map(|(name, _)| name.as_str()).collect()
    }

    #[test]
    fn variables_named_as_secrets_never_reach_an_agent() {
        let all = vars(&[
            "PATH",
            "OPENAI_API_KEY",
            "EDITOR",
            "GITHUB_TOKEN",
            "github_token",
            "Db_Password",
            "AWS_SECRET",
            "HOME",
        ]);
        let kept = Env::default().of_agent(&all);
        assert_eq!(names(&kept), ["PATH", "EDITOR", "HOME"]);
        // Only the end of the name counts, and it must be the whole end.
        let near = vars(&["TOKEN", "API_KEY_FILE", "MY_TOKENS", "XAPI_KEY"]);
        assert_eq!(Env::default().of_agent(&near), near);
        assert!(Env::default().of_agent(&[]).is_empty());
    }

    #[test]
    fn a_project_passes_the_variables_it_names_exactly() {
        let env = Env {
            pass: vec!["MM_API_KEY".into(), "EDITOR".into()],
        };
        let all = vars(&["MM_API_KEY", "mm_api_key", "B_API_KEY", "EDITOR"]);
        assert_eq!(names(&env.of_agent(&all)), ["MM_API_KEY", "EDITOR"]);
        let values = [("A_TOKEN".to_string(), "x=y".to_string())];
        let passed = Env {
            pass: vec!["A_TOKEN".into()],
        };
        assert_eq!(passed.of_agent(&values), values);
    }
}
