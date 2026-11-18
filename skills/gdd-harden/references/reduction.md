# Evidence for relaxing or retiring a check

Read `gdd metrics --all --summary` for the check you are considering. Choose the changes or dates the proposal covers. Give the person one proposal with what would still reject the bad case.

In the harden step, use `--action relax` or `--action retire` and put the check’s id in `--expected-check`. If there was no defect, use `--caught-by none`. Include both pieces of evidence:

- In `--catches`, give the period, how often the check ran, what it found in gates and what builders reported. Say which counts are missing. A failure can be another run of the same defect.
- In `--cost`, give the measured time, tokens or wait over that period and say what went unmeasured.

`--catches "<period>: <runs> gate runs, <failures> failures after the builders finished, builder counts unknown" --cost "<seconds> measured"`
