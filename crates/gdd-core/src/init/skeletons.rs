//! The skeletons gdd init writes: the files a new project of each stack starts with, for every
//! stack the folder has no project of yet, with `{package}` filled with the package identifier
//! of the project. Nothing else of their text changes.

use std::path::Path;

use super::exists;
use crate::agents::sheets::fill;
use crate::stacks::{self, File, Sheet};

/// The package identifier of a project named `name`: lowercase, each run of characters other
/// than `a-z` and `0-9` one `_`, without `_` at either end; `app_` before one that starts with a
/// digit, and `app` for one with nothing left.
pub(super) fn package(name: &str) -> String {
    let mut identifier = String::new();
    for character in name.to_lowercase().chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            identifier.push(character);
        } else if !identifier.ends_with('_') {
            identifier.push('_');
        }
    }
    let identifier = identifier.trim_matches('_');
    match identifier.chars().next() {
        None => "app".to_string(),
        Some(first) if first.is_ascii_digit() => format!("app_{identifier}"),
        Some(_) => identifier.to_string(),
    }
}

/// The skeleton files gdd init writes in the folder `root` for the `stacks`, whose sheets are
/// `sheets`, with the package identifier `package`: their paths and their texts, in the order of
/// the stacks (see [`merged`]). The skeleton of a stack one of whose detect files is in the
/// folder is left out whole: the folder has a project of that stack already.
pub(super) fn files(
    root: &Path,
    stacks: &[String],
    sheets: &[Sheet],
    package: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut chosen = Vec::new();
    for (stack, sheet) in stacks.iter().zip(sheets) {
        if !has_project(root, sheet)? {
            let skeleton = stacks::skeleton(stack).map_err(|e| e.to_string())?;
            chosen.push((stack.as_str(), skeleton));
        }
    }
    merged(&chosen, package)
}

