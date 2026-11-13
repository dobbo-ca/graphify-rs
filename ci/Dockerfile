# CI image: Rust runs here, on the in-cluster BuildKit, because the ARC runner has no toolchain.
# Copied from dobbo-ca/deflate-rs ci/Dockerfile. Every tool is installed before `COPY . .`, so a
# new commit does not install it again.
FROM rust:1.99.1-bookworm AS toolchain
RUN rustup component add rustfmt clippy && rustup target add aarch64-unknown-linux-gnu
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-deny --locked --version ~0.18
WORKDIR /src

FROM toolchain AS src
COPY . .

# The UCD files the table generator (crates/gostd-gen) reads, listed in ucd.files and checked
# against ucd.lock (sha256sum format). An empty ucd.lock skips the check (bootstrap only: the `ci`
# stage fails until the computed lock is committed). /ucd/ucd.lock is always the computed one.
FROM debian:bookworm-slim AS ucd
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /ucd
COPY crates/gostd-gen/ucd.files crates/gostd-gen/ucd.lock /ucdcfg/
RUN set -e; while read -r f; do mkdir -p "$(dirname "$f")"; \
      curl -fsSL --retry 3 -o "$f" "https://www.unicode.org/Public/${f%%/*}/ucd/${f#*/}"; \
    done < /ucdcfg/ucd.files \
    && if [ -s /ucdcfg/ucd.lock ]; then sha256sum -c /ucdcfg/ucd.lock; fi \
    && sha256sum $(cat /ucdcfg/ucd.files) > /ucd/ucd.lock && cat /ucd/ucd.lock

# The generated tables (exported by the `gen` job with outputs: type=local).
FROM src AS gen-build
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=private \
    --mount=type=bind,from=ucd,source=/ucd,target=/ucd \
    sh ci/fresh.sh && cargo run --locked --release -p gostd-gen -- /ucd /out && cp /ucd/ucd.lock /out/

FROM scratch AS gen
COPY --from=gen-build /out /

# The Go oracle (parity/go/oracle) and the Go test inputs. Go runs only here, never on a Mac.
# The modules are downloaded by name and checked against go.sum (a copy of graphify-go's), so a
# changed module fails the build.
FROM golang:1.27.4-bookworm AS go-mods
ENV GOTOOLCHAIN=local CGO_ENABLED=0 GOFLAGS=-mod=readonly
RUN go version | grep -q 'go1.27.4 '
WORKDIR /src/parity/go/oracle
COPY parity/go/oracle/go.mod parity/go/oracle/go.sum ./
RUN go mod download github.com/BurntSushi/toml golang.org/x/exp golang.org/x/text && go mod verify

# Test inputs too large to commit: RE2's exhaustive search tests (bz2 in the Go tree) and the
# toml-test suite vendored in BurntSushi/toml v1.4.0. The test stages bind-mount /gotest.
FROM go-mods AS testinputs
RUN apt-get update && apt-get install -y --no-install-recommends bzip2 && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /gotest/regexp \
    && bunzip2 -c "$(go env GOROOT)/src/regexp/testdata/re2-exhaustive.txt.bz2" > /gotest/regexp/re2-exhaustive.txt \
    && cp -r "$(go list -m -f '{{.Dir}}' github.com/BurntSushi/toml)/internal/toml-test/tests" /gotest/toml-test \
    && chmod -R u+w /gotest && du -sh /gotest

FROM go-mods AS go-oracle
COPY parity/go/oracle ./
# The oracle must run with graphify-go's GODEBUG defaults (its go.mod says go 1.25.0): go.mod has
# `godebug default=go1.25`. Print the effective defaults and fail if net/url is not lenient.
RUN --mount=type=cache,target=/root/.cache/go-build go vet ./... && go build -o /out/oracle . \
    && echo "oracle DefaultGODEBUG: $(go list -f '{{.DefaultGODEBUG}}' .)" \
    && go list -f '{{.DefaultGODEBUG}}' . | tr ',' '\n' | grep -qx 'urlstrictcolons=0'

FROM go-oracle AS go-run
RUN /out/oracle -gen /inputs && /out/oracle /inputs /go-out \
    && echo "$(find /inputs -type f | wc -l) inputs, $(find /go-out -type f | wc -l) outputs"

# The checks run in the build, so a red check fails the build; a rerun of the same source is a
# BuildKit cache hit.
FROM src AS ci
# Build (go vet + go build) the Go oracle on every PR, so an oracle compile error shows on the PR.
# Running it against Rust (`parity`) stays a main-only stage; BuildKit builds this in parallel.
COPY --from=go-oracle /out/oracle /tmp/go-oracle-built
# Informational: prints "sha_ni" if the CI host has SHA-NI, so the log shows whether the x86 SHA kernel can run here.
RUN grep -o -m1 -w sha_ni /proc/cpuinfo || true
RUN cargo fmt --all --check
# The committed UCD lock and tables must equal what the generator makes from the pinned files.
RUN --mount=type=bind,from=gen-build,source=/out,target=/gen \
    { cmp /gen/ucd.lock crates/gostd-gen/ucd.lock \
      && diff -r --exclude=mod.rs --exclude=ucd.lock /gen crates/gostd/src/tables; } \
    || { echo 'crates/gostd/src/tables or ucd.lock differ from the gostd-gen output: commit the gen job artifact'; exit 1; }
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=private \
    sh ci/fresh.sh && cargo clippy --locked --workspace --all-targets -- -D warnings
