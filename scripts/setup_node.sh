# install miden-node
cargo install --locked miden-node

# create node-data directory
mkdir node-data
cd node-data

# create data & accounts directories
mkdir data
mkdir accounts

# bootstrap the node
miden-node bundled bootstrap \
  --data-directory data \
  --accounts-directory .