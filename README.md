# MASM Project Template

A minimal example for compiling, deploying, and testing MASM contracts & notes.

### Run the miden-node locally:
1) Install & setup miden-node:
```bash
./scripts/setup_node.sh
```

2) Run the node locally: 
```bash
./scripts/start_node.sh
```

### Running the program on testnet:
```bash
cargo run --release
```

### Running the tests:
*Before running, ensure you have the miden-node running locally in a separate terminal window:*
```bash
cargo test --release -- --nocapture --test-threads=1
```