FROM rust:latest AS builder
RUN apt-get update && apt-get install -y \ 
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# build dependencies (including mediasoup-sys)
WORKDIR /usr/src/vulcan-relay
RUN USER=root cargo init
COPY Cargo.toml Cargo.lock ./
COPY third-party ./third-party
RUN cargo build --release

# build vulcan-relay
COPY src ./src
RUN cargo install --path .

# create deploy image
FROM debian 
COPY --from=builder /usr/local/cargo/bin/vulcan-relay .
USER 1000
ENTRYPOINT ["./vulcan-relay"]