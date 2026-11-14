# syntax=docker/dockerfile:1.7
# multi-stage build for entanglement: native server/tools binaries + the Dioxus
# WASM webapp, combined into one runtime image used for local docker-compose dev.
#
# this is new tooling for local development only -- it does not modify anything
# under api/, common/, server/, tools/, or webapp/.
#
# build caching: the cargo registry and each stage's target/ live in BuildKit cache mounts
# (id entg-*), so third-party crates -- about 640 of them, rocksdb's C++ among them -- compile
# once and later builds only recompile the workspace crates.  cargo trusts file mtimes, and a
# branch switch can bring in a source file that is older than the cached artifact, so the
# workspace sources are touched before each build: the workspace always recompiles, the
# dependencies never do (unless Cargo.lock changes).  the target dirs are not part of the image,
# so the binaries are copied out to /out.
#
# `docker builder prune -a` also deletes these cache mounts (the next build is cold again);
# to free disk while keeping them:  docker builder prune -f --filter 'type!=exec.cachemount'

# ---- stage 1: native binaries (server, entg-db, entg-auth, entg-gss, entg-http) ----
FROM rust:bookworm AS rust-builder

# libkrb5-dev is required at build time by libgssapi/ldap3's gssapi feature even
# though the local dev config never exercises GSSAPI/LDAP at runtime.
# clang/cmake are required to build the rocksdb crate (tools' dbtool dump/undump).
RUN apt-get update && apt-get install -y --no-install-recommends \
    libkrb5-dev \
    clang \
    cmake \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY api ./api
COPY common ./common
COPY server ./server
COPY tools ./tools
COPY webapp/Cargo.toml ./webapp/Cargo.toml

# webapp is wasm-only and doesn't build natively; give cargo just enough of a
# crate skeleton to resolve the workspace without pulling it into this stage.
RUN mkdir -p webapp/src && echo "fn main() {}" > webapp/src/main.rs

RUN --mount=type=cache,id=entg-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=entg-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=entg-native-target,target=/build/target,sharing=locked \
    find api common server tools -name '*.rs' -exec touch {} + \
    && cargo build --release -p server -p tools \
    && mkdir -p /out \
    && cp target/release/server target/release/entg-db target/release/entg-auth \
          target/release/entg-gss target/release/entg-http /out/

# ---- stage 2: Dioxus WASM webapp build ----
FROM rust:bookworm AS wasm-builder

# pinned to match the workspace's `dioxus = "0.6.3"` -- an unpinned/newer dioxus-cli
# (0.7.x) uses a different output layout (target/dx/.../web/public instead of the
# dist/ folder Dioxus.toml's out_dir expects here) and will silently build to the
# wrong path.
RUN rustup target add wasm32-unknown-unknown \
    && cargo install dioxus-cli --version 0.6.3 --locked

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY api ./api
COPY common ./common
COPY server ./server
COPY tools ./tools
COPY webapp ./webapp

WORKDIR /build/webapp
RUN --mount=type=cache,id=entg-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=entg-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=entg-wasm-target,target=/build/target,sharing=locked \
    find /build/api /build/webapp -name '*.rs' -exec touch {} + \
    && rm -rf /build/target/dx/webapp/release/web/public \
    && dx build --platform web --release \
    && mkdir -p /out \
    && cp -r /build/target/dx/webapp/release/web/public /out/public

# ---- stage 3: runtime ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    ffmpegthumbnailer \
    libkrb5-3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /out/server /out/entg-db /out/entg-auth /out/entg-gss /out/entg-http /usr/local/bin/

# dx build (even pinned to 0.6.3) writes release output under target/dx/..., not
# the dist/ folder Dioxus.toml's out_dir names -- that setting isn't honored for
# release web builds in this CLI version, so we copy from where it actually lands.
COPY --from=wasm-builder /out/public /srv/entanglement/webapp-dist

COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh", "server"]
CMD ["--config", "/etc/entanglement/config.toml"]
