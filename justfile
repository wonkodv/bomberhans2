
build:
    cargo build

server: build
    cargo run --bin bomberhans2-server
#   while true; do target/debug/bomberhans2-server & pid=$! ; inotifywait -e modify  target/debug/bomberhans2-server; kill $pid; done

client: build
    cargo run --bin bomberhans2
