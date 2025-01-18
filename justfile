build:
    cargo build

server: build
    cargo run --bin bomberhans2-server

rkr-server:
    rkr target/debug/bomberhans2-server

rkr-client:
    rkr target/debug/bomberhans2

client: build
    cargo run --bin bomberhans2

both: build
    kitty bash -c "RUST_LOG=trace just server 2>&1 | tee .server-log" &
    # kitty bash -c "RUST_LOG=info just client 2>&1 | tee .client-log" &
    RUST_LOG=trace target/debug/bomberhans2