/// Whether the folder `root` has a project of the stack of `sheet`: one of its detect files.
fn has_project(root: &Path, sheet: &Sheet) -> Result<bool, String> {
    for path in sheet.detect() {
        if exists(root, path)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The files of the skeletons `chosen`, each with its stack, with `{package}` filled in their
/// paths and texts: in order, each path once. Two stacks that have the same path with different
/// texts are an error that names it and both.
fn merged(chosen: &[(&str, Vec<File>)], package: &str) -> Result<Vec<(String, String)>, String> {
    let mut files: Vec<(String, String, &str)> = Vec::new();
    for (stack, skeleton) in chosen {
        for file in skeleton {
            let path = filled(&file.path, package);
            let content = filled(&file.content, package);
            match files.iter().find(|(known, ..)| *known == path) {
                None => files.push((path, content, *stack)),
                Some((_, same, _)) if *same == content => {}
                Some((_, _, first)) => return Err(different(&path, first, stack)),
            }
        }
    }
    let texts = files.into_iter().map(|(path, content, _)| (path, content));
    Ok(texts.collect())
}

/// `text` with `{package}` as `package`; any other brace stays.
fn filled(text: &str, package: &str) -> String {
    fill(text, &[("package", package)])
}

/// Why the skeletons of the stacks `first` and `second` cannot both be written: they have `path`
/// with different texts.
fn different(path: &str, first: &str, second: &str) -> String {
    format!(
        "the {first} and {second} skeletons both have {path}, with different contents: gdd init \
         cannot set up these stacks together"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Folder;
    use std::fs;

    #[test]
    fn the_package_identifier_comes_from_the_name() {
        for (name, expected) in [
            ("Notes App", "notes_app"),
            ("2nd Brain!", "app_2nd_brain"),
            ("my-garden", "my_garden"),
            ("notes", "notes"),
            ("  Été -- 2026  ", "t_2026"),
            ("a__b", "a_b"),
            ("_x_", "x"),
            ("9", "app_9"),
            ("!!!", "app"),
            ("", "app"),
            ("ÁRBOL", "rbol"),
        ] {
            assert_eq!(package(name), expected, "{name:?}");
        }
    }

    fn file(path: &str, content: &str) -> File {
        File {
            path: path.to_string(),
            content: content.to_string(),
        }
    }

    fn pairs(files: &[(&str, &str)]) -> Vec<(String, String)> {
        let pair = |(path, content): &(&str, &str)| (path.to_string(), content.to_string());
        files.iter().map(pair).collect()
    }

    #[test]
    fn the_package_fills_paths_and_texts_and_nothing_else() {
        let one = vec![
            file("src/{package}/a.py", "{package} {name} ${x} {}"),
            file("{packages}", "{ package }"),
        ];
        let filled = merged(&[("python", one)], "notes").unwrap();
        let expected = [
            ("src/notes/a.py", "notes {name} ${x} {}"),
            ("{packages}", "{ package }"),
        ];
        assert_eq!(filled, pairs(&expected));
    }

    /// Why the skeletons `chosen` cannot be merged; empty when they can.
    fn merged_err(chosen: &[(&str, Vec<File>)]) -> String {
        merged(chosen, "p").err().unwrap_or_default()
    }

    #[test]
    fn a_file_two_stacks_have_alike_is_written_once_and_a_different_one_is_an_error() {
        let a = vec![file("same", "x"), file("a", "{package}")];
        let b = vec![file("b", "b"), file("same", "x"), file("a", "p")];
        let both = merged(&[("a", a.clone()), ("b", b)], "p").unwrap();
        assert_eq!(both, pairs(&[("same", "x"), ("a", "p"), ("b", "b")]));
        let c = vec![file("c", "c"), file("same", "y")];
        let said = merged_err(&[("a", a.clone()), ("c", c)]);
        let why = "the a and c skeletons both have same, with different contents: gdd init \
                   cannot set up these stacks together";
        assert_eq!(said, why);
        assert_eq!(merged_err(&[("a", a.clone()), ("b", a)]), "");
    }

    /// The skeleton files gdd init writes in `folder` for the stacks `named`.
    fn written(folder: &Folder, named: &[&str]) -> Vec<(String, String)> {
        let chosen: Vec<String> = named.iter().map(|name| name.to_string()).collect();
        let sheets: Vec<Sheet> = named.iter().map(|n| stacks::sheet(n).unwrap()).collect();
        files(&folder.0, &chosen, &sheets, "notes_app").unwrap()
    }

    /// The paths of the skeleton files gdd init writes in `folder` for the stacks `named`.
    fn paths(folder: &Folder, named: &[&str]) -> Vec<String> {
        let files = written(folder, named).into_iter();
        files.map(|(path, _)| path).collect()
    }

    #[test]
    fn a_stack_the_folder_has_a_project_of_keeps_its_own_files() {
        let folder = Folder::new();
        let rust = paths(&folder, &["rust"]);
        assert_eq!(rust[0], "Cargo.toml");
        let both = paths(&folder, &["rust", "typescript"]);
        assert_eq!(both[..rust.len()], rust);
        assert!(both.contains(&"package.json".to_string()), "{both:?}");
        folder.write("Cargo.toml", "[package]\n");
        let typescript = paths(&folder, &["typescript"]);
        let both = paths(&folder, &["rust", "typescript"]);
        assert_eq!(both, typescript);
        assert_eq!(paths(&folder, &["rust"]), Vec::<String>::new());
        // Any detect file is enough, even as a folder.
        fs::create_dir(folder.0.join("tsconfig.json")).unwrap();
        let none = paths(&folder, &["typescript"]);
        assert_eq!(none, Vec::<String>::new());
    }

    #[test]
    fn the_python_skeleton_gets_the_package_and_shares_its_scanner_files() {
        let folder = Folder::new();
        let files = written(&folder, &["python", "typescript"]);
        let text = |path: &str| {
            let found = files.iter().find(|(known, _)| known == path);
            found.map(|(_, text)| text.as_str()).unwrap_or_default()
        };
        let greeting = text("src/notes_app/greeting.py");
        assert!(greeting.contains("f\"Hello, {name}!\""), "{greeting}");
        let test = text("tests/unit/test_greeting.py");
        let import = "from notes_app.greeting import greeting\n";
        assert!(test.starts_with(import), "{test}");
        let shared = files.iter().filter(|(path, _)| path == "osv-scanner.toml");
        assert_eq!(shared.count(), 1);
    }
}
