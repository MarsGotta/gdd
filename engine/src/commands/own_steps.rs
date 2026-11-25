//! The orchestrator's own steps in the blueprint's log: prepare, harden and exam.

use super::*;

#[derive(Debug, Clone, Default, clap::Args)]
pub struct StepArgs {
    /// prepare: proposed or approved; harden: proposed (the default).
    #[arg(long)]
    pub result: Option<String>,
    /// What it caught that others missed (prepare, harden), or how many questions (exam).
    #[arg(long)]
    pub caught: Option<String>,
    /// The level it ran at (the reviewer's, for an exam); deep by default.
    #[arg(long)]
    pub level: Option<String>,
    /// The model it ran on (the reviewer's, for an exam).
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long)]
    pub skill_version: Option<String>,
    #[arg(long)]
    pub seconds: Option<f64>,
    #[arg(long)]
    pub tokens: Option<i64>,
    /// prepare: a file it wrote; repeat it for each.
    #[arg(long = "file")]
    pub files: Vec<String>,
    /// prepare: the probe, a JSON list of {"try","command","exit","tail"}.
    #[arg(long)]
    pub probe: Option<String>,
    /// prepare: the levels tried, a JSON list of {"environment","level","launch","model"}.
    #[arg(long)]
    pub levels: Option<String>,
    /// harden: the change that let it through, or none.
    #[arg(long)]
    pub change: Option<String>,
    /// harden: where it got through, or what no longer adds anything.
    #[arg(long)]
    pub crack: Option<String>,
    /// harden: strengthen, relax or retire a check.
    #[arg(long, value_parser = ["strengthen", "relax", "retire"])]
    pub action: Option<String>,
    /// harden: the check that should catch the defect, or the check under consideration.
    #[arg(long)]
    pub expected_check: Option<String>,
    /// harden: who or what found the defect (none if there was no defect).
    #[arg(long)]
    pub caught_by: Option<String>,
    /// harden: what the check caught over the period reviewed.
    #[arg(long)]
    pub catches: Option<String>,
    /// harden: what the check cost over that period, including missing measurements.
    #[arg(long)]
    pub cost: Option<String>,
    /// harden: the file and Rule of its scenario, or none.
    #[arg(long)]
    pub scenario: Option<String>,
    /// harden: the blueprint change it proposes.
    #[arg(long)]
    pub proposal: Option<String>,
    /// exam: how many questions it was asked.
    #[arg(long)]
    pub asked: Option<i64>,
    /// exam: a question it missed, by its id; repeat it for each.
    #[arg(long)]
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

fn prepare_fields(ctx: &Ctx, a: &StepArgs, ev: &mut Value) -> R<()> {
    if !matches!(a.result.as_deref(), Some("proposed" | "approved")) {
        return Err(Fail::usage("--result is proposed or approved"));
    }
    for file in &a.files {
        if !ctx.root.join(file).exists() {
            return Err(Fail::missing(
                file,
                format!("{file} is not in the working copy"),
            ));
        }
    }
    ev["files"] = json!(a.files);
    ev["probe"] = json_list(&a.probe, "--probe", &["try", "command", "exit", "tail"])?;
    ev["levels"] = json_list(
        &a.levels,
        "--levels",
        &["environment", "level", "launch", "model"],
    )?;
    Ok(())
}

fn harden_fields(a: &StepArgs, ev: &mut Value) -> R<()> {
    let (Some(crack), Some(proposal)) = (&a.crack, &a.proposal) else {
        return Err(Fail::usage("a harden step takes --crack and --proposal"));
    };
    let change = a.change.as_deref().filter(|c| *c != "none");
    if let Some(id) = change {
        check_id(id)?;
    }
    ev["change"] = json!(change);
    ev["crack"] = json!(crack);
    ev["proposal"] = json!(proposal);
    ev["scenario"] = json!(a.scenario.as_deref().filter(|s| *s != "none"));
    ev["expected_check"] = json!(a.expected_check.as_deref().unwrap_or("unknown"));
    ev["caught_by"] = json!(a.caught_by.as_deref().unwrap_or("unknown"));
    for (key, value) in [
        ("action", &a.action),
        ("catches", &a.catches),
        ("cost", &a.cost),
    ] {
        if let Some(value) = value {
            ev[key] = json!(value);
        }
    }
    Ok(())
}

fn exam_fields(a: &StepArgs, ev: &mut Value) -> R<()> {
    let (Some(asked), Some(model)) = (a.asked, &a.model) else {
        return Err(Fail::usage(
            "an exam step takes --asked, --caught, --level and --model (the reviewer's)",
        ));
    };
    let caught: i64 = a
        .caught
        .as_deref()
        .unwrap_or("none")
        .parse()
        .map_err(|_| Fail::usage("--caught is how many questions it caught, a number"))?;
    if caught > asked || caught < 0 {
        return Err(Fail::usage("--caught is between 0 and --asked"));
    }
    ev["reviewer"] = json!({"level": a.level.as_deref().unwrap_or("deep"), "model": model});
    ev["asked"] = json!(asked);
    ev["caught"] = json!(caught);
    ev["missed"] = json!(a.missed);
    Ok(())
}

/// The orchestrator's own steps, `prepare`, `harden` and `exam`, in the shapes of the contract,
/// so nobody composes a whole report by hand. They go to the blueprint's log.
pub fn step_cmd(ctx: &Ctx, id: &str, st: &str, a: &StepArgs) -> Out {
    run_change(ctx, id, "step", |out| {
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
        let level = a.level.as_deref().unwrap_or("deep");
        let caught = a.caught.as_deref().unwrap_or("none");
        let mut ev = new_event(
            id,
            "step",
            &time::now_string(),
            obj(vec![("step", json!(st))]),
        );
        match st {
            "prepare" => {
                super::events::context_asked(&l)?;
                prepare_fields(ctx, a, &mut ev)?
            }
            "harden" => harden_fields(a, &mut ev)?,
            "exam" => exam_fields(a, &mut ev)?,
            _ => return Err(Fail::usage("the step is prepare, harden or exam")),
        }
        if st != "exam" {
            ev["report"] = report_obj(
                &format!("gdd-{st}"),
                a.skill_version
                    .as_deref()
                    .or(Some(env!("CARGO_PKG_VERSION"))),
                Some(level),
                a.model.as_deref(),
                round,
                a.result.as_deref().unwrap_or("proposed"),
                caught,
                a.seconds,
                a.tokens,
            );
        }
        let line = write(&l, id, &ev)?;
        out.say(format!(
            "wrote the {st} step to gdd/log/blueprint.jsonl, line {line}, round {round}"
        ));
        out.fact("line", line);
        out.fact("round", round);
        Ok(())
    })
}
