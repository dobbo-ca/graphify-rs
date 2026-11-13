#!/bin/sh
# Compares a Go oracle tree with a Rust one. On a difference it prints the first 50 diff lines,
# the line count, and the number of differing files per oracle package, then fails.
diff -r "$1" "$2" >/tmp/diff.txt && exit 0
head -50 /tmp/diff.txt
wc -l /tmp/diff.txt
echo "differing files per package:"
grep -E '^(diff -r|Only in) ' /tmp/diff.txt \
    | sed -E 's#^diff -r [^ ]*/([^/ ]+)/[^ ]* .*#\1#; s#^Only in /[^/]+/([^/:]+).*#\1#; s#^Only in /[^/:]+: (.*)#\1 (whole package)#' \
    | sort | uniq -c
exit 1
