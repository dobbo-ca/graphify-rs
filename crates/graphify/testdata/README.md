# graphify test data

Copied unchanged from graphify-go v0.12.0 (`35fcd9e79624716b9d9372070055bad85a8ea779`), MIT
licence (the repository `LICENSE` keeps graphify-go's copyright line).

| Path | Source at 35fcd9e | Used by |
|---|---|---|
| `extract/` (38 files: `bashproj`, `cppproj`, `cproj`, `csharpproj`, `javaproj`, `juliaproj`, `ktproj`, `luaproj`, `mdproj`, `phpproj`, `proj`, `pyproj`, `rsproj`, `rubyproj`, `scalaproj`, `tf`, `verilogproj`, `zigproj`) | `internal/extract/testdata/` | the extract tests (plan Tasks 15 to 41) |
| `cluster/louvain_gonum.golden` (29,680 bytes) | `internal/cluster/testdata/louvain_gonum.golden` (Louvain partitions made with gonum v0.15.1 `community.Modularize`) | `cluster` `TestLouvainMatchesGonumGolden` (plan Task 8) |

Upstream graphify v0.9.107 fixtures go to `upstream/` with graphify's licence beside them, when
a task ports them.
