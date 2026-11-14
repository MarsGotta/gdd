//! Shipping: what goes in, the bookkeeping, the merge and the record of it.

use super::*;

pub fn ship_describe(ctx: &Ctx, id: &str) -> Out {
    run("ship", |out| {
        let l = load(ctx, id)?;
        if l.state.next.step != "ship" {
            return Err(Fail::refuse(
                "out-of-order",
                format!("{id} isn't ready to ship: {}", l.state.next.say),
                way_on(&l, id),
            ));
        }
        let events = log::events(&l.entries);
        let brief = l.project.brief(id);
        let text = crate::ship::describe(&l.state, &events, brief.as_ref());
        for line in text.lines() {
            out.say(line.to_string());
        }
        out.fact("description", json!(text));
        Ok(())
    })
}

/// The head a change ships: the one the state was derived on (named with `--head`, else the tip
/// of its branch), else, with no branch, the commit that was judged.
fn ship_head(l: &Loaded, id: &str) -> R<String> {
    l.state
        .git
        .head
        .clone()
        .or_else(|| judged_commit(l).map(|(_, j)| j))
        .ok_or(Fail::missing(
            format!("the head of {id}"),
            format!("no head for {id}: no branch and no gates step names one; pass --head"),
        ))
}

/// The `<start>` and the commit the change was judged on: the head its last passing gates judged,
/// from the start they judged it from; for a setup log, the head its trial judged.
pub(super) fn judged_commit(l: &Loaded) -> Option<(String, String)> {
    let p = &l.state.progress;
    if p.kind == Some(LogKind::Setup) {
        let r = p.last_review.as_ref().filter(|r| r.verdict == "pass")?;
        Some((r.start.clone().unwrap_or_default(), r.head.clone()?))
    } else {
        let g = p.last_gates.as_ref().filter(|g| g.passed)?;
        Some((g.start.clone(), g.head.clone()?))
    }
}

/// What the state must allow before anything ships: the ship step itself, and a next step that
/// is shipping, derived on the head that ships. `--head` only names that head: it never stands in
/// for the state being ready, so a head nobody judged never goes in.
fn ready_to_ship(l: &Loaded, id: &str) -> R<()> {
    allowed(l, id, "ship", None)?;
    if l.state.next.step != "ship" {
        return Err(Fail::refuse(
            "out-of-order",
            format!("{id} isn't ready to ship: {}", l.state.next.say),
            way_on(l, id),
        ));
    }
    Ok(())
}

/// The commit the change was judged on, which `head` must be, or stand for by the layer's own
/// rule (it differs only in the layer's bookkeeping); and on a change, the review judged it too.
fn judged_for(ctx: &Ctx, l: &Loaded, id: &str, head: &str) -> R<String> {
    let gv = &l.state.git;
    let unjudged = |why: String| Fail::refuse("unjudged", why, format!("gdd next {id}"));
    let Some((_, judged)) = judged_commit(l) else {
        return Err(unjudged(
            "no step the layer wrote names the commit that was judged".into(),
        ));
    };
    let stands = |a: &str, b: &str| {
        a == b || gv.stands_for(a, b) || crate::judged::only_bookkeeping(&ctx.root, id, a, b)
    };
    if !stands(&judged, head) {
        return Err(unjudged(format!(
            "{} is not {}, the commit that was judged, and differs from it in more than the layer's bookkeeping",
            git::short(head),
            git::short(&judged)
        )));
    }
    if l.state.progress.kind != Some(LogKind::Setup) {
        let reviewed = l
            .state
            .progress
            .last_review
            .as_ref()
            .and_then(|r| r.head.clone());
        if !reviewed.is_some_and(|r| stands(&r, &judged) || stands(&r, head)) {
            return Err(unjudged(format!(
                "the review didn't judge {}, the commit its gates judged",
                git::short(&judged)
            )));
        }
    }
    Ok(judged)
}

