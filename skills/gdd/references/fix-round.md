# Returning a fix to its agent

Use the session of the last builder or checks agent for this change and role. The layer prints the next round's prompt with what failed. In three runs, starting a fresh agent for every fix made it read the brief and code all over again.

When the first launch used `--run`, keep the `session_id` it printed for that role. Codex and Claude include it in their JSON output. To send the fix directly, use the same launch settings with the environment's resume option, that exact id and the model placeholder. For example, for a Codex launch of `codex exec --json -m <model>`, replace `SESSION_ID` with the builder's id in this command:

```sh
gdd launch <id> build --run --command 'codex exec resume --json -m <model> SESSION_ID <prompt>'
```

The layer supplies the new prompt and records the step. A shortcut that selects the latest session could send the fix to the reviewer. When the environment reports only a running token total for the whole session, resume it by hand and report only this round's usage when available. If the first launch used an agent tool, resume its handle with the prompt from `gdd launch <id> <role>` on the named model and record the result with `gdd report`. Start a fresh role only when the original session cannot be resumed.
