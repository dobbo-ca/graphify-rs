#!/bin/sh
# Times the same workloads in Go (`oracle -bench`) and Rust (`bench`, crates/gostd/examples/bench.rs)
# and prints a markdown table. Both print "<workload> <total ns>" lines, or "<workload> skipped".
# Not a gate: the exit status is 0 even where Rust is slower than 1.10x Go.
# usage: sh parity/bench.sh [inputs] [corpus]   (defaults: the bench stage of ci/Dockerfile)
set -e
in=${1:-/inputs}
corpus=${2:-/corpus.txt}
t=$(mktemp -d)
oracle -bench "$in" "$corpus" >"$t/go"
bench "$in" "$corpus" >"$t/rs"
echo '| workload | go | rust | rust/go | target 1.10x |'
echo '|---|---|---|---|---|'
awk 'NR == FNR { go[$1] = $2; order[++n] = $1; next } { rs[$1] = $2 }
END {
  for (i = 1; i <= n; i++) {
    w = order[i]; g = go[w]; r = rs[w]
    if (g ~ /^[0-9]+$/ && r ~ /^[0-9]+$/ && g > 0) {
      x = r / g
      printf "| %s | %.1f ms | %.1f ms | %.2f | %s |\n", w, g / 1e6, r / 1e6, x, (x <= 1.10) ? "ok" : "slow"
    } else {
      printf "| %s | %s | %s | - | - |\n", w, g, r
    }
  }
}' "$t/go" "$t/rs"
rm -rf "$t"