/// The orchestrator's bookkeeping in the working copy that travels with a change: the backlog,
/// and of the logs and journals directly in `gdd/log/` named for a valid id
/// ([`crate::judged::is_bookkeeping`]), the change's own, the blueprint's, and those of every change that
/// is over, shipped or dropped. A change still under way ships its log with itself.
fn bookkeeping_files(l: &Loaded, root: &Path, id: &str) -> Vec<String> {
    let travels = |x: &str| {
        if x == id || x == "blueprint" {
            return true;
        }
        let p = state::Progress::of(x, &l.project.read_log(x));
        p.shipped.is_some() || p.brief_stopped.is_some()
    };
    let mut out = vec![];
    if let Ok(d) = std::fs::read_dir(root.join("gdd/log")) {
        for e in d.flatten() {
            if e.file_type().is_ok_and(|t| t.is_file())
                && let Ok(n) = e.file_name().into_string()
                && crate::judged::is_bookkeeping(&format!("gdd/log/{n}"))
                && n.strip_suffix(".engine.jsonl")
                    .or_else(|| n.strip_suffix(".jsonl"))
                    .is_some_and(travels)
            {
                out.push(format!("gdd/log/{n}"));
            }
        }
    }
    out.sort();
    if root.join("gdd/backlog.md").is_file() {
        out.push("gdd/backlog.md".into());
    }
    out
}

/// The change's branch and its tip, which must still be `head`, the commit the state was derived
/// on: a branch that moved since carries what nobody judged.
fn branch_at(ctx: &Ctx, l: &Loaded, id: &str, head: &str) -> R<(String, String)> {
    let branch = l.state.git.branch.clone().ok_or(Fail::missing(
        format!("the branch of {id}"),
        format!("no branch for {id} (work/{id}, gdd-build/{id}, build/{id}, or gdd-seal/{id} for a setup log): pass --branch"),
    ))?;
    let reference = format!("refs/heads/{branch}");
    let tip = git::commit_of(&ctx.root, &reference).ok_or(Fail::missing(
        reference.clone(),
        format!("{branch} is not a local branch"),
    ))?;
    if tip != head {
        return Err(branch_moved(&branch, &tip, head, id));
    }
    Ok((branch, tip))
}

fn branch_moved(branch: &str, tip: &str, head: &str, id: &str) -> Fail {
    Fail::refuse(
        "branch-moved",
        format!(
            "{branch} is at {}, and the commit that stands judged is {}: the branch moved, and what it holds now wasn't judged",
            git::short(tip),
            git::short(head)
        ),
        format!("gdd next {id}: it names what judges the branch as it is now"),
    )
}

/// Commit the bookkeeping on top of the change's branch, which must be at `head`, when its tip
/// doesn't hold it as it is. Returns the new tip, and whether it moved. The commit it makes goes
/// to the call's journal with the blob of each file it changed: that is how the layer knows later
/// that a head differs from the judged one only by its own bookkeeping.
fn commit_bookkeeping(
    ctx: &Ctx,
    l: &Loaded,
    id: &str,
    head: &str,
    out: &mut Out,
) -> R<(String, bool)> {
    let (branch, tip) = branch_at(ctx, l, id, head)?;
    let reference = format!("refs/heads/{branch}");
    let files = bookkeeping_files(l, &ctx.root, id);
    if files.is_empty() {
        return Ok((tip, false));
    }
    let worktrees = git::out(&ctx.root, &["worktree", "list", "--porcelain"]).unwrap_or_default();
    if worktrees
        .lines()
        .any(|w| w.trim() == format!("branch {reference}"))
    {
        out.say(format!(
            "note: {branch} is checked out in a worktree, which now trails it by the bookkeeping: remove it once the change is in"
        ));
    }
    let msg = format!("Bookkeeping of {id}: its log, the journals and the backlog");
    let c = seal::commit_on(&ctx.root, &reference, &tip, &files, &msg)
        .map_err(|e| Fail::refuse("bookkeeping", e, format!("gdd next {id}")))?;
    if !c.unchanged {
        let changed = git::diff_names(&ctx.root, &tip, &c.commit).unwrap_or_default();
        let blobs: Map<String, Value> = c
            .files
            .iter()
            .filter(|(f, _)| changed.contains(f))
            .map(|(f, h)| (f.clone(), json!(h)))
            .collect();
        let mut journal = match out.journal.take() {
            Some(Value::Object(m)) => m,
            _ => Map::new(),
        };
        journal.insert(
            "bookkeeping".into(),
            json!({"commit": c.commit, "parent": tip, "files": blobs}),
        );
        out.journal = Some(Value::Object(journal));
    }
    Ok((c.commit, !c.unchanged))
}

