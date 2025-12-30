build:
    cargo build

server: build
    cargo run --bin bomberhans2-server

client: build
    cargo run --bin bomberhans2

both: build
    kitty bash -c "RUST_LOG=info,bomberhans2=trace,bomberhans2_lib=trace,bomberhans2_server=trace target/debug/bomberhans2-server 2>&1 | tee .server-log" &
    RUST_LOG=info,bomberhans2=trace,bomberhans2_lib=trace,bomberhans2_server=trace target/debug/bomberhans2 2>&1 > tee .client-log &
    RUST_LOG=info,bomberhans2=trace,bomberhans2_lib=trace,bomberhans2_server=trace target/debug/bomberhans2 2>&1 > tee .client2-log &
