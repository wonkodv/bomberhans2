
build:
    cargo build

server: build
    cargo run --bin bomberhans2-server

client: build
    cargo run --bin bomberhans2
