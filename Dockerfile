# One image with the game server and the browser client it serves.
# Build: docker build -t reis-ymber-maailma .
# Run:   docker run -p 8080:8080 reis-ymber-maailma

FROM rust:1.98.1-trixie AS build
ARG TRUNK_VERSION=0.21.14
RUN rustup target add wasm32-unknown-unknown \
 && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-x86_64-unknown-linux-musl.tar.gz" \
    | tar -xz -C /usr/local/bin trunk
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p server
# Trunk fetches the wasm-bindgen matching Cargo.lock, and wasm-opt.
RUN cd crates/client && trunk build --release

FROM debian:trixie-slim
RUN useradd --system --home /app game
WORKDIR /app
COPY --from=build /src/target/release/server /app/server
COPY --from=build /src/crates/client/dist /app/dist
ENV STATIC_DIR=/app/dist \
    PORT=8080
EXPOSE 8080
USER game
CMD ["/app/server"]
