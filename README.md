# hgs_advance

This repository contains a modified version of `hgs_advance`, a Rust
implementation of an "advanced" and scalable Hybrid Genetic Search (HGS) for the
Vehicle Routing Problem with Time Windows (VRPTW) and the Capacitated Vehicle
Routing Problem (CVRP), originally developed by Thibaut Vidal for
[The Innovation Game (TIG)](https://tig.foundation/)
([upstream repository](https://github.com/tig-foundation/hgs-advance)).

Compared to the original, this version:

- runs from the command line on classical instance files (Solomon,
  Gehring-Homberger, [CVRPLib](http://vrp.galgos.inf.puc-rio.br/)) and writes the
  best solution found as a `.sol` file;
- adds a **hierarchical objective for VRPTW**: minimize the number of routes
  first, then the total distance (see
  [Hierarchical objective](#hierarchical-objective-for-vrptw-vehicles-first-then-distance)).

This is a research tool, not a polished product. The instructions below assume
no prior experience with Rust.

*Released under the TIG Open Data License v3.0. See
[License and attribution](#license-and-attribution).*

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
Cost 191
NB_ROUTES: 3
CPU_TIME: 0.004207
```

(Format example; the numbers are illustrative.) Each `Route #k` line lists the
customers visited, in order (the depot is implicit at the start/end of each
route). `Cost` is the total distance under the selected distance convention.
With the default `--distance round`, every arc is `nint(Euclidean distance)`, so
costs are integers and **differ from the values in the literature** that use
real-valued or x10-truncated distances (e.g. Solomon C101 gives 829 here, versus
827.3 with real distances). Compare results only against references computed
with the same convention.

### Simple call

```bash
./target/release/hgs_advance <instance> [time_limit_seconds]
./target/release/hgs_advance instance.txt 1800          # your own instance path
./target/release/hgs_advance instance.txt 60 --quiet    # only print the final line
```

This is equivalent to VRPTW, `--mode k-search` (routes first, then distance),
exploration level 5 and `max_it_noimprov=5000`, writing `<instance-name>.sol`
to the current directory. Instance files are not included in this repository.

## Hierarchical objective for VRPTW: vehicles first, then distance

By default (`--mode distance`) hgs_advance minimizes total distance only. The
following modes minimize **(1) the number of routes, then (2) the total
distance** (ties in (1) broken by (2)). They use `--time-limit` (default 60 s)
and, unless `--exploration-level` is given, exploration level 5 with GA restarts
until the time limit.

| Mode | What it does |
|---|---|
| `--mode k-search` | **Option 1.** HGS runs with a *fixed fleet of k vehicles* and minimizes distance only. Outside, k is chosen by binary search on [capacity lower bound, routes of the first feasible solution]. A probe that finds no feasible solution within `--probe-time` seconds (default: 10% of the time limit) is assumed infeasible, so only larger k are tried (a heuristic: it is not a proof of infeasibility, so the minimum number of routes found is an upper bound); a probe that succeeds with r routes sets the upper bound to r. The remaining time minimizes distance with the smallest k found. |
| `--mode lexico` | **Option 2.** One search on `M * #routes + distance`, where M exceeds any possible total distance (exact lexicographic order). M is added to the cost of every depot->client arc, travel times are kept separately, so all move evaluations stay exact. |
| `--mode fixed-k --k K` | Minimize distance with a fleet of K vehicles (one fixed-fleet run). If K is below the capacity lower bound (total demand / capacity, rounded up), it is raised to that bound; a K that is too low for the time windows may end with no feasible solution. |

Distances: `--distance round` (default) uses `nint(euclidean)` with no scaling,
for both cost and travel time; `--distance trunc10` restores the legacy
x10-and-truncate convention.

```bash
./target/release/hgs_advance --instance instance.txt --mode k-search --time-limit 60 --output-dir results
./target/release/hgs_advance --instance instance.txt --mode lexico   --time-limit 60 --output-dir results
```

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

## Use of AI assistance

The modifications in this repository (hierarchical VRPTW objective with the
`k-search`, `lexico` and `fixed-k` modes, `nint` distances, the simple
command-line call and the documentation) were developed with the assistance of
an AI model (Claude, by Anthropic) under the supervision of the authors. The
core hgs_advance algorithm is the original work of Thibaut Vidal (see below).
The modifications were checked by running them on instances with known
optimal solutions and by verifying solutions with an
independent feasibility checker; users should still validate results for
their own use.

## License and attribution

This repository is a modified version of `hgs_advance` by Thibaut Vidal
(© 2026), submitted to The Innovation Game (TIG) `vehicle_routing` challenge
(Unique Algorithm Identifier `c002_a110`). The upstream code is at
<https://github.com/tig-foundation/hgs-advance>; it builds on HGS-CVRP and on
TIG's `hgs_v1`.

Modifications in this repository: a standalone command-line program for
classical VRPTW/CVRP instance files (wall-clock time limit, output directory,
incremental checkpointing), a hierarchical VRPTW objective (number of routes,
then distance) with the `k-search`, `lexico` and `fixed-k` modes,
`nint` distances without scaling, and a simple command-line call.

These modifications are published under the same license as the original code:
the **TIG Open Data License v3.0** (see [`LICENSE.md`](LICENSE.md) and
[`open_data_license.pdf`](open_data_license.pdf)), including its share-alike
and open data terms. TIG also offers a **TIG Commercial License**
([`commercial_license.pdf`](commercial_license.pdf)) for users who do not wish
to comply with those terms. The copyright and license notices of the original
code are kept.
