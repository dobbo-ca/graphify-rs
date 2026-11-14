#!/bin/sh
# Corpus filter (plan "Parity gate"), run by the graphify-corpus stage of ci/Dockerfile.
# usage: sh filter.sh <dir holding groups.txt and groups.map> <corpus root>
# Runs only in CI (needs jq). LC_ALL=C so file names with invalid UTF-8 match as bytes.
set -eu
LC_ALL=C
export LC_ALL
cfg=$1
root=$2

# 1. committed graphify-out directories
find "$root" -name graphify-out -type d -prune -exec rm -rf {} +

# 2. D-001/D-002: MCP configs whose env object has 2+ keys (Go ranges a map), poetry manifests
for name in .mcp.json claude_desktop_config.json mcp.json mcp_servers.json; do
  find "$root" -type f -name "$name" -exec sh -c '
    for f; do
      if jq -e "[.. | objects | select(has(\"env\")) | .env | select(type == \"object\" and length >= 2)] | length > 0" "$f" >/dev/null 2>&1; then
        echo "D-001: remove $f"; rm -f "$f"
      fi
    done' sh {} +
done
find "$root" -type f -name pyproject.toml -exec sh -c '
  for f; do
    if grep -qF tool.poetry "$f"; then echo "D-002: remove $f"; rm -f "$f"; fi
  done' sh {} +

# 3. group filter
listed() { grep -v '^#' "$cfg/groups.txt" | grep -qx "$1"; }
while read -r ext grp; do
  case $ext in ''|'#'*) continue;; esac
  if ! listed "$grp"; then
    find "$root" -type f -iname "*$ext" -exec rm -f {} +
  fi
done < "$cfg/groups.map"
# Extensionless files that start with a shebang route to python, js, ruby, bash, lua, php or julia
# (detect.ShebangExt); delete them until every one of those groups is listed.
all=1
for g in bash python js ruby php lua julia; do listed "$g" || all=0; done
if [ "$all" = 0 ]; then
  find "$root" -type f ! -name '*.*' -exec sh -c '
    for f; do
      if [ "$(head -c 2 "$f")" = "#!" ]; then rm -f "$f"; fi
    done' sh {} +
fi

# 4. manifest
find "$root" -type f ! -path "$root/MANIFEST" -exec sha256sum {} + | sort -k 2 > "$root/MANIFEST"
cat "$root/MANIFEST"
echo "$(wc -l < "$root/MANIFEST") files in the gate corpus"
