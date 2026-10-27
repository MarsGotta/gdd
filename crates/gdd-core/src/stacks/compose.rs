//! What several stack sheets give together, as `gdd stacks compose` prints it: the shape of
//! `gdd/commands.toml` plus a `[protected]` table. Every gate runs the commands of every sheet;
//! a command that is the same in all of them runs once; the protected paths add up.

use toml::Value;

use super::{Commands, Error, SECTIONS, Sheet, once, sheet};

/// What the sheets of the stacks `names` give together, as TOML (see [`composed`]). A name
/// given twice counts once, where it comes first.
pub fn compose(names: &[String]) -> Result<String, Error> {
    let sheets: Result<Vec<Sheet>, Error> = once(names).iter().map(|name| sheet(name)).collect();
    composed(&sheets?)
}

/// What `sheets` give together, as TOML without a line break at the end: their installs one
/// after the other, the commands of each section (see [`combined`]), the self-test of the first
/// and the protected paths of all, each once. No sheet is an error.
fn composed(sheets: &[Sheet]) -> Result<String, Error> {
    let [first, ..] = sheets else {
        return Err(Error::NoName);
    };
    let installs = sheets.iter().map(|sheet| sheet.install.as_str());
    let installs: Vec<&str> = installs.filter(|install| !install.is_empty()).collect();
    let mut lines = vec![entry("install", installs.join(" && "))];
    for (at, section) in SECTIONS.iter().enumerate() {
        lines.extend([String::new(), format!("[{section}]")]);
        let commands = combined(sheets, at).into_iter();
        lines.extend(commands.map(|(name, command)| entry(&name, command)));
    }
    let selftest = &first.selftest;
    lines.extend([String::new(), "[selftest]".to_string()]);
    lines.push(entry("file", selftest.file.clone()));
    lines.push(entry("content", selftest.content.clone()));
    let protected = sheets.iter().map(|sheet| &sheet.protected);
    let build = once(protected.clone().flat_map(|paths| &paths.builder_build));
    let test = once(protected.flat_map(|paths| &paths.builder_test));
    lines.extend([String::new(), "[protected]".to_string()]);
    lines.push(entry("builder_build", build));
    lines.push(entry("builder_test", test));
    Ok(lines.join("\n"))
}

/// `<key> = <value>`, the value written as TOML.
fn entry(key: &str, value: impl Into<Value>) -> String {
    format!("{key} = {}", value.into())
}

/// The commands `sheets` give together in the section `at` of [`SECTIONS`]. One sheet gives its
/// own, empty ones included. Several give, for each command name, one command under that name
/// when every sheet where it is not empty has the same one; else one per such sheet, in their
/// order, as `<name>-<stack>`: a suffix, because the gates know the mutation commands by the
/// start of their name. Empty commands are left out.
fn combined(sheets: &[Sheet], at: usize) -> Commands {
    if let [one] = sheets {
        return one.sections[at].clone();
    }
    let names = sheets.iter().flat_map(|sheet| &sheet.sections[at]);
    let mut commands = Vec::new();
    for name in once(names.map(|(name, _)| name)) {
        let given = |sheet: &Sheet| Some((sheet.name.clone(), command(sheet, at, &name)?));
        let given: Vec<(String, String)> = sheets.iter().filter_map(given).collect();
        match given.as_slice() {
            [(_, first), rest @ ..] if rest.iter().all(|(_, other)| other == first) => {
                commands.push((name, first.clone()));
            }
            _ => {
                let suffixed = |(stack, command)| (format!("{name}-{stack}"), command);
                commands.extend(given.into_iter().map(suffixed));
            }
        }
    }
    commands
}

/// The command `name` of the section `at` of `sheet`, when it has it and it is not empty.
fn command(sheet: &Sheet, at: usize, name: &str) -> Option<String> {
    let commands = &sheet.sections[at];
    let (_, command) = commands.iter().find(|(known, _)| known == name)?;
    Some(command.clone()).filter(|command| !command.is_empty())
}

#[cfg(test)]
mod tests {
    use super::super::parse;
    use super::*;

    /// A sheet of the stack `name` whose `[g2]` is `g2`. It installs with `i-<name>`, its
    /// mutation command is `m-<name>`, and its Builder in build mode must not touch `f/**` nor
    /// `<name>/**`.
    fn demo(name: &str, g2: &str) -> Sheet {
        let text = format!(
            "name = \"{name}\"\ntitle = \"T\"\nscenario_runner = \"r\"\ndetect = []\n\
             install = \"i-{name}\"\ntool = []\n[g2]\n{g2}\n[g3]\nmutation = \"m-{name}\"\n\
             [learn]\n[selftest]\nfile = \"{name}.test\"\ncontent = \"fail\"\n[protected]\n\
             builder_build = [\"f/**\", \"{name}/**\"]\nbuilder_test = [\"src/**\"]\n"
        );
        parse(&text).unwrap()
    }

    fn names(named: &[&str]) -> Vec<String> {
        named.iter().map(|name| name.to_string()).collect()
    }

    fn pairs(commands: &[(&str, &str)]) -> Commands {
        let pair = |(name, command): &(&str, &str)| (name.to_string(), command.to_string());
        commands.iter().map(pair).collect()
    }

