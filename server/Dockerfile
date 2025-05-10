FROM rust:latest

RUN apt-get update && apt-get install -y protobuf-compiler cmake pkg-config libssl-dev

WORKDIR /server_image
COPY . .

RUN cargo build --release

EXPOSE 8080

CMD ["./target/release/server"]