# Jam relay (#25). Build context: the repository root (the guest page
# comes from src-tauri/src/jam/page/). See relay/README.md.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY relay/ relay/
COPY src-tauri/src/jam/page/ src-tauri/src/jam/page/
RUN cd relay && cargo build --release --locked

FROM debian:bookworm-slim
RUN useradd --system --no-create-home --shell /usr/sbin/nologin relay
COPY --from=build /src/relay/target/release/seaglass-relay /usr/local/bin/seaglass-relay
USER relay
ENV RELAY_BIND=0.0.0.0:8080 \
    RELAY_TRUST_PROXY=1
EXPOSE 8080
CMD ["seaglass-relay"]
