# Build the actantdb-server binary in a multi-stage container.
#
# The companion file at `deploy/docker/Dockerfile` is the older recipe used
# by the local-cluster smoke compose. This Dockerfile is the one referenced
# by `deploy/docker-compose.yml` and is the canonical image for self-host.

FROM rust:1.88-slim-bookworm AS builder
WORKDIR /src
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY migrations ./migrations
COPY specs ./specs
COPY sdks ./sdks
COPY bench ./bench
COPY rustfmt.toml clippy.toml ./

# Build only the server binary; cargo will pull just the crates it needs.
RUN cargo build --release -p actant-server --bin actantdb-server

FROM gcr.io/distroless/cc-debian12 AS runtime
COPY --from=builder /src/target/release/actantdb-server /usr/local/bin/actantdb-server
COPY --from=builder /src/migrations /opt/actantdb/migrations

ENV ACTANTDB_BIND=0.0.0.0:4555
EXPOSE 4555
ENTRYPOINT ["/usr/local/bin/actantdb-server"]
CMD []
