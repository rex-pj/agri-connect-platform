# syntax=docker/dockerfile:1.7
# Build context: backend/. Templates, static files, locales and migrations are
# compiled into the binary, so the runtime image needs nothing else.

FROM rust:1.97.1-slim-bookworm AS build
WORKDIR /src
COPY . .
# Cache mounts keep the registry and target dir between local builds; CI
# starts cold either way.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked --bin agri-connect \
 && cp target/release/agri-connect /agri-connect

# distroless: no shell, no package manager, runs as uid 65532. TLS uses rustls
# with bundled roots, so no OpenSSL is needed at runtime.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /agri-connect /app/agri-connect
ENV BIND_ADDR=0.0.0.0:8080 \
    LOG_FORMAT=json
EXPOSE 8080
HEALTHCHECK --interval=15s --timeout=6s --start-period=30s --retries=3 \
    CMD ["/app/agri-connect", "healthcheck", "live"]
ENTRYPOINT ["/app/agri-connect"]