    #[test]
    fn one_sheet_gives_its_commands_as_they_are() {
        let one = demo("a", "lint = \"l\"\ntypes = \"\"");
        let both = [("lint", "l"), ("types", "")];
        assert_eq!(combined(std::slice::from_ref(&one), 0), pairs(&both));
        let expected = "install = \"i-a\"\n\n[g2]\nlint = \"l\"\ntypes = \"\"\n\n\
                        [g3]\nmutation = \"m-a\"\n\n[learn]\n\n\
                        [selftest]\nfile = \"a.test\"\ncontent = \"fail\"\n\n[protected]\n\
                        builder_build = [\"f/**\", \"a/**\"]\nbuilder_test = [\"src/**\"]";
        assert_eq!(composed(&[one]).unwrap(), expected);
    }

    #[test]
    fn several_sheets_run_each_command_once_or_once_per_sheet() {
        let a = "lint = \"la\"\nsame = \"s\"\nonly = \"\"";
        let b = "lint = \"lb\"\nonly = \"o\"\nnone = \"\"\nsame = \"s\"";
        let c = "new = \"n\"\nlint = \"lb\"";
        let sheets = [demo("a", a), demo("b", b), demo("c", c)];
        let expected = [
            ("lint-a", "la"),
            ("lint-b", "lb"),
            ("lint-c", "lb"),
            ("same", "s"),
            ("only", "o"),
            ("new", "n"),
        ];
        assert_eq!(combined(&sheets, 0), pairs(&expected));
        // The mutation commands keep the word mutation first.
        let mutation = [
            ("mutation-a", "m-a"),
            ("mutation-b", "m-b"),
            ("mutation-c", "m-c"),
        ];
        assert_eq!(combined(&sheets, 1), pairs(&mutation));
        assert!(combined(&sheets, 2).is_empty());
        let two = [("lint", "lb"), ("only", "o")];
        assert_eq!(combined(&sheets[1..], 0)[..2], pairs(&two));
    }

    #[test]
    fn the_installs_run_one_after_the_other_and_the_selftest_is_the_first_one() {
        let mut sheets = [demo("a", ""), demo("b", ""), demo("c", "")];
        sheets[1].install.clear();
        let text = composed(&sheets).unwrap();
        let start = "install = \"i-a && i-c\"\n\n[g2]\n\n[g3]\n";
        assert!(text.starts_with(start), "{text}");
        assert!(text.contains("\nfile = \"a.test\"\n"), "{text}");
        let none = composed(&sheets[1..2]).unwrap();
        assert!(none.starts_with("install = \"\"\n"), "{none}");
    }

    #[test]
    fn the_protected_paths_add_up_each_once() {
        let text = composed(&[demo("a", ""), demo("b", "")]).unwrap();
        let protected = "[protected]\nbuilder_build = [\"f/**\", \"a/**\", \"b/**\"]\n\
                         builder_test = [\"src/**\"]";
        assert!(text.ends_with(protected), "{text}");
    }

    #[test]
    fn composing_needs_a_known_stack_and_counts_each_once() {
        assert_eq!(composed(&[]), Err(Error::NoName));
        assert_eq!(compose(&[]), Err(Error::NoName));
        let unknown = compose(&names(&["rust", "cobol"]));
        assert_eq!(unknown, Err(Error::Unknown("cobol".into())));
        let twice = compose(&names(&["rust", "rust"])).unwrap();
        assert_eq!(twice, compose(&names(&["rust"])).unwrap());
        assert!(twice.contains("\nacceptance_full = "), "{twice}");
        assert!(!twice.contains("-rust"), "{twice}");
    }

    /// The text of `value`, which must be TOML text.
    fn text(value: &Value) -> String {
        value.as_str().unwrap().to_string()
    }

    #[test]
    fn what_the_built_in_sheets_give_reads_back_as_they_are() {
        for combo in [
            "python",
            "rust",
            "typescript",
            "rust python",
            "python typescript",
            "typescript rust python",
        ] {
            let named: Vec<String> = combo.split(' ').map(str::to_string).collect();
            let sheets: Vec<Sheet> = named.iter().map(|name| sheet(name).unwrap()).collect();
            let table: toml::Table = toml::from_str(&compose(&named).unwrap()).unwrap();
            let installs: Vec<&str> = sheets.iter().map(|s| s.install.as_str()).collect();
            let install = text(&table["install"]);
            assert_eq!(install, installs.join(" && "), "{combo}");
            for (at, section) in SECTIONS.iter().enumerate() {
                let read = table[*section].as_table().unwrap().iter();
                let mut read: Commands = read.map(|(k, v)| (k.clone(), text(v))).collect();
                let mut expected = combined(&sheets, at);
                read.sort();
                expected.sort();
                assert_eq!(read, expected, "{combo} {section}");
            }
            let selftest = &sheets[0].selftest;
            assert_eq!(text(&table["selftest"]["file"]), selftest.file);
            assert_eq!(text(&table["selftest"]["content"]), selftest.content);
            let paths = |key: &str| -> Vec<String> {
                let list = table["protected"][key].as_array().unwrap();
                list.iter().map(text).collect()
            };
            let build = sheets.iter().flat_map(|s| &s.protected.builder_build);
            assert_eq!(paths("builder_build"), once(build), "{combo}");
            let test = sheets.iter().flat_map(|s| &s.protected.builder_test);
            assert_eq!(paths("builder_test"), once(test), "{combo}");
        }
    }
}
