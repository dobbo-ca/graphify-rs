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
RUN --mount=type=cache,target=/root/.cache/go-build go vet ./... && go build -o /out/oracle .

FROM go-oracle AS go-run
RUN /out/oracle -gen /inputs && /out/oracle /inputs /go-out \
    && echo "$(find /inputs -type f | wc -l) inputs, $(find /go-out -type f | wc -l) outputs"

# The checks run in the build, so a red check fails the build; a rerun of the same source is a
# BuildKit cache hit.
FROM src AS ci
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo fmt --all --check
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo clippy --locked --workspace --all-targets -- -D warnings
# aarch64-only code (NEON kernels, unsigned C char) is not compiled for x86_64, so lint it too.
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo clippy --locked --workspace --all-targets --target aarch64-unknown-linux-gnu -- -D warnings
# No C in the build graph: the ban list in deny.toml.
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    cargo deny --locked check
# /gotest holds the large Go test inputs (see the testinputs stage); a test that needs it panics
# if GOSTD_GOTEST is not set.
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    --mount=type=bind,from=testinputs,source=/gotest,target=/gotest \
    GOSTD_GOTEST=/gotest cargo test --locked --workspace

# qemu-user and the aarch64 cross toolchain, for the arm64 stage.
FROM toolchain AS qemu-tools
RUN apt-get update && apt-get install -y --no-install-recommends qemu-user gcc-aarch64-linux-gnu libc6-dev-arm64-cross \
    && rm -rf /var/lib/apt/lists/*
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER="qemu-aarch64 -L /usr/aarch64-linux-gnu"

FROM qemu-tools AS qemu-arm64
COPY . .

# The full test suite for aarch64 under qemu-user.
FROM qemu-arm64 AS arm64
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    --mount=type=bind,from=testinputs,source=/gotest,target=/gotest \
    GOSTD_GOTEST=/gotest cargo test --locked --workspace --target aarch64-unknown-linux-gnu

# gostd against the Go oracle: the two output trees must be equal. A difference is a Rust bug.
# The Go trees are rw bind mounts from go-run (as in deflate-rs: keyed by go-run's cache key, not by
# a hash of their content). PARITY_MIN_OUTPUTS proves the ops ran; each module PR raises it.
FROM src AS parity
ARG PARITY_MIN_OUTPUTS=4
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    --mount=type=bind,from=go-run,source=/inputs,target=/inputs,rw \
    --mount=type=bind,from=go-run,source=/go-out,target=/go-out,rw \
    cargo run --locked --release -p gostd --example oracle -- /inputs /rust-out \
    && n=$(find /go-out -type f | wc -l) && echo "$n Go outputs" && [ "$n" -ge "$PARITY_MIN_OUTPUTS" ] \
    && { diff -r /go-out /rust-out >/tmp/diff.txt || { head -50 /tmp/diff.txt; wc -l /tmp/diff.txt; exit 1; }; } \
    && GOSTD_SIMD=scalar cargo run --locked --release -p gostd --example oracle -- /inputs /rust-out-scalar \
    && { diff -r /go-out /rust-out-scalar >/tmp/diff.txt || { head -50 /tmp/diff.txt; wc -l /tmp/diff.txt; exit 1; }; } \
    && rm -rf /rust-out /rust-out-scalar

# The same on aarch64 under qemu-user (the ARMv8 SHA-256 tier, unsigned char paths).
FROM qemu-arm64 AS parity-arm64
ARG PARITY_MIN_OUTPUTS=4
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    --mount=type=bind,from=go-run,source=/inputs,target=/inputs,rw \
    --mount=type=bind,from=go-run,source=/go-out,target=/go-out,rw \
    cargo run --locked --release -p gostd --target aarch64-unknown-linux-gnu --example oracle -- /inputs /arm-out \
    && n=$(find /go-out -type f | wc -l) && echo "$n Go outputs" && [ "$n" -ge "$PARITY_MIN_OUTPUTS" ] \
    && { diff -r /go-out /arm-out >/tmp/diff.txt || { head -50 /tmp/diff.txt; wc -l /tmp/diff.txt; exit 1; }; } \
    && GOSTD_SIMD=scalar cargo run --locked --release -p gostd --target aarch64-unknown-linux-gnu --example oracle -- /inputs /arm-out-scalar \
    && { diff -r /go-out /arm-out-scalar >/tmp/diff.txt || { head -50 /tmp/diff.txt; wc -l /tmp/diff.txt; exit 1; }; } \
    && rm -rf /arm-out /arm-out-scalar
