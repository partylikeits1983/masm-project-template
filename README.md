# MASM Project Template

A minimal example for compiling, deploying, and testing MASM contracts & notes.

### Running the program on testnet:
Deploying the counter and incrementing:
```bash
cargo run --release
```

Incrementing an existing counter contract:
```bash
cargo run --release --bin increment
```

### Running the tests:
*Before running, ensure you have the miden-node running locally in a separate terminal window:*
```bash
cargo test --release -- --nocapture --test-threads=1
```

### Run the miden-node locally:
1) Install & setup miden-node:
```bash
./scripts/setup_node.sh
```

2) Run the node locally: 
```bash
./scripts/start_node.sh
```

### Miden WebClient Frontend Repo

https://github.com/partylikeits1983/miden-counter-contract