# aarch64-only code (NEON kernels, unsigned C char) is not compiled for x86_64, so lint it too.
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=private \
    sh ci/fresh.sh && cargo clippy --locked --workspace --all-targets --target aarch64-unknown-linux-gnu -- -D warnings
# No C in the build graph: the ban list in deny.toml.
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    cargo deny --locked check
# /gotest holds the large Go test inputs (see the testinputs stage) and /ucd the pinned UCD files
# (NormalizationTest.txt); a test that needs one panics if GOSTD_GOTEST or GOSTD_UCD is not set.
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=private \
    --mount=type=bind,from=testinputs,source=/gotest,target=/gotest \
    --mount=type=bind,from=ucd,source=/ucd,target=/ucd \
    sh ci/fresh.sh && GOSTD_GOTEST=/gotest GOSTD_UCD=/ucd cargo test --locked --workspace
# The slow regexp exhaustive test (ignored by default) runs in release mode.
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=private \
    --mount=type=bind,from=testinputs,source=/gotest,target=/gotest \
    sh ci/fresh.sh && GOSTD_GOTEST=/gotest cargo test --locked --release -p gostd --test regexp_exhaustive -- --ignored
# The scalar SHA-256 tier on its own (GOSTD_SIMD=scalar), after the default run above.
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=private \
    --mount=type=bind,from=testinputs,source=/gotest,target=/gotest \
    --mount=type=bind,from=ucd,source=/ucd,target=/ucd \
    sh ci/fresh.sh && { grep -o -m1 -w sha_ni /proc/cpuinfo || echo "cpu: no sha_ni"; } \
    && GOSTD_SIMD=scalar GOSTD_GOTEST=/gotest GOSTD_UCD=/ucd cargo test --locked -p gostd

# qemu-user and the aarch64 cross toolchain, for the arm64 stage.
FROM toolchain AS qemu-tools
RUN apt-get update && apt-get install -y --no-install-recommends qemu-user gcc-aarch64-linux-gnu libc6-dev-arm64-cross \
    && rm -rf /var/lib/apt/lists/*
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER="qemu-aarch64 -L /usr/aarch64-linux-gnu" \
    GRAPHIFY_TEST_RUNNER="qemu-aarch64 -L /usr/aarch64-linux-gnu"

FROM qemu-tools AS qemu-arm64
COPY . .

# The full test suite for aarch64 under qemu-user.
FROM qemu-arm64 AS arm64
# bedrock's tests start child processes (credential_process, the aws CLI). Under qemu-user a fork
# while other test threads run can hang the child before its exec, so bedrock runs on one test
# thread here, with its safety deadlines scaled 10x (BEDROCK_TEST_TIME_SCALE).
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=private \
    --mount=type=bind,from=testinputs,source=/gotest,target=/gotest \
    --mount=type=bind,from=ucd,source=/ucd,target=/ucd \
    sh ci/fresh.sh && GOSTD_GOTEST=/gotest GOSTD_UCD=/ucd cargo test --locked --workspace --exclude bedrock --target aarch64-unknown-linux-gnu \
    && BEDROCK_TEST_TIME_SCALE=10 cargo test --locked -p bedrock --target aarch64-unknown-linux-gnu -- --test-threads=1
# The scalar SHA-256 tier on aarch64 (the default run above uses the ARMv8 tier).
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=private \
    --mount=type=bind,from=testinputs,source=/gotest,target=/gotest \
    --mount=type=bind,from=ucd,source=/ucd,target=/ucd \
    sh ci/fresh.sh && GOSTD_SIMD=scalar GOSTD_GOTEST=/gotest GOSTD_UCD=/ucd cargo test --locked -p gostd --target aarch64-unknown-linux-gnu

# gostd against the Go oracle: the two output trees must be equal. A difference is a Rust bug.
# The Go trees are rw bind mounts from go-run (as in deflate-rs: keyed by go-run's cache key, not by
# a hash of their content). Each oracle package declares the fewest outputs it may write (Go
# register call, Rust PKGS entry); both oracles fail below it, so the stage needs no shared count.
FROM src AS parity
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=private \
    --mount=type=bind,from=go-run,source=/inputs,target=/inputs,rw \
    --mount=type=bind,from=go-run,source=/go-out,target=/go-out,rw \
    sh ci/fresh.sh && cargo run --locked --release -p gostd --example oracle -- /inputs /rust-out \
    && sh ci/paritydiff.sh /go-out /rust-out \
    && GOSTD_SIMD=scalar cargo run --locked --release -p gostd --example oracle -- /inputs /rust-out-scalar \
    && sh ci/paritydiff.sh /go-out /rust-out-scalar \
    && rm -rf /rust-out /rust-out-scalar

# The same on aarch64 under qemu-user (the ARMv8 SHA-256 tier, unsigned char paths).
FROM qemu-arm64 AS parity-arm64
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=private \
    --mount=type=bind,from=go-run,source=/inputs,target=/inputs,rw \
    --mount=type=bind,from=go-run,source=/go-out,target=/go-out,rw \
    sh ci/fresh.sh && cargo run --locked --release -p gostd --target aarch64-unknown-linux-gnu --example oracle -- /inputs /arm-out \
    && sh ci/paritydiff.sh /go-out /arm-out \
    && GOSTD_SIMD=scalar cargo run --locked --release -p gostd --target aarch64-unknown-linux-gnu --example oracle -- /inputs /arm-out-scalar \
    && sh ci/paritydiff.sh /go-out /arm-out-scalar \
    && rm -rf /arm-out /arm-out-scalar
