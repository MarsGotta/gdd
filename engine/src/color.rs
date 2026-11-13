//! The color of a change: git sets the floor from the zones its paths fall in, and it only rises.

use crate::blueprint::Blueprint;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Colored {
    /// `planned` from the paths a brief lists, `touched` from what a change did.
    pub on: String,
    pub paths: Vec<String>,
    /// Each zone hit, with the paths that fall in it.
    pub zones: BTreeMap<String, Vec<String>>,
    pub uncovered: Vec<String>,
    pub floor: String,
    pub color: String,
    pub why: String,
}

/// The floor a set of zone hits gives: the highest color among them, and never green for a path
/// no zone covers (yellow, or red where the blueprint says so).
pub fn floor(
    bp: &Blueprint,
    zones: &BTreeMap<String, Vec<String>>,
    uncovered: &[String],
) -> String {
    let mut f = "green".to_string();
    for z in bp.zones() {
        if zones.contains_key(&z.id) {
            f = crate::higher(&f, &z.color).to_string();
        }
    }
    if !uncovered.is_empty() {
        let at_least = if bp.uncovered_is_red() {
            "red"
        } else {
            "yellow"
        };
        f = crate::higher(&f, at_least).to_string();
    }
    f
}

/// Each zone hit, with its paths, and the paths no zone covers.
pub type Sorted = (BTreeMap<String, Vec<String>>, Vec<String>);

/// A path as the log keeps it: relative to the project's root, with no `./` in front and no `/`
/// at the end. A path outside the project is refused: an agent's `./src/x` and `src/x/` used to
/// reach git as they were, match no pathspec, and come back as paths no zone covers.
pub fn clean_path(path: &str) -> Result<String, String> {
    let mut q = path.trim().replace('\\', "/");
    while let Some(r) = q.strip_prefix("./") {
        q = r.to_string();
    }
    let q = q.trim_end_matches('/').to_string();
    if q.is_empty() || q == "." {
        return Err("a path can't be the whole project: name its files or its folders".into());
    }
    if q.starts_with('/') || q.split('/').any(|c| c == "..") {
        return Err(format!(
            "{path}: a path is relative to the project's root, with no .."
        ));
    }
    Ok(q)
}

/// The files a path stands for: itself, or, for a folder, the tracked files under it, and one
/// made-up file inside it when it holds none yet, so a new folder is still matched by its zone.
fn stands_for(root: &Path, given: &str, clean: &str) -> Result<Vec<String>, String> {
    if !(given.trim().ends_with('/') || root.join(clean).is_dir()) {
        return Ok(vec![clean.to_string()]);
    }
    let tracked = crate::git::ls_files(root, &[format!(":(literal){clean}")])?;
    if tracked.is_empty() {
        Ok(vec![format!("{clean}/_")])
    } else {
        Ok(tracked)
    }
}

/// Put each path in the zones whose pathspecs git matches it to, and list what no zone covers.
/// A folder is in every zone that holds one of its files, and it is covered only when all of them
/// are.
pub fn sort_paths(root: &Path, bp: &Blueprint, paths: &[String]) -> Result<Sorted, String> {
    let mut stands: Vec<(String, Vec<String>)> = vec![];
    for p in paths {
        let clean = clean_path(p)?;
        let files = stands_for(root, p, &clean)?;
        stands.push((clean, files));
    }
    let mut all: Vec<String> = stands.iter().flat_map(|(_, f)| f.iter().cloned()).collect();
    all.sort();
    all.dedup();
    let mut zones: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut covered: Vec<String> = vec![];
    for z in bp.zones() {
        let hit = crate::git::match_paths(root, &all, &z.paths)?;
        if !hit.is_empty() {
            let given: Vec<String> = stands
                .iter()
                .filter(|(_, files)| files.iter().any(|f| hit.contains(f)))
                .map(|(p, _)| p.clone())
                .collect();
            covered.extend(hit);
            zones.insert(z.id.clone(), given);
        }
    }
    let uncovered = stands
        .iter()
        .filter(|(_, files)| files.iter().any(|f| !covered.contains(f)))
        .map(|(p, _)| p.clone())
        .collect();
    Ok((zones, uncovered))
}

/// The color of some paths, raised by judgment or by the color already logged, never lowered.
pub fn color_of(
    root: &Path,
    bp: &Blueprint,
    on: &str,
    paths: &[String],
    raise: Option<&str>,
    logged: Option<&str>,
    why: &str,
) -> Result<Colored, String> {
    let (zones, uncovered) = sort_paths(root, bp, paths)?;
    let paths: Vec<String> = paths
        .iter()
        .map(|p| clean_path(p))
        .collect::<Result<_, _>>()?;
    let paths = &paths[..];
    let fl = floor(bp, &zones, &uncovered);
    let mut color = fl.clone();
    let mut reasons = vec![];
    if !uncovered.is_empty() {
        reasons.push(format!(
            "{} no zone covers, never green",
            uncovered.join(", ")
        ));
    }
    if let Some(r) = raise {
        if crate::color_rank(r).is_none() {
            return Err(format!("{r:?} is not a color"));
        }
        if crate::color_rank(r) > crate::color_rank(&color) {
            color = r.to_string();
            reasons.push(if why.is_empty() {
                "judgment raised it".into()
            } else {
                why.to_string()
            });
        }
    }
    if let Some(l) = logged
        && crate::color_rank(l) > crate::color_rank(&color)
    {
        color = l.to_string();
        reasons.push(format!("it was logged as {l}, and a color only rises"));
    }
    Ok(Colored {
        on: on.into(),
        paths: paths.to_vec(),
        zones,
        uncovered,
        floor: fl,
        color,
        why: reasons.join("; "),
    })
}

/// What `gdd zones` finds: each pathspec that matches no tracked file, and each tracked file no
/// zone covers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ZoneReport {
    pub empty: Vec<(String, String)>,
    pub uncovered: Vec<String>,
    pub counts: Vec<(String, String, usize)>,
}

pub fn zones(root: &Path, bp: &Blueprint) -> Result<ZoneReport, String> {
    let mut r = ZoneReport::default();
    let mut all_specs = vec![];
    for z in bp.zones() {
        for spec in &z.paths {
            let hit = crate::git::ls_files(root, std::slice::from_ref(spec))?;
            if hit.is_empty() {
                r.empty.push((z.id.clone(), spec.clone()));
            }
            r.counts.push((z.id.clone(), spec.clone(), hit.len()));
            all_specs.push(spec.clone());
        }
    }
    let tracked = crate::git::ls_files(root, &[])?;
    let covered = if all_specs.is_empty() {
        vec![]
    } else {
        crate::git::ls_files(root, &all_specs)?
    };
    r.uncovered = tracked
        .into_iter()
        .filter(|f| !covered.contains(f))
        .collect();
    Ok(r)
}