pub fn ship_bookkeeping(ctx: &Ctx, id: &str) -> Out {
    run("ship", |out| {
        let l = load(ctx, id)?;
        need_repo(ctx, "the bookkeeping is committed with git")?;
        git_writes(ctx)?;
        git_commits(ctx)?;
        ready_to_ship(&l, id)?;
        let head = ship_head(&l, id)?;
        judged_for(ctx, &l, id, &head)?;
        let (commit, moved) = commit_bookkeeping(ctx, &l, id, &head, out)?;
        if moved {
            out.say(format!(
                "committed the logs, the journals and the backlog on {} as {commit}; the checkout and its index were not touched",
                l.state.git.branch.clone().unwrap_or_default()
            ));
        } else {
            out.say(format!(
                "{} already holds the bookkeeping as it is, at {}",
                l.state.git.branch.clone().unwrap_or_default(),
                git::short(&commit)
            ));
        }
        out.say(format!(
            "what the layer logs from now on travels with the next change; next: open the pull request, and once it is merged: gdd ship {id} --done <where>"
        ));
        out.fact("commit", json!(commit));
        out.fact("moved", moved);
        Ok(())
    })
}

/// One entry of `git status --porcelain=v1 -z`: its two status letters and its path.
fn status_entries(root: &Path) -> R<Vec<(char, char, String)>> {
    let o = git::run(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    .map_err(|e| Fail::missing("the status of the checkout", e))?;
    let parts: Vec<String> = o
        .stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();
    let mut out = vec![];
    let mut i = 0;
    while i < parts.len() {
        let e = &parts[i];
        if e.len() < 4 {
            i += 1;
            continue;
        }
        let mut c = e.chars();
        let (x, y) = (c.next().unwrap_or(' '), c.next().unwrap_or(' '));
        out.push((x, y, e[3..].to_string()));
        i += if x == 'R' || x == 'C' { 2 } else { 1 };
    }
    Ok(out)
}

pub fn ship_merge(ctx: &Ctx, id: &str) -> Out {
    run("ship", |out| {
        let l = load(ctx, id)?;
        need_repo(ctx, "a merge needs git")?;
        git_writes(ctx)?;
        git_commits(ctx)?;
        let main = git::main_branch(&ctx.root);
        ready_to_ship(&l, id)?;
        let head = ship_head(&l, id)?;
        let judged = judged_for(ctx, &l, id, &head)?;
        let on = git::out(&ctx.root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .unwrap_or_default();
        if on != main {
            return Err(Fail::refuse(
                "not-on-main",
                format!(
                    "the checkout is on {}, not on {main}",
                    if on.is_empty() {
                        "a detached head".into()
                    } else {
                        on
                    }
                ),
                format!("check out {main} in the orchestrator's copy, then: gdd ship {id} --merge"),
            ));
        }
        let branch = l.state.git.branch.clone().ok_or(Fail::missing(
            format!("the branch of {id}"),
            format!("no branch for {id}: pass --branch"),
        ))?;
        // The bookkeeping first, so the branch holds the copy's logs and backlog as they are. The
        // branch must still be at the head that stands judged, and what merges is that commit,
        // with the bookkeeping on top, by its hash.
        let (tip, moved) = commit_bookkeeping(ctx, &l, id, &head, out)?;
        if moved {
            out.say(format!(
                "committed the bookkeeping on {branch} as {}",
                git::short(&tip)
            ));
            if !crate::judged::only_bookkeeping(&ctx.root, id, &judged, &tip) {
                return Err(Fail::refuse(
                    "unjudged",
                    format!(
                        "{} differs from {}, the commit that was judged, in more than the layer's bookkeeping",
                        git::short(&tip),
                        git::short(&judged)
                    ),
                    format!("gdd next {id}"),
                ));
            }
        }
        // Nothing else may be pending: what the copy changed is either bookkeeping, or a file the
        // branch holds as it is (such as the brief and its scenarios), or nothing the merge touches.
        let mut aside: Vec<String> = vec![];
        let mut pending: Vec<String> = vec![];
        for (x, y, path) in status_entries(&ctx.root)? {
            let untracked = x == '?' && y == '?';
            if !untracked && x != ' ' {
                pending.push(format!("{path} (staged)"));
                continue;
            }
            if crate::judged::is_bookkeeping(&path) {
                if ctx.root.join(&path).is_file() {
                    aside.push(path);
                }
                continue;
            }
            if untracked {
                match git::blob_at(&ctx.root, &tip, &path) {
                    Some(b)
                        if git::hash_object(&ctx.root, &path).ok().as_deref()
                            == Some(b.as_str()) =>
                    {
                        aside.push(path)
                    }
                    Some(_) => {
                        pending.push(format!("{path} (your copy differs from the change's)"))
                    }
                    None => {}
                }
                continue;
            }
            pending.push(format!("{path} (changed)"));
        }
        if !pending.is_empty() {
            return Err(Fail::refuse(
                "dirty-tree",
                format!(
                    "something else is pending in the checkout: {}",
                    pending.join(", ")
                ),
                "commit or set it aside yourself; a correction of the brief is a new version",
            ));
        }
        // Move aside what the merge will bring, as it is.
        let keep = runs_dir(&ctx.root, id, "merge", out)?.join("aside");
        for p in &aside {
            let from = ctx.root.join(p);
            let to = keep.join(p);
            if let Some(d) = to.parent() {
                crate::runs::create_dirs(d).map_err(|e| crate::out::io(d, e))?;
            }
            crate::runs::copy_new(&from, &to).map_err(|e| crate::out::io(&to, e))?;
            let tracked = git::out(&ctx.root, &["ls-files", "--error-unmatch", "--", p]).is_ok();
            if tracked {
                git::out(&ctx.root, &["checkout", "--", p])
                    .map_err(|e| Fail::missing(p.clone(), e))?;
            } else {
                std::fs::remove_file(&from).map_err(|e| crate::out::io(&from, e))?;
            }
        }
        let put_back = |all: bool| -> Vec<String> {
            let mut back = vec![];
            let head = git::commit_of(&ctx.root, "HEAD");
            for p in &aside {
                let saved = keep.join(p);
                let brought = head
                    .as_ref()
                    .and_then(|h| git::blob_bytes(&ctx.root, h, p))
                    .zip(std::fs::read(&saved).ok())
                    .is_some_and(|(a, b)| a == b);
                if all || !brought {
                    let to = ctx.root.join(p);
                    if let Some(d) = to.parent() {
                        let _ = std::fs::create_dir_all(d);
                    }
                    let _ = std::fs::copy(&saved, &to);
                    back.push(p.clone());
                }
            }
            back
        };
        // What merges is the commit that stands judged, by its hash, never the branch by its name:
        // if the branch moved meanwhile, nothing merges.
        let now = git::commit_of(&ctx.root, &format!("refs/heads/{branch}"));
        if now.as_deref() != Some(tip.as_str()) {
            put_back(true);
            return Err(branch_moved(
                &branch,
                now.as_deref().unwrap_or("nothing"),
                &tip,
                id,
            ));
        }
        let msg = format!("Merge {branch}: {id}");
        let merged = git::run_env(
            &ctx.root,
            &["merge", "--no-ff", "--no-edit", "-m", &msg, &tip],
            &[("GIT_MERGE_AUTOEDIT", "no")],
        )
        .map_err(|e| Fail::missing("git merge", e))?;
        if !merged.status.success() {
            let conflicts = git::out(&ctx.root, &["diff", "--name-only", "--diff-filter=U"])
                .unwrap_or_default();
            let conflicts: Vec<&str> = conflicts.lines().filter(|l| !l.is_empty()).collect();
            if !conflicts.is_empty() && conflicts.iter().all(|p| crate::judged::is_bookkeeping(p)) {
                // The copy's bookkeeping is what reaches the main branch: the branch holds it.
                for p in &conflicts {
                    git::out(&ctx.root, &["checkout", "--theirs", "--", p])
                        .map_err(|e| Fail::missing(p.to_string(), e))?;
                    git::out(&ctx.root, &["add", "--", p])
                        .map_err(|e| Fail::missing(p.to_string(), e))?;
                }
                git::out_env(
                    &ctx.root,
                    &["commit", "--no-edit", "-m", &msg],
                    &[("GIT_MERGE_AUTOEDIT", "no")],
                )
                .map_err(|e| Fail::missing("the merge commit", e))?;
                out.say(format!(
                    "the bookkeeping conflicted in {}: the copy's version went in",
                    conflicts.join(", ")
                ));
            } else {
                let _ = git::out(&ctx.root, &["merge", "--abort"]);
                put_back(true);
                return Err(Fail::refuse(
                    "conflict",
                    format!(
                        "merging {branch} into {main} conflicts{}",
                        if conflicts.is_empty() {
                            format!(": {}", String::from_utf8_lossy(&merged.stderr).trim())
                        } else {
                            format!(" in {}", conflicts.join(", "))
                        }
                    ),
                    format!(
                        "bring the change up to date first: gdd seal {id} --merge-main, merge it into {branch}, gdd gates {id}"
                    ),
                ));
            }
        }
        let back = put_back(false);
        if !back.is_empty() {
            out.say(format!(
                "put back what the merge didn't bring: {}",
                back.join(", ")
            ));
        }
        let main_tip = git::commit_of(&ctx.root, &main).ok_or(Fail::missing(
            format!("refs/heads/{main}"),
            format!("there is no {main} branch"),
        ))?;
        if !git::is_ancestor(&ctx.root, &tip, &main_tip) {
            return Err(Fail::refuse(
                "not-merged",
                format!("{} is not in {main} after the merge", git::short(&tip)),
                format!(
                    "look at git log {main} and tell the person: the merge didn't take {}",
                    git::short(&tip)
                ),
            ));
        }
        out.say(format!(
            "merged {} ({branch}) into {main} as {}",
            git::short(&tip),
            git::short(&main_tip)
        ));
        let line = write_ship(&l, id, &main_tip, &tip, None, out)?;
        out.fact("line", line);
        out.fact("head", json!(tip));
        out.fact("merge", json!(main_tip));
        // Once this call is journaled, its ship line, its journal and the backlog go on the main
        // branch in a commit of their own, so the checkout holds nothing of the layer's.
        out.commit_log = Some(format!("Log the ship of {id}"));
        Ok(())
    })
}

/// Write the `ship` step and set the change's backlog item to done. `matched_in` is the commit of
/// the main branch that holds the judged content when the change's commits aren't in it.
fn write_ship(
    l: &Loaded,
    id: &str,
    place: &str,
    head: &str,
    matched_in: Option<&str>,
    out: &mut Out,
) -> R<usize> {
    let main = git::main_branch(&l.project.root);
    let color = l.state.progress.color.clone().unwrap_or_default();
    let mut m = obj(vec![
        ("step", json!("ship")),
        ("where", json!(place)),
        ("color", json!(color)),
        ("head", json!(head)),
    ]);
    if let Some(at) = matched_in {
        m.insert("matched".into(), json!("content"));
        m.insert("in".into(), json!(at));
    }
    let ev = new_event(id, "step", &time::now_string(), m);
    let line = write(l, id, &ev)?;
    out.say(format!(
        "{} is in {main}: wrote the ship step, line {line}; the log of {id} is closed",
        git::short(head)
    ));
    let path = l.project.path("gdd/backlog.md");
    if let Ok(text) = std::fs::read_to_string(&path) {
        let items = backlog::items(&text);
        let named = l
            .project
            .brief(id)
            .and_then(|(f, _)| f["backlog"].as_str().map(str::to_string));
        let item = items
            .iter()
            .find(|i| Some(&i.id) == named.as_ref())
            .or_else(|| items.iter().find(|i| i.brief == id))
            .or_else(|| items.iter().find(|i| i.id == id));
        match item {
            Some(i) if i.state != "done" => {
                let t = backlog::set(&text, &i.id, "state", "done")
                    .map_err(|e| Fail::missing("gdd/backlog.md", e))?;
                std::fs::write(&path, t).map_err(|e| crate::out::io(&path, e))?;
                out.say(format!(
                    "set {} to done in the backlog: say so only once what it builds is in use",
                    i.id
                ));
                out.fact("backlog", json!(i.id));
            }
            Some(i) => out.say(format!("{} was already done in the backlog", i.id)),
            None => out.say(format!("no backlog item names {id}")),
        }
    }
    // A zone drawn before its code existed can name a path nobody wrote, and what was built
    // for it lands in another zone, or in none. What ships is when it shows.
    if let Ok(z) = color::zones(&l.project.root, &l.bp)
        && !z.empty.is_empty()
    {
        let mut names: Vec<String> = vec![];
        for (zone, _) in &z.empty {
            if !names.contains(zone) {
                names.push(zone.clone());
            }
        }
        out.say(format!(
            "note: pathspecs of these zones match no tracked file: {}. What was built for them may be judged in another zone, or in none: gdd zones shows it, and correcting a path is a change to the blueprint, for the person",
            names.join(", ")
        ));
        out.fact("empty_zones", json!(names));
    }
    Ok(line)
}

pub fn ship_done(ctx: &Ctx, id: &str, place: &str) -> Out {
    run("ship", |out| {
        let l = load(ctx, id)?;
        need_repo(ctx, "shipping is checked against git")?;
        let main = git::main_branch(&ctx.root);
        ready_to_ship(&l, id)?;
        let head = ship_head(&l, id)?;
        let judged = judged_for(ctx, &l, id, &head)?;
        let main_tip = git::commit_of(&ctx.root, &main).ok_or(Fail::missing(
            format!("refs/heads/{main}"),
            format!("there is no {main} branch"),
        ))?;
        let gv = &l.state.git;
        let stands = |c: &str| {
            c == judged
                || gv.stands_for(&judged, c)
                || crate::judged::only_bookkeeping(&ctx.root, id, &judged, c)
        };
        // The change's branch is what a pull request merges. While it exists, whatever its tip
        // holds is what a merge of it brought, --head or not: past what was judged, it can't ship.
        let branch_tip = gv.branch.as_ref().and_then(|b| {
            git::commit_of(&ctx.root, &format!("refs/heads/{b}")).map(|t| (b.clone(), t))
        });
        if let Some((branch, tip)) = &branch_tip
            && !stands(tip)
        {
            return Err(Fail::refuse(
                "unjudged",
                format!(
                    "{branch} is at {}, which differs from {}, the commit that was judged, in more than the layer's bookkeeping: what a merge of it brought wasn't judged",
                    git::short(tip),
                    git::short(&judged)
                ),
                format!("gdd next {id}: it names what judges the branch as it is now"),
            ));
        }
        let bound = branch_tip
            .map(|(_, t)| t)
            .or_else(|| ctx.head.as_ref().and(gv.head.clone()));
        // What went in, from git: the merge that brought it, a fast-forward bounded by what comes
        // after it, or, by squash or rebase, exactly the content that was judged and nothing else.
        let (start, _) = judged_commit(&l).unwrap_or_default();
        let went = crate::ship::went_in(
            &ctx.root,
            id,
            &main,
            &main_tip,
            &start,
            &judged,
            bound.as_deref(),
            &stands,
        )
        .map_err(|n| {
            let instead = match n.code {
                "which-tip" => format!(
                    "name the tip that was merged: gdd ship {id} --done {place} --head <that commit>"
                ),
                "unjudged" => format!("tell the person: what went into {main} wasn't judged"),
                _ => format!(
                    "merge it as the constitution says (gdd ship {id} --merge, or a pull request after gdd ship {id} --bookkeeping), then: gdd ship {id} --done <where>"
                ),
            };
            Fail::refuse(n.code, n.why, instead)
        })?;
        let (shipped, matched_in) = match went {
            crate::ship::WentIn::Merged { tip, merge } => {
                out.say(format!(
                    "{main} took {} by the merge {}",
                    git::short(&tip),
                    git::short(&merge)
                ));
                (tip, None)
            }
            crate::ship::WentIn::FastForward { tip } => {
                out.say(format!(
                    "{main} took {} by a fast-forward",
                    git::short(&tip)
                ));
                (tip, None)
            }
            crate::ship::WentIn::Content { files, at } => {
                out.say(format!(
                    "matched by content: {main} took at {} exactly what was judged at {} in each of the {files} files the change touched, and nothing else, though not its commits (a merge by squash or rebase)",
                    git::short(&at),
                    git::short(&judged)
                ));
                out.fact("matched", "content");
                (judged.clone(), Some(at))
            }
        };
        let line = write_ship(&l, id, place, &shipped, matched_in.as_deref(), out)?;
        out.say(
            "the ship line stays in this copy and travels with the next change's bookkeeping: it is not something to report to the person",
        );
        out.fact("line", line);
        out.fact("head", json!(shipped));
        Ok(())
    })
}

/// What a command leaves for after its call is journaled: for `ship --merge`, committing the
/// bookkeeping the main branch doesn't hold yet (`gdd/log/**`, `gdd/backlog.md`), in one commit
/// that touches nothing else. A program that uses the library calls it after the journal.
pub fn finish(ctx: &Ctx, out: &mut Out) {
    let Some(msg) = out.commit_log.take() else {
        return;
    };
    let main = git::main_branch(&ctx.root);
    let not_done = |why: &str| {
        format!(
            "the ship's line and the journal stay uncommitted on {main} ({why}): that is bookkeeping, which travels with the next change, not something to report to the person"
        )
    };
    let on =
        git::out(&ctx.root, &["symbolic-ref", "--quiet", "--short", "HEAD"]).unwrap_or_default();
    if on != main {
        out.say(not_done(&format!("the checkout is on {on}")));
        return;
    }
    let paths: Vec<&str> = ["gdd/log", "gdd/backlog.md"]
        .into_iter()
        .filter(|p| ctx.root.join(p).exists())
        .collect();
    let mut add = vec!["add", "-A", "--"];
    add.extend(&paths);
    if let Err(e) = git::out(&ctx.root, &add) {
        out.say(not_done(e.lines().last().unwrap_or("git add failed")));
        return;
    }
    let mut staged = vec!["diff", "--cached", "--quiet", "--"];
    staged.extend(&paths);
    if git::run(&ctx.root, &staged).is_ok_and(|o| o.status.success()) {
        return;
    }
    let mut commit = vec!["commit", "-q", "-m", msg.as_str(), "--"];
    commit.extend(&paths);
    match git::out(&ctx.root, &commit) {
        Ok(_) => {
            let sha = git::commit_of(&ctx.root, "HEAD").unwrap_or_default();
            out.say(format!(
                "committed the ship's bookkeeping on {main} as {}: {msg}; nothing of the layer's is left in the checkout",
                git::short(&sha)
            ));
            out.fact("logged", json!(sha));
        }
        Err(e) => out.say(not_done(e.lines().last().unwrap_or("git commit failed"))),
    }
}
