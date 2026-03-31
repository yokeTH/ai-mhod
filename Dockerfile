FROM --platform=$BUILDPLATFORM rust:1-slim-bookworm AS builder

ARG TARGETARCH

RUN apt-get update && apt-get install -y --no-install-recommends \
    musl-tools musl-dev clang llvm lld wget ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install musl cross toolchain
RUN case "$TARGETARCH" in \
    arm64) MUSL_ARCH=aarch64 ;; \
    *)     MUSL_ARCH=x86_64 ;; \
    esac && \
    wget -qO- "https://github.com/musl-cc/musl.cc/releases/latest/download/${MUSL_ARCH}-linux-musl-cross.tgz" \
    | tar xz -C /opt

WORKDIR /app

COPY rust-toolchain.toml ./

RUN case "$TARGETARCH" in \
    amd64) echo "x86_64-unknown-linux-musl" ;; \
    arm64) echo "aarch64-unknown-linux-musl" ;; \
    esac > /tmp/rust-target \
    && rustup target add "$(cat /tmp/rust-target)"

# Copy workspace manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY model/Cargo.toml model/Cargo.toml
COPY repository/Cargo.toml repository/Cargo.toml
COPY server/Cargo.toml server/Cargo.toml
COPY dynamodb/Cargo.toml dynamodb/Cargo.toml

# Copy source
COPY src src
COPY model/src model/src
COPY repository/src repository/src
COPY server/src server/src
COPY dynamodb/src dynamodb/src

ENV RUSTFLAGS="-C target-feature=+crt-static"

RUN case "$TARGETARCH" in \
    arm64) \
    export CARGO_TARGET=aarch64-unknown-linux-musl && \
    export PATH="/opt/aarch64-linux-musl-cross/bin:$PATH" && \
    export CC_aarch64_unknown_linux_musl=aarch64-linux-musl-gcc && \
    export CXX_aarch64_unknown_linux_musl=aarch64-linux-musl-g++ && \
    export AR_aarch64_unknown_linux_musl=aarch64-linux-musl-ar && \
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc ;; \
    *) \
    export CARGO_TARGET=x86_64-unknown-linux-musl && \
    export PATH="/opt/x86_64-linux-musl-cross/bin:$PATH" && \
    export CC_x86_64_unknown_linux_musl=x86_64-linux-musl-gcc && \
    export CXX_x86_64_unknown_linux_musl=x86_64-linux-musl-g++ && \
    export AR_x86_64_unknown_linux_musl=x86_64-linux-musl-ar && \
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc ;; \
    esac && \
    cargo build --release --target "$CARGO_TARGET" && \
    cp "/app/target/$CARGO_TARGET/release/mhod" /mhod

FROM scratch

COPY --from=builder /mhod /mhod
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

EXPOSE 8080

ENTRYPOINT ["/mhod", "serve"]
