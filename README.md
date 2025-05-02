# MASM Project Template

### Running the program:
```bash
cargo run --release
```

### Running the tests:
1) Install & setup miden-node:
```bash
./setup_node.sh
```

2) Run the node: 
```bash
./start_node.sh
```

3) In a separate terminal, run the tests:
```bash
cargo test --release -- --nocapture
```