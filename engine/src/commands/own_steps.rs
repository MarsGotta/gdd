//! The orchestrator's own steps in the blueprint's log: prepare, harden and exam.

use super::*;

#[derive(Debug, Clone, Default)]
pub struct StepArgs {
    pub result: Option<String>,
    pub caught: Option<String>,
    pub level: Option<String>,
    pub model: Option<String>,
    pub skill_version: Option<String>,
    pub seconds: Option<f64>,
    pub tokens: Option<i64>,
    pub files: Vec<String>,
    pub probe: Option<String>,
    pub levels: Option<String>,
    pub change: Option<String>,
    pub crack: Option<String>,
    pub scenario: Option<String>,
    pub proposal: Option<String>,
    pub asked: Option<i64>,
    pub missed: Vec<String>,
}

fn json_list(text: &Option<String>, what: &str, keys: &[&str]) -> R<Value> {
    let Some(t) = text else { return Ok(json!([])) };
    let v: Value =
        serde_json::from_str(t).map_err(|e| Fail::usage(format!("{what} isn't JSON: {e}")))?;
    let ok = v.as_array().is_some_and(|a| {
        a.iter()
            .all(|x| x.is_object() && keys.iter().all(|k| x.get(*k).is_some()))
    });
    if !ok {
        return Err(Fail::usage(format!(
            "{what} is a JSON list of objects, each with {}",
            keys.join(", ")
        )));
    }
    Ok(v)
}

/// The orchestrator's own steps, `prepare`, `harden` and `exam`, in the shapes of the contract,
/// so nobody composes a whole report by hand. They go to the blueprint's log.
pub fn step_cmd(ctx: &Ctx, id: &str, st: &str, a: &StepArgs) -> Out {
    run("step", |out| {
        if id != "blueprint" {
            return Err(Fail::usage(format!(
                "a {st} step goes to the blueprint's log: gdd step blueprint {st} ..."
            )));
        }
        let l = load(ctx, id)?;
        let round = 1 + log::events(&l.entries)
            .iter()
            .filter(|(_, v)| step(v) == st)
            .count() as i64;
        let level = a.level.clone().unwrap_or_else(|| "deep".into());
        let caught = a.caught.clone().unwrap_or_else(|| "none".into());
        let mut m = obj(vec![("step", json!(st))]);
        match st {
            "prepare" => {
                let result = a
                    .result
                    .clone()
                    .ok_or(Fail::usage("--result is proposed or approved"))?;
                if result != "proposed" && result != "approved" {
                    return Err(Fail::usage("--result is proposed or approved"));
                }
                for f in &a.files {
                    if !ctx.root.join(f).exists() {
                        return Err(Fail::missing(
                            f.to_string(),
                            format!("{f} is not in the working copy"),
                        ));
                    }
                }
                m.insert(
                    "report".into(),
                    report_obj(
                        "gdd-prepare",
                        a.skill_version
                            .as_deref()
                            .or(Some(env!("CARGO_PKG_VERSION"))),
                        Some(&level),
                        a.model.as_deref(),
                        round,
                        &result,
                        &caught,
                        a.seconds,
                        a.tokens,
                    ),
                );
                m.insert("files".into(), json!(a.files));
                m.insert(
                    "probe".into(),
                    json_list(&a.probe, "--probe", &["try", "command", "exit", "tail"])?,
                );
                m.insert(
                    "levels".into(),
                    json_list(
                        &a.levels,
                        "--levels",
                        &["environment", "level", "launch", "model"],
                    )?,
                );
            }
            "harden" => {
                let (Some(crack), Some(proposal)) = (&a.crack, &a.proposal) else {
                    return Err(Fail::usage("a harden step takes --crack and --proposal"));
                };
                let result = a.result.clone().unwrap_or_else(|| "proposed".into());
                m.insert(
                    "report".into(),
                    report_obj(
                        "gdd-harden",
                        a.skill_version
                            .as_deref()
                            .or(Some(env!("CARGO_PKG_VERSION"))),
                        Some(&level),
                        a.model.as_deref(),
                        round,
                        &result,
                        &caught,
                        a.seconds,
                        a.tokens,
                    ),
                );
                let change = a.change.clone().filter(|c| c != "none");
                if let Some(c) = &change {
                    check_id(c)?;
                }
                m.insert("change".into(), json!(change));
                m.insert("crack".into(), json!(crack));
                m.insert(
                    "scenario".into(),
                    json!(a.scenario.clone().filter(|s| s != "none")),
                );
                m.insert("proposal".into(), json!(proposal));
            }
            "exam" => {
                let (Some(asked), Some(model)) = (a.asked, &a.model) else {
                    return Err(Fail::usage(
                        "an exam step takes --asked, --caught, --level and --model (the reviewer's)",
                    ));
                };
                let caught: i64 = caught.parse().map_err(|_| {
                    Fail::usage("--caught is how many questions it caught, a number")
                })?;
                if caught > asked || caught < 0 {
                    return Err(Fail::usage("--caught is between 0 and --asked"));
                }
                m.insert("reviewer".into(), json!({"level": level, "model": model}));
                m.insert("asked".into(), json!(asked));
                m.insert("caught".into(), json!(caught));
                m.insert("missed".into(), json!(a.missed));
            }
            _ => return Err(Fail::usage("the step is prepare, harden or exam")),
        }
        let ev = new_event(id, "step", &time::now_string(), m);
        let line = write(&l, id, &ev)?;
        out.say(format!(
            "wrote the {st} step to gdd/log/blueprint.jsonl, line {line}, round {round}"
        ));
        out.fact("line", line);
        out.fact("round", round);
        Ok(())
    })
}
