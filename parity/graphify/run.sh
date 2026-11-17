#!/bin/sh
# Runs commands.txt against /usr/local/bin/graphify and writes the transcripts (plan "Parity gate").
# usage: SIDE=go|rs sh run.sh <out> [allow-file]
# With an allow-file only the lines whose subcommand (first argument) is listed in it run (an empty
# file runs nothing); without one, every line runs. Runs only in CI (docker build stages).
# Line format: <n> <corpus> <args...> [< <stdin file under parity/graphify>]
#   corpus = a name under /corpus, or an absolute directory (/work/<kind> is made from the built
#   graph.json files on first use, /hookrepo is a fresh git repo). Comments: whole lines starting #.
out=$1
allow=${2:-}
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$out/t" /home/gate /work
git init -q /hookrepo
: > "$out/times.raw"

sel() {
  [ -z "$allow" ] && return 0
  grep -v '^[[:space:]]*#' "$allow" | grep -qx -- "$1"
}

# The damaged and cross-corpus inputs, made from the built graph.json of ggo / ggo-testdata.
prep() {
  d=$1
  [ -d "$d" ] && return 0
  g=/corpus/ggo/graphify-out/graph.json
  mkdir -p "$d/graphify-out"
  case $d in
    /work/bom) { printf '\357\273\277'; cat "$g"; } > "$d/graphify-out/graph.json" ;;
    /work/truncated) head -c 1000 "$g" > "$d/graphify-out/graph.json" ;;
    # second node gets the first node's id (nodes are the only 6-space-indented "id" keys)
    /work/dupid) awk '/^      "id": /{n++; if(n==1)first=$0; else if(n==2)$0=first} {print}' "$g" > "$d/graphify-out/graph.json" ;;
    /work/diff)
      cp "$g" "$d/graphify-out/a.json"
      cp /corpus/ggo-testdata/graphify-out/graph.json "$d/graphify-out/b.json" ;;
    /work/merge)
      cp "$g" "$d/graphify-out/base.json"
      cp /corpus/ggo-testdata/graphify-out/graph.json "$d/graphify-out/cur.json"
      cp "$g" "$d/graphify-out/other.json" ;;
  esac
}

grep -v '^[[:space:]]*#' "$here/commands.txt" | grep -v '^[[:space:]]*$' > /tmp/commands.sel
while IFS= read -r line; do
  n=${line%% *}; rest=${line#* }
  corpus=${rest%% *}; rest=${rest#* }
  stdin=/dev/null
  case $rest in *' < '*) stdin=$here/${rest##* < }; rest=${rest%% < *} ;; esac
  sel "${rest%% *}" || continue
  case $corpus in
    /work/*) prep "$corpus"; dir=$corpus ;;
    /*) dir=$corpus ;;
    *) dir=/corpus/$corpus ;;
  esac
  t0=$(date +%s)
  ( cd "$dir" || exit 127
    eval "set -- $rest"
    exec env -i PATH="$PATH" HOME=/home/gate TZ=UTC LC_ALL=C /usr/local/bin/graphify "$@"
  ) < "$stdin" > "$out/t/$n.stdout" 2> "$out/t/$n.stderr"
  echo $? > "$out/t/$n.exit"
  echo "$corpus $(( $(date +%s) - t0 ))" >> "$out/times.raw"
done < /tmp/commands.sel

for d in /corpus/*/; do
  name=$(basename "$d")
  if [ -d "$d/graphify-out" ]; then
    mkdir -p "$out/corpus/$name"
    cp -R "$d/graphify-out/." "$out/corpus/$name/"
  fi
done
[ -d /work ] && cp -R /work "$out/work"

mkdir -p "$out/hook"
for f in post-commit post-merge post-checkout; do
  [ -f "/hookrepo/.git/hooks/$f" ] && cp "/hookrepo/.git/hooks/$f" "$out/hook/$f"
done
[ -f /hookrepo/.gitattributes ] && cp /hookrepo/.gitattributes "$out/hook/gitattributes"
git -C /hookrepo config --get-regexp '^merge\.graphify' > "$out/hook/gitconfig.txt"
cp /corpus/MANIFEST "$out/MANIFEST"

echo "== wall time, side ${SIDE:-?}, seconds per corpus =="
awk '{s[$1]+=$2; t+=$2} END{for (c in s) print c, s[c]; print "total", t}' "$out/times.raw" | sort | tee "$out/times.txt"
rm "$out/times.raw"
echo "$(find "$out" -type f | wc -l) files in $out"
exit 0
