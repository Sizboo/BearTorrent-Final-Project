FROM rust:latest

WORKDIR /server_image
COPY . .

RUN cargo build --release

EXPOSE 8080

CMD ["./target/release/server"]