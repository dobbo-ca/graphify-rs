# Divergences from graphify-go v0.12.0 (35fcd9e)

Every place where graphify-rs output differs from graphify-go on purpose. The parity gate allows
a difference only if it is listed here; from phase 3 each entry also has an allowlist rule in
`parity/graphdiff`.

| ID | Divergence | Reason | Covered by |
|---|---|---|---|
| D-001 | MCP config env keys are emitted in sorted order (Go: mcpconfig.go:113 ranges a map) | Go map order is random, so Go's own output is not reproducible | unit tests; kept out of the byte corpus |
| D-002 | Poetry dependencies are emitted in sorted order (Go: manifest.go:279 ranges a map) | as D-001 | unit tests; kept out of the byte corpus |
| D-003 | `watch` on SIGINT or SIGTERM ends with the default signal exit, not Go's clean exit (maintenance.go:336) | a signal handler needs FFI to libc | CLI test |
| D-004 | `ask` scores on arm64 can differ from Go arm64 in the last bit | Go fuses multiply-add on arm64 (ask.go); Rust never fuses; goldens are amd64 | none needed |
| D-005 | `GFY_CPUPROFILE` is ignored (Go: prof.go:13) | it writes a Go pprof profile | CLI test |
| D-006 | (a) SSO, `role_arn`/`source_profile`, `credential_source`, `web_identity_token_file`, `AWS_WEB_IDENTITY_TOKEN_FILE`, ECS and IMDS credentials come from `aws configure export-credentials`, not an in-process client. (b) Go loads the shared config before the chain and fails on a broken `AWS_PROFILE` profile even when env keys are set (config.go:197-219); graphify-rs reads the profile only when env keys are absent. When `aws` is not on `PATH` and no other source gives credentials, the error says so and names what to set | graphify-rs has no native SSO, STS, IMDS or ECS client; the AWS CLI provides them | Covered by: src/creds/tests.rs |
| D-007 | A proxy URL with scheme `https://` or `socks5://` is an error ("unsupported proxy scheme <s>"); Go's http.Transport supports both | not needed for the Bedrock call; `http://` and bare `host:port` proxies work | `src/proxy/tests.rs` `proxy_scheme_socks5_rejected` |
