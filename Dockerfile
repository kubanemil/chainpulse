# syntax=docker/dockerfile:1
FROM rust:1.99-slim-trixie AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p chainpulse \
    && cp target/release/chainpulse /usr/local/bin/chainpulse

FROM gcr.io/distroless/cc-debian13:nonroot
COPY --from=build /usr/local/bin/chainpulse /usr/local/bin/chainpulse
COPY chainpulse.toml /etc/chainpulse/chainpulse.toml
ENV CHAINPULSE_CONFIG=/etc/chainpulse/chainpulse.toml \
    LOG_FORMAT=json
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/chainpulse"]
