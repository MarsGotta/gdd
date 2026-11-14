//! A project's files, where the contract puts them.

use crate::blueprint::Blueprint;
use crate::log::Entry;
use crate::out::{Fail, R};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
}

impl Project {
    pub fn open(root: &Path) -> Project {
        Project {
            root: root.to_path_buf(),
        }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    pub fn has_blueprint(&self) -> bool {
        self.path("gdd/blueprint.md").is_file()
    }

    pub fn blueprint(&self) -> R<Blueprint> {
        let p = self.path("gdd/blueprint.md");
        let text = std::fs::read_to_string(&p).map_err(|_| {
            Fail::missing(
                "gdd/blueprint.md",
                "gdd/blueprint.md is missing: the next moment is Prepare",
            )
        })?;
        Blueprint::parse(&text).map_err(|e| Fail {
            exit: 2,
            reason: "unreadable".into(),
            lines: vec![format!("gdd/blueprint.md can't be read: {e}")],
            what: Some("gdd/blueprint.md".into()),
        })
    }

    pub fn log_rel(id: &str) -> String {
        format!("gdd/log/{id}.jsonl")
    }

    pub fn log_path(&self, id: &str) -> PathBuf {
        self.path(&Self::log_rel(id))
    }

    pub fn has_log(&self, id: &str) -> bool {
        self.log_path(id).is_file()
    }

    /// The entries of a change's log, or none when it has no log yet.
    pub fn read_log(&self, id: &str) -> Vec<Entry> {
        std::fs::read_to_string(self.log_path(id))
            .map(|t| crate::log::parse(&t))
            .unwrap_or_default()
    }

    pub fn journal_rel(id: &str) -> String {
        format!("gdd/log/{id}.engine.jsonl")
    }

    pub fn journal_path(&self, id: &str) -> PathBuf {
        self.path(&Self::journal_rel(id))
    }

    /// The names of the files directly in `gdd/log/`, sorted.
    fn log_files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.path("gdd/log"))
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// Every log of the project, without the layer's journals, by id.
    pub fn log_ids(&self) -> Vec<String> {
        self.log_files()
            .iter()
            .filter(|n| !n.ends_with(".engine.jsonl"))
            .filter_map(|n| n.strip_suffix(".jsonl").map(str::to_string))
            .collect()
    }

    /// Every journal of the layer, `gdd/log/<id>.engine.jsonl`, by id.
    pub fn journal_ids(&self) -> Vec<String> {
        self.log_files()
            .iter()
            .filter_map(|n| n.strip_suffix(".engine.jsonl").map(str::to_string))
            .collect()
    }

    /// The front matter of a change's brief in the working copy, and its body.
    pub fn brief(&self, id: &str) -> Option<(Value, String)> {
        let text = std::fs::read_to_string(self.path(&format!("gdd/briefs/{id}.md"))).ok()?;
        let (front, body) = crate::blueprint::split_front_matter(&text)?;
        Some((crate::blueprint::read_yaml(front).ok()?, body.to_string()))
    }
}

/// A Markdown section's text, from its `## <title>` to the next heading of the same level or above.
pub fn section(body: &str, title: &str) -> Option<String> {
    let mut out = vec![];
    let mut inside = false;
    for line in body.lines() {
        if let Some(t) = line.strip_prefix("## ") {
            if inside {
                break;
            }
            inside = t.trim().eq_ignore_ascii_case(title);
            continue;
        }
        if line.starts_with("# ") && inside {
            break;
        }
        if inside {
            out.push(line);
        }
    }
    Some(out.join("\n").trim().to_string()).filter(|t| !t.is_empty())
}

/// The first `# ` heading of a Markdown body.
pub fn title(body: &str) -> Option<String> {
    body.lines()
        .find_map(|l| l.strip_prefix("# ").map(|t| t.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections() {
        let body = "\n# A title\n\n## Request\n\n\"Do it.\"\n\n## Undoing it\n\nRevert.\n";
        assert_eq!(title(body).as_deref(), Some("A title"));
        assert_eq!(section(body, "request").as_deref(), Some("\"Do it.\""));
        assert_eq!(section(body, "Undoing it").as_deref(), Some("Revert."));
        assert_eq!(section(body, "Color"), None);
    }
}
