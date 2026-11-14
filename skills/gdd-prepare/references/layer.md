# Setting up the layer

The layer is `gdd`, a command the skills need from version 0.11 on: it keeps the order of each change, counts its limits and writes its log. In a project it is the file `.gdd/gdd`, which git leaves out, so every new clone sets it up again.

## Where it comes from

From the `engine/` folder of the plugin the skills were loaded from: two folders up from this skill's own folder, next to `skills/`. Where the skills were copied by hand, it is the `engine/` folder of the GDD repository they came from.

1. Build it with `sh <plugin>/engine/build.sh`. It needs a Rust toolchain, Rust 1.89 or later, and builds outside the plugin's folder. Its last line is the path of the binary.
2. Copy that binary to `.gdd/gdd` in the project, and add a line `.gdd/` to the project's `.gitignore` if it isn't there.
3. Check it: `.gdd/gdd --version` names the layer and its version, and `.gdd/gdd next` ends with a line that starts with `gdd {`. Where a `gdd` on the path gives the same, that one does as well; GNU coreutils installs its `dd` under the same name, and that one is not the layer.

## When it can't be built

Without a Rust toolchain nothing builds, and the skills don't work by hand: tell the person in one line what it takes, and stop. "GDD needs a small tool built with Rust, which this computer doesn't have yet: install it from rustup.rs and I'll go on. Shall I wait?" Never write the log yourself meanwhile.

## What CI runs

On `rule`, the layer is within the reach of the agents that run it, like any file of the repository. What keeps it honest is CI, where no agent writes:

- `gdd verify --all` on every change: it exits 1 when a log is broken, a hash doesn't match or a step came out of order.
- `gdd gates <id> --at .` where the platform builds the change's branch, with its dependencies installed and its seal fetched (`gdd-seal/<id>`, which a full fetch brings): it runs the checks the change's color asks for on exactly that commit, from the seal git holds.

The gates run in a fresh copy of the repository, which holds only what git holds. When the checks need more, such as the project's dependencies, the blueprint names the command that installs them (`install: npm ci`): the layer runs it in that copy before any check. Without it, a check that ends with "command not found" there is the environment and not the change, and the layer says so and names `--at`.

A job that runs in a container often checks the repository out as another user, and git then refuses to read it. The layer refuses every command there, `verify` included, instead of going on without git, and names the line that marks the folder as safe: put it in the job, before the layer runs (`git config --global --add safe.directory "$PWD"`). Neither command writes a commit, so CI needs no name and email in git. A copy that seals or ships does, and where git has none the layer stops and asks whose they are: never make them up.

CI builds the layer from the same source, with `cargo install --locked --git https://github.com/MarsGotta/gdd gdd`. Both go in the protections you give the person to apply, as required checks of the main branch, because setting them up takes their credentials.
