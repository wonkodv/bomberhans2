
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
    kitty bash -c "target/debug/bomberhans2-server | tee .server-log" &
    kitty bash -c "target/debug/bomberhans2        | tee .client-log" &
    target/debug/bomberhans2
