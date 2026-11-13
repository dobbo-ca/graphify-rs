# Test data sources and licences

| Path | Source | Licence |
|---|---|---|
| `sigv4/v4/` | aws-sigv4 1.6.5 `aws-signing-test-suite/v4` (38 cases from aws-c-auth v0.9.0, plus `double-encode-path`, `double-url-encode`); `context.json`, `request.txt` and `header-*.txt` only | Apache-2.0, `sigv4/LICENSE`; upstream notes in `sigv4/README.md` |
| `ini/valid/` | aws-sdk-go-v2 `internal/ini` v1.8.0 `testdata/valid` | Apache-2.0, `ini/LICENSE.txt` |
| `config/` | aws-sdk-go-v2 `config` v1.27.27 `testdata` (selected files) | Apache-2.0, `config/LICENSE.txt` |
| `processcreds/` | aws-sdk-go-v2 `credentials` v1.17.27 `processcreds/testdata` | Apache-2.0, `processcreds/LICENSE.txt` |
| `tls/` | rustls-graviola 0.4.0 `tests/keys/ecdsa-p256` (test-only key, certificate for `localhost`) | Apache-2.0 OR ISC OR MIT-0, `tls/README.md` |
| `request_body.json` | derived from anthropic-sdk-go v1.51.0 and graphify-go bedrock.go (plan Task 3) | MIT (this repo) |
| `LICENSES-x-net.txt` | golang.org/x/net v0.55.0 `LICENSE`, for the httpproxy test tables ported into `src/proxy/tests.rs` | BSD-3-Clause |

Ported test code keeps a `// Port of <file>:<line> <name>` comment naming its source and version.
The fixtures contain no real credentials: AWS documentation example keys and obvious fakes only.
