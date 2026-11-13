#!/bin/sh
# Cargo decides freshness by file mtime, not content. /src/target is a cache mount shared by every
# build, so another build (other commit, same cache) can leave artifacts newer than this checkout's
# sources, and cargo then reuses a stale binary: a test job can pass without running the new tests.
# Touch every source file so it is newer than anything in the cache. Run it before each cargo
# command that uses the /src/target mount.
set -e
find /src -path /src/target -prune -o -type f -exec touch -c {} +
