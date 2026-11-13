# Test data

Files copied verbatim from upstream for the ported tests. Each directory keeps the upstream
licence beside the files.

| File | Source | Licence |
|---|---|---|
| `strconv/atof1k.txt`, `strconv/README` | Go 1.27.4 `src/internal/strconv/testdata/` | BSD-3-Clause, `strconv/LICENSE` (Go's `LICENSE`) |

Larger inputs are not committed: `re2-exhaustive.txt` and the toml-test suite come from the
`testinputs` stage (`GOSTD_GOTEST`), and the UCD files from the `ucd` stage (`GOSTD_UCD`). See
parity/README.md.
