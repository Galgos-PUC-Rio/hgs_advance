#!/bin/bash
# Calibration runs for the hierarchical modes. Usage (from the hgs_advance folder):
#   tools/tune.sh <instance> [time_limit=60] [seeds="1 2 3"] [configs file=tools/tune_configs.txt]
# Each config line: <name> <args...>   (lines starting with # are ignored)
# Runs JOBS runs in parallel (default: number of performance cores), then prints a summary.
INST=$1; TL=${2:-60}; SEEDS=${3:-"1 2 3"}; CFG=${4:-tools/tune_configs.txt}
[ -z "$INST" ] && { echo "usage: tools/tune.sh <instance> [time] [seeds] [configs]"; exit 1; }
BIN=./target/release/hgs_advance
JOBS=${JOBS:-$(sysctl -n hw.perflevel0.physicalcpu 2>/dev/null || sysctl -n hw.physicalcpu 2>/dev/null || nproc)}
STEM=$(basename "$INST" .txt)
OUT=tune_runs/$STEM; mkdir -p "$OUT"
JOBFILE=$OUT/jobs.txt; : > "$JOBFILE"
grep -v '^\s*#' "$CFG" | grep -v '^\s*$' | while read -r name args; do
  for s in $SEEDS; do
    echo "$BIN --instance $INST --time-limit $TL --seed $s --quiet $args --output-dir $OUT/${name}_s$s > $OUT/${name}_s$s.log 2>&1" >> "$JOBFILE"
  done
done
N=$(wc -l < "$JOBFILE" | tr -d ' ')
echo "Running $N runs of ${TL}s, $JOBS in parallel (~$(( (N + JOBS - 1) / JOBS * TL / 60 + 1 )) min)..."
tr '\n' '\0' < "$JOBFILE" | xargs -0 -n 1 -P "$JOBS" sh -c 'eval "$1"' _
python3 tools/tune_summary.py "$INST" "$OUT" "$CFG" "$SEEDS"
