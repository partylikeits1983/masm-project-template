// src/main.rs  — cargo run

use miden_client_tools::delete_keystore_and_store;

use miden_client::{builder::ClientBuilder, rpc::Endpoint, rpc::TonicRpcClient};
use std::sync::Arc;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    delete_keystore_and_store(None).await;

    // -------------------------------------------------------------------------
    // Instantiate client
    // -------------------------------------------------------------------------
    // Initialize client
    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api)
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    Ok(())
}
