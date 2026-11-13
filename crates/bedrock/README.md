# bedrock

Makes graphify-go's one Bedrock (mantle) call, the Anthropic Messages `record_concepts` tool call
(internal/semantic/bedrock.go:103), with the same request body, auth, credential chain, retries and error text.
Its own HTTP/1.1 client over `std::net::TcpStream` and rustls, with no C and no `unsafe`.
Plan: `docs/superpowers/plans/2026-11-13-bedrock.md`.

## Public API

`Config`, `Env`, `Client` (`Client::new`, `Client::record_concepts`), `Error`, `ApiError`, and the constants
`MODEL`, `MAX_TOKENS`, `MAX_NOTE_BYTES`, `TOOL_NAME`.

## Third-party crates

| Crate | Reason |
|---|---|
| `rustls` 0.23 (`std`, `tls12`) | TLS client; no default features, so no `aws-lc-rs`, `ring` or `log` |
| `rustls-graviola` 0.4 | crypto provider for rustls in pure Rust and assembly, no C |
| `webpki-roots` 1 | Mozilla root certificates compiled in (CDLA-Permissive-2.0, a `deny.toml` exception) |

Transitive, from `Cargo.lock`:

| Crate | Reason |
|---|---|
| `graviola` | transitive, pulled by `rustls-graviola`: the crypto implementation |
| `getrandom` | transitive, pulled by `graviola`: OS randomness |
| `cfg-if` | transitive, pulled by `graviola` |
| `libc` | transitive, pulled by `getrandom`: declarations only, no C is built |
| `rustls-pki-types` | transitive, pulled by `rustls` and `webpki-roots`: certificate and name types, PEM parsing |
| `rustls-webpki` | transitive, pulled by `rustls`: certificate path validation |
| `untrusted` | transitive, pulled by `rustls-webpki`: byte-slice parsing |
| `once_cell` | transitive, pulled by `rustls` |
| `subtle` | transitive, pulled by `rustls`: constant-time comparisons |
| `zeroize` | transitive, pulled by `rustls` and `rustls-pki-types`: clears key material |

## Divergences from graphify-go

- D-006: SSO, `role_arn`, `source_profile`, `credential_source`, web identity, ECS and IMDS credentials go through `aws configure export-credentials`; a broken `AWS_PROFILE` profile is read only when env keys are absent.
- D-007: a proxy URL with scheme `https://` or `socks5://` is an error ("unsupported proxy scheme <s>"); `http://` and bare `host:port` work.
