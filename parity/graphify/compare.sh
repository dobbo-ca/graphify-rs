#!/bin/sh
# Go output tree against Rust output tree (the graphify-parity stage). Only what the Rust side has
# is compared, as set by rs-commands.txt (see README.md). No output normalisation.
# usage: sh compare.sh <go-out> <rs-out>
go=$1
rs=$2
list=/parity/graphify/rs-commands.txt
has() { grep -v '^[[:space:]]*#' "$list" | grep -qx -- "$1"; }
fail=0
bad() { echo "DIFF: $1"; fail=1; }
echo "files: go $(find "$go" -type f | wc -l), rs $(find "$rs" -type f | wc -l)"

# transcripts: every one the Rust side wrote
for f in "$rs"/t/*; do
  [ -e "$f" ] || continue
  cmp "$go/t/${f##*/}" "$f" || bad "t/${f##*/}"
done

# graphify-out per corpus, only once `build` is listed; export files only once `export` is
if has build; then
  ex="--exclude=.graphify_cache.json --exclude=.graphify_stat.json --exclude=.graphify_root --exclude=nonutf8"
  has export || ex="$ex --exclude=graph.graphml --exclude=graph.dot --exclude=graph.nodes.csv --exclude=graph.edges.csv --exclude=okf"
  # shellcheck disable=SC2086
  diff -r $ex "$go/corpus" "$rs/corpus" || bad corpus
  # D-008: graph.json only
  cmp "$go/corpus/nonutf8/graph.json" "$rs/corpus/nonutf8/graph.json" || bad corpus/nonutf8/graph.json
fi

# hook files
if has hook; then diff -r "$go/hook" "$rs/hook" || bad hook; fi

# /work copies the Rust side made (the merge-driver result)
for d in "$rs"/work/*; do
  [ -d "$d" ] || continue
  diff -r "$go/work/${d##*/}" "$d" || bad "work/${d##*/}"
done

[ "$fail" = 0 ] && echo "graphify-parity: no differences"
exit "$fail"
