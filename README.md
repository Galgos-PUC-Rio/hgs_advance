# hgs_advance

A standalone command-line program that solves classical **Vehicle Routing
Problem with Time Windows (VRPTW)** and **Capacitated Vehicle Routing Problem
(CVRP)** benchmark instances — the kind found on
[CVRPLib](http://vrp.galgos.inf.puc-rio.br/) — using `hgs_advance`, a Hybrid
Genetic Search algorithm originally developed by Thibaut Vidal for
[The Innovation Game (TIG)](https://tig.foundation/).

This is a research/experimentation tool, not a polished product. This README
assumes no prior experience with Rust.

**License:** not yet decided — see the note at the bottom of this file before
using this for anything beyond your own local experiments.

## 1. Install Rust (one-time setup)

You need the Rust compiler and its package manager, `cargo`. If you already
have them (check with `cargo --version` in a terminal), skip to step 2.

- **Windows / macOS / Linux:** go to <https://rustup.rs> and follow the one-line
  install command shown there. On Windows it will also prompt you to install
  the "Visual Studio C++ Build Tools" if you don't already have them — accept
  that, it's required to compile Rust programs.
- After installing, close and reopen your terminal, then check it worked:

```bash
cargo --version
```

You should see something like `cargo 1.xx.0`.

## 2. Build the program

From inside this folder (`hgs_advance/`), run:

```bash
cargo build --release
```

This downloads a handful of small dependencies and compiles the program. It
takes about 30-60 seconds the first time. When it's done, the executable is
at:

- Windows: `target\release\hgs_advance.exe`
- macOS/Linux: `target/release/hgs_advance`

You only need to do this once (or again if you change the code).

## 3. Run it on an instance file

Download a classical instance file — e.g. a Solomon `C101.txt` or a
Gehring-Homberger instance for VRPTW, or a CVRPLib `.vrp` file for CVRP — and
run:

```bash
# Windows (PowerShell or cmd)
.\target\release\hgs_advance.exe --instance C:\path\to\C101.txt --output-dir results

# macOS/Linux
./target/release/hgs_advance --instance /path/to/C101.txt --output-dir results
```

This will:
1. Read the instance file.
2. Run the solver (using sensible defaults if you don't specify anything else).
3. Print a one-line summary (number of routes, total distance).
4. Write the solution to `results/C101.sol` (created automatically).

The `.sol` file looks like this:

```
Route #1: 13 17 18 19 15 16 14 12
Route #2: 5 3 7 8 10 11 9 6 4 2 1
Route #3: 20 24 23 22 21
Cost 190.90
NB_ROUTES: 3
CPU_TIME: 0.004207
```

Each `Route #k` line lists the customers visited, in order (the depot is
implicit at the start/end of each route). `Cost` is the total distance, already
in the same units used by CVRPLib/Solomon best-known-solution tables — no
manual conversion needed.

## 4. All the options

```
hgs_advance --instance <path> [--type vrptw|cvrp] [--output-dir <dir>]
          [--time-limit <seconds>] [--exploration-level <0-6>]
          [--param key=value ...] [--seed <0-255>]
```

| Flag | Required? | What it does |
|---|---|---|
| `--instance <path>` | **Yes** | Path to the instance file to solve. |
| `--type vrptw\|cvrp` | No | Forces the file format. If you omit it, the program guesses from the file contents (Solomon/Gehring-Homberger style => `vrptw`, CVRPLib/TSPLIB style with `NODE_COORD_SECTION`/`DIMENSION` => `cvrp`). Only needed if the auto-detection guesses wrong. |
| `--output-dir <dir>` | No | Folder to write `<instance-name>.sol` into (created if it doesn't exist). If you omit this, nothing is written to disk — you only get the printed summary. |
| `--time-limit <seconds>` | No | Stops the search after this many seconds and returns the best solution found so far. **Strongly recommended for anything but tiny instances** — see "How long should I run it?" below. Without it, the search runs until its own internal iteration budget is exhausted, which for large instances/high exploration levels can take a very long time. |
| `--exploration-level <0-6>` | No | How hard the solver searches. `0` = fastest/lowest quality, `6` = slowest/deepest search. Default is `4`. See the table below. |
| `--param key=value` | No | Advanced: overrides one specific internal parameter (see `src/params.rs`). Most people won't need this. Can be repeated for multiple overrides. |
| `--seed <0-255>` | No | Fixes the random seed, so re-running with the same seed reproduces the same result. Useful for debugging. |

### How long should I run it? (`--exploration-level` + `--time-limit`)

Think of these two flags together: `--exploration-level` picks *how* thoroughly
it searches, `--time-limit` caps *how long* you let it run. For small
instances the search naturally converges quickly on its own; for large
instances you want a high exploration level **and** an explicit time limit,
so it uses the time well instead of running forever.

| Instance size | Suggested `--exploration-level` | Suggested `--time-limit` |
|---|---|---|
| Small (< 200 customers) | `3`–`4` | usually converges in seconds; a time limit isn't essential but doesn't hurt |
| Medium (a few hundred) | `5` | 60–600 |
| Large (thousands, e.g. Gehring-Homberger) | `6` | several hours |
| Very large (~10,000 nodes) | `6` | e.g. `--time-limit 86000` for a ~24h run |

Example for a big overnight run, with a log file so you can check on progress
later, and results checkpointed as they improve (not just at the very end):

```bash
./target/release/hgs_advance --instance big_instance.txt --exploration-level 6 \
  --output-dir results/ --time-limit 86000 --param display_traces=true \
  > run.log 2>&1 &
```

The `.sol` file in `results/` is updated every time the solver finds a better
solution, not just when it finishes — so if you interrupt a long run, you
still have the best solution found so far on disk.

## 5. Troubleshooting

- **"No feasible solution found"** — this can happen on very tightly
  constrained instances at low exploration levels. Try a higher
  `--exploration-level`.
- **Build fails on Windows mentioning a linker** — you're missing the Visual
  Studio C++ Build Tools; rerun the rustup installer and accept that prompt,
  or install "Build Tools for Visual Studio" separately, then try
  `cargo build --release` again.
- **It seems to hang** — for large instances at high exploration levels this
  is expected without `--time-limit`; either wait, or re-run with a time
  limit and/or lower exploration level.

## About this code

This is an extracted, standalone version of `hgs_advance`, an "advance"
algorithm submission by Thibaut Vidal to
[The Innovation Game](https://github.com/tig-foundation/tig-monorepo)
(`tig-algorithms/src/vehicle_routing/hgs_advance`). It has been adapted here
to run directly on classical instance files (rather than TIG's own on-chain
challenge format) with a few practical additions: a wall-clock time limit,
configurable output location, incremental checkpointing of the best solution
found so far, and correctly-scaled costs in the `.sol` output. The underlying
optimization algorithm itself is unmodified.

**License:** this repository doesn't have a finalized license yet. The
original code is covered by TIG's own multi-license model (see
[TIG's licensing docs](https://github.com/tig-foundation/tig-monorepo/tree/main/docs/licenses)),
which has implications if results or code derived from it are published or
redistributed. Don't treat anything here as cleared for redistribution,
publication, or commercial use until that's explicitly sorted out.
