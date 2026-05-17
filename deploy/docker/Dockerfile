# Multi-stage build for the actantdb-server binary.
# Phase-6 cloud/team deployment.

FROM rust:1.88-slim-bookworm AS builder
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the workspace.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY migrations ./migrations
COPY specs ./specs
COPY rustfmt.toml clippy.toml ./

RUN cargo build --release -p actant-server --bin actantdb-server

# Runtime image — distroless to keep attack surface small.
FROM gcr.io/distroless/cc-debian12 AS runtime
COPY --from=builder /src/target/release/actantdb-server /usr/local/bin/actantdb-server
COPY --from=builder /src/migrations /opt/actantdb/migrations

ENV ACTANTDB_BIND=0.0.0.0:4555
EXPOSE 4555
ENTRYPOINT ["/usr/local/bin/actantdb-server"]
CMD []
