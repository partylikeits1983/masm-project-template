# install miden-node
cargo install miden-node

mkdir node-data

cd node-data

# resets old node data
rm -r data
rm -r accounts
rm account.mac
rm genesis.toml

# creates directories for node
mkdir data
mkdir accounts

# bootstrap the node
miden-node bundled bootstrap \
  --data-directory data \
  --accounts-directory .
