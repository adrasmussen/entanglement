# multi-stage build for entanglement: native server/tools binaries + the Dioxus
# WASM webapp, combined into one runtime image used for local docker-compose dev.
#
# this is new tooling for local development only -- it does not modify anything
# under api/, common/, server/, tools/, or webapp/.

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

RUN cargo build --release -p server -p tools

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
RUN dx build --platform web --release

# ---- stage 3: runtime ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    ffmpegthumbnailer \
    libkrb5-3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /build/target/release/server /usr/local/bin/server
COPY --from=rust-builder /build/target/release/entg-db /usr/local/bin/entg-db
COPY --from=rust-builder /build/target/release/entg-auth /usr/local/bin/entg-auth
COPY --from=rust-builder /build/target/release/entg-gss /usr/local/bin/entg-gss
COPY --from=rust-builder /build/target/release/entg-http /usr/local/bin/entg-http

# dx build (even pinned to 0.6.3) writes release output under target/dx/..., not
# the dist/ folder Dioxus.toml's out_dir names -- that setting isn't honored for
# release web builds in this CLI version, so we copy from where it actually lands.
COPY --from=wasm-builder /build/target/dx/webapp/release/web/public /srv/entanglement/webapp-dist

COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh", "server"]
CMD ["--config", "/etc/entanglement/config.toml"]
