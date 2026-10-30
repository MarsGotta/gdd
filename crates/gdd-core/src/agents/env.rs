//! The environment of an agent: the one gdd runs in, without the variables that hold a secret,
//! unless `gdd/models.toml` passes them on purpose in `[env] pass`, by their exact name. A variable
//! holds a secret when one of the words of its name says so (`AWS_SECRET_ACCESS_KEY`, `DB_PASS`,
//! `SENTRY_DSN`) or when its value is a URL with a password in it (`postgres://app:pw@db/app`).

use serde::Deserialize;

/// The words of a name that say its variable holds a secret, in capitals.
const SECRETS: [&str; 14] = [
    "KEY",
    "KEYS",
    "TOKEN",
    "TOKENS",
    "SECRET",
    "SECRETS",
    "PASSWORD",
    "PASSWORDS",
    "PASSWD",
    "PASS",
    "CREDENTIAL",
    "CREDENTIALS",
    "APIKEY",
    "DSN",
];

/// The `[env]` table of `gdd/models.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Env {
    /// Variables the agents get even when their name or their value says they hold a secret: the
    /// exact names.
    pub pass: Vec<String>,
}

impl Env {
    /// The variables of `env`, the environment gdd runs in, that an agent gets, in their order.
    pub fn of_agent(&self, env: &[(String, String)]) -> Vec<(String, String)> {
        let gets = |(name, value): &&(String, String)| {
            let secret = secret_name(name) || secret_value(value);
            !secret || self.pass.contains(name)
        };
        env.iter().filter(gets).cloned().collect()
    }
}

/// Whether the variable `name` holds a secret, by its name: one of its words, split at `_`, `-`
/// and `.`, is one of `SECRETS`, in any case.
fn secret_name(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    name.split(['_', '-', '.'])
        .any(|word| SECRETS.contains(&word))
}

/// Whether `value` holds a secret: it is a URL with a password in its user part
/// (`scheme://user:password@host` or `scheme://:password@host`). The user part ends at the last
/// `@` before the path, the query or the fragment, as browsers read a URL.
fn secret_value(value: &str) -> bool {
    let Some((_, rest)) = value.split_once("://") else {
        return false;
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let Some((user, _)) = rest[..end].rsplit_once('@') else {
        return false;
    };
    user.split_once(':')
        .is_some_and(|(_, password)| !password.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(names: &[&str]) -> Vec<(String, String)> {
        let var = |name: &&str| (name.to_string(), "v".to_string());
        names.iter().map(var).collect()
    }

    fn pairs(env: &[(&str, &str)]) -> Vec<(String, String)> {
        let pair = |(name, value): &(&str, &str)| (name.to_string(), value.to_string());
        env.iter().map(pair).collect()
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
            "AWS_SECRET_ACCESS_KEY",
            "github_token",
            "Db_Pass",
            "SENTRY_DSN",
            "HOME",
        ]);
        let kept = Env::default().of_agent(&all);
        assert_eq!(names(&kept), ["PATH", "EDITOR", "HOME"]);
        assert!(Env::default().of_agent(&[]).is_empty());
    }

    #[test]
    fn a_name_holds_a_secret_when_one_of_its_words_says_so() {
        let secrets = [
            "OPENAI_API_KEY",
            "SSH_KEYS",
            "TOKEN",
            "npm-tokens",
            "client.secret",
            "APP_SECRETS",
            "Db_Password",
            "SMTP_PASSWORDS",
            "MYSQL_PASSWD",
            "PASS_FILE",
            "GCP_CREDENTIAL",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "MAPS_APIKEY",
            "SENTRY_DSN",
            "API_KEY_FILE",
            "XAPI_KEY",
            "MY_TOKENS",
        ];
        for name in secrets {
            assert!(secret_name(name), "{name}");
        }
        // Only whole words count.
        let near = [
            "TOKENIZER_PATH",
            "KEYBOARD_LAYOUT",
            "PASSAGE_DIR",
            "PWD",
            "MONKEY",
            "APITOKEN",
        ];
        for name in near {
            assert!(!secret_name(name), "{name}");
        }
    }

    #[test]
    fn a_value_holds_a_secret_when_it_is_a_url_with_a_password() {
        let secrets = [
            "postgres://app:s3cret@db.example/app",
            "redis://:s3cret@cache:6379",
            "amqp://app:s3cret@queue",
            "jdbc:mysql://app:s3cret@db?ssl=true",
            "postgres://app:p@ss@db/app",
            "https://me@mail.example:s3cret@host/",
        ];
        for value in secrets {
            assert!(secret_value(value), "{value}");
        }
        // A user without a password, a colon in the host, or an `@` past the host is no secret.
        let plain = [
            "https://example.com/docs",
            "http://localhost:8080",
            "https://k@host",
            "https://k:@host",
            "app:s3cret@db",
            "https://example.com/a:b@c",
            "https://example.com?login=a:b@c",
            "https://example.com#a:b@c",
            "",
        ];
        for value in plain {
            assert!(!secret_value(value), "{value}");
        }
    }

    #[test]
    fn either_the_name_or_the_value_keeps_a_variable_from_an_agent() {
        let all = pairs(&[
            ("DATABASE_URL", "postgres://app:s3cret@db/app"),
            ("DOCS_URL", "https://example.com/docs"),
            ("DB_PASS", "plain"),
        ]);
        assert_eq!(names(&Env::default().of_agent(&all)), ["DOCS_URL"]);
    }

    #[test]
    fn a_project_passes_the_variables_it_names_exactly() {
        let env = Env {
            pass: vec!["MM_API_KEY".into(), "EDITOR".into()],
        };
        let all = vars(&["MM_API_KEY", "mm_api_key", "B_API_KEY", "EDITOR"]);
        assert_eq!(names(&env.of_agent(&all)), ["MM_API_KEY", "EDITOR"]);
        let values = pairs(&[
            ("A_TOKEN", "x=y"),
            ("DATABASE_URL", "postgres://app:s3cret@db/app"),
        ]);
        let passed = Env {
            pass: vec!["A_TOKEN".into(), "DATABASE_URL".into()],
        };
        assert_eq!(passed.of_agent(&values), values);
    }
}
