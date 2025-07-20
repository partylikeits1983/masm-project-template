// src/main.rs  — cargo run
use std::{fs, path::Path};

use miden_client_tools::{
    create_library, create_tx_script, delete_keystore_and_store, instantiate_client,
};

use miden_client::{
    Word, account::AccountId, rpc::Endpoint, transaction::TransactionRequestBuilder,
};
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    delete_keystore_and_store(None).await;

    // -------------------------------------------------------------------------
    // Instantiate client
    // -------------------------------------------------------------------------
    let endpoint = Endpoint::testnet();
    let mut client = instantiate_client(endpoint, None).await.unwrap();

    let sync_summary = client.sync_state().await.unwrap();
    println!("⛓  Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1 – Query Counter State
    // -------------------------------------------------------------------------
    let (_network_id, counter_contract_id) =
        AccountId::from_bech32("mtst1qr845cd3fadh5qrc96rvwqepsg8fjyts").unwrap();

    client
        .import_account_by_id(counter_contract_id)
        .await
        .unwrap();

    let account_state = client
        .get_account(counter_contract_id)
        .await?
        .expect("counter contract not found");

    let word: Word = account_state.account().storage().get_item(0)?.into();
    let counter_val = word.get(3).unwrap().as_int();
    println!("🔢 Counter value before tx: {}", counter_val);

    // -------------------------------------------------------------------------
    // STEP 2 – Compile the increment script
    // -------------------------------------------------------------------------
    let script_code =
        fs::read_to_string(Path::new("./masm/scripts/increment_script.masm")).unwrap();

    let account_code = fs::read_to_string(Path::new("./masm/accounts/counter.masm")).unwrap();
    let library_path = "external_contract::counter_contract";

    let library = create_library(account_code, library_path).unwrap();

    let tx_script = create_tx_script(script_code, Some(library)).unwrap();

    // -------------------------------------------------------------------------
    // STEP 3 – Build & send transaction
    // -------------------------------------------------------------------------
    let tx_increment_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(counter_contract_id, tx_increment_request)
        .await
        .unwrap();

    let _ = client.submit_transaction(tx_result.clone()).await;

    println!("🚀 Increment transaction submitted – waiting for finality …");
    sleep(Duration::from_secs(7)).await;

    // -------------------------------------------------------------------------
    // STEP 4 – Fetch contract state & verify increment
    // -------------------------------------------------------------------------
    client.sync_state().await.unwrap();

    let account_state = client
        .get_account(counter_contract_id)
        .await?
        .expect("counter contract not found");

    let word: Word = account_state.account().storage().get_item(0)?.into();
    let counter_val = word.get(3).unwrap().as_int();
    println!("🔢 Counter value after tx: {}", counter_val);

    println!("✅ Success! The counter was incremented.");

    let tx_id = tx_result.executed_transaction().id();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    Ok(())
}
