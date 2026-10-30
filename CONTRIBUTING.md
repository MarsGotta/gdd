# Contributing to gdd

Thank you for helping. This page says how to set up the repository, run the tests and send
a change, and how gdd's own gates judge it.

## gdd is built with GDD

Every change to this repository goes through GDD's gates, run by gdd itself: scenarios
approved first, the Builder in a worktree, G2 and G3 checks, a Reviewer, and a human at the
Release Gate when the change has new scenarios or touches a red zone. Your pull request meets
the same checks in CI (`bin/gdd ci`), judged by the gdd built from `main`, never by the code
under review. See [GDD in one page](docs/explanation/method.md) if the words are new.

## Before you start

- **Open an issue first** for anything larger than a typo: what you saw, what you expected,
  and how to reproduce it (`gdd --version`, the command, its output). A change that adds or
  changes behaviour needs scenarios, and scenarios need the maintainer's approval (G1).
- **Paths have colors.** [`gdd/zones.toml`](gdd/zones.toml) maps them: documentation is
  green; the gates, Path Guard, the agents and the repository code are red; everything else
  is yellow. A red change needs an approved plan before any code.

## Set up

You need Rust (the toolchain is pinned in [`rust-toolchain.toml`](rust-toolchain.toml) and
installed by rustup on first use), [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)
and [cargo-mutants](https://mutants.rs) for G3, and [gitleaks](https://github.com/gitleaks/gitleaks)
for the secrets check.

```bash
git clone https://github.com/MarsGotta/gdd
cd gdd
cargo install --locked cargo-deny cargo-mutants
cargo build --release --locked
mkdir -p .gdd/bin && install -m 755 target/release/gdd .gdd/bin/gdd-rust
bin/gdd status
```

`bin/gdd` runs `.gdd/bin/gdd-rust`, the binary built from `main`. Rebuild it from `main`
only, never from a change branch: a branch must not be able to rewrite its own judge.

## Test

```bash
cargo test --workspace --locked                      # unit tests and every scenario
GDD_FEATURES=features/report.feature cargo test -q -p gdd-acceptance   # one feature
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --all --check
bin/gdd selftest                                     # the harness checks itself (4/4)
```

The scenarios in [`features/`](features/) are gdd's executable specification, run with
[cucumber-rs](https://github.com/cucumber-rs/cucumber) in strict mode: an undefined step
fails.

## Send a change

1. Fork, then branch from `main`.
2. Keep the change small: one behaviour, its scenarios and its code.
3. Follow [`docs/standards.md`](docs/standards.md): what good looks like here, in words.
4. Open a pull request that links the issue. CI runs `bin/gdd ci`: Path Guard (no harness
   path changed against `main`), then the G2 and G3 commands of
   [`gdd/commands.toml`](gdd/commands.toml).
5. A maintainer brings the change through gdd's own gates and answers its Release Gate.

## Documentation

The docs follow [Diátaxis](https://diataxis.fr) and [their own style guide](docs/STYLE.md).
They are in English, with a Spanish translation in `docs/es/` that mirrors them page by page:
when you change an English page, change its Spanish twin too, or say in the pull request that
it still needs translating. Scripts check what can be checked:

```bash
python3 scripts/make_reference.py          # regenerate docs/reference/cli.md (and the es one) from --help
python3 scripts/check_md_links.py .        # relative links and anchors resolve
python3 scripts/check_translation.py       # every page has its Spanish twin, with the same code
python3 scripts/build_site.py              # build the website, English and Spanish
```

Change a command's help text in the code (`crates/gdd/src/lib.rs`), then regenerate the pages.

## License

gdd is dual-licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.
