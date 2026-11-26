# Returning a fix to its agent

If every finding names a test sealed for this change, resume the checks agent with the findings. Then run `gdd launch <id> build`. When the product is the one reviewed, the layer merges the corrected seal itself, no builder runs, and `gdd gates <id>` comes next. Otherwise it prints the prompt that resumes the builder to merge it and run the required checks, and while the product matches the reviewed version this merge uses no build round. The gates run, leaving out on that product what only reports outside red, and a fresh reviewer checks only the corrected tests. If no finding names a test sealed for this change, resume the builder with the whole review. For mixed findings, the layer names `checks` first to correct the named tests. Then resume the builder to merge the seal and address the rest, including findings without a file. If the checks agent answers `CANNOT:` to findings confined to its tests, the builder takes every finding in a normal fix round: run what the layer prints. Only the checks agent or a person edits a sealed test.

Send the prompt from `gdd launch <id> <role>` to the last builder or checks agent for this change and role. It gives the seal tip, what sent the change back and what the last builder tried. When the reviewer returns the change, the prompt keeps that review’s findings and citations in full, along with its notes for the builder. A command’s output may show only its end. The agent can run it again if it needs more.

When the role first ran with `--run`, use the `session_id` that Codex or Claude printed for it. A shortcut that picks the latest session could send the fix to the reviewer. A returned role needs an explicit resume command because a plain `--run` would start a new session and the layer refuses it. Keep the original launch settings. For a builder started with `codex exec --json -m <model>`, put its exact id in place of `SESSION_ID`.

```sh
gdd launch <id> build --run --command 'codex exec resume --json -m <model> SESSION_ID <prompt>'
```

The layer records the round. For Codex it compares the thread counters with the previous step for that role. Keep the session for that role alone so the difference measures its work. If the session has done other work, resume by hand and report only what you can measure for this round.

If the role used an agent tool, resume its handle with the printed prompt on the named model. Start a fresh agent by hand only if the original session cannot resume. After a manual launch, use `gdd report`. Pass measured round counts with `--tokens` for the cap and `--tokens-read` for all input, including cache reads and writes. Leave out any count you cannot measure.

When the layer sends a reviewer's report back (`unbacked-fix` or `unbacked-pass`), resume that reviewer the same way, with the session the layer printed for it: a fresh one would start the review over.

A builder that believes a finding is wrong contests it in its report instead of changing the product, with a line the reviewer can read. The prompt of its fix round says how. A contest closes nothing: the next reviewer reads the line and closes the finding as `false` or `intended`, or keeps it. If the layer refuses the report (`unbacked-contest`), resume the builder with what the refusal says. A finding the reviewer kept isn't contested again (`contest-upheld`). The builder fixes it or stops with `CANNOT:`, and what only the person can settle goes to them with `gdd ask <id> --of decision`.
