# install miden-node
cargo install miden-node

mkdir node-data

cd node-data

# Write the default genesis configuration to a file.
miden-node store dump-genesis > genesis.toml

# Create a folder to store the node's data.
mkdir data 

# Create a folder to store the genesis block's account secrets and data.
mkdir accounts

# Bootstrap the node.
miden-node bundled bootstrap \
  --data-directory data \
  --accounts-directory accounts \
  --config genesis.toml