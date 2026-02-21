use std::{fs, path::Path};

use masm_project_template::common::{
    create_library, create_tx_script, delete_keystore_and_store, instantiate_client, wait_for_tx,
};
use miden_client::{
    Word,
    account::{Account, StorageSlotName},
    rpc::Endpoint,
    transaction::TransactionRequestBuilder,
};
use miden_protocol::account::AccountId;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    delete_keystore_and_store().await;

    // -------------------------------------------------------------------------
    // Instantiate client
    // -------------------------------------------------------------------------
    let endpoint = Endpoint::testnet();
    let mut client = instantiate_client(endpoint).await.unwrap();

    let sync_summary = client.sync_state().await.unwrap();
    println!("⛓  Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1 – Query Counter State
    let counter_contract_id = AccountId::from_bech32("mtst1azxmwd8waj5cuqq24h995zc73snfrp89")
        .map_err(|e| format!("Invalid bech32 address: {}", e))?
        .1;

    client
        .import_account_by_id(counter_contract_id)
        .await
        .unwrap();

    let account_record: Account = client
        .get_account(counter_contract_id)
        .await?
        .unwrap()
        .try_into()
        .unwrap();

    let storage_slot_name = StorageSlotName::new("counter::counter_slot")?;
    let word: Word = account_record
        .storage()
        .get_item(&storage_slot_name)
        .unwrap();

    let counter_val = word.get(3).unwrap().as_int();
    println!("🔢 Counter value before tx: {}", counter_val);

    // -------------------------------------------------------------------------
    // STEP 2 – Compile the increment script
    // -------------------------------------------------------------------------
    let script_code =
        fs::read_to_string(Path::new("./masm/scripts/increment_script.masm")).unwrap();

    let account_code = fs::read_to_string(Path::new("./masm/accounts/counter.masm")).unwrap();
    let library_path = "external_contract::counter_contract";

    let library = create_library(account_code, library_path).unwrap();

    let tx_script = create_tx_script(script_code, Some(library)).unwrap();

    // -------------------------------------------------------------------------
    // STEP 3 – Build & send transaction
    // -------------------------------------------------------------------------
    let tx_increment_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_id = client
        .submit_new_transaction(counter_contract_id, tx_increment_request)
        .await
        .unwrap();

    println!("🚀 Increment transaction submitted – waiting for finality …");
    wait_for_tx(&mut client, tx_id).await?;

    // -------------------------------------------------------------------------
    // STEP 4 – Fetch contract state & verify increment
    // -------------------------------------------------------------------------
    client.sync_state().await.unwrap();

    let account_record: Account = client
        .get_account(counter_contract_id)
        .await?
        .unwrap()
        .try_into()
        .unwrap();

    let storage_slot_name = StorageSlotName::new("counter::counter_slot")?;
    let word: Word = account_record
        .storage()
        .get_item(&storage_slot_name)
        .unwrap();

    let counter_val = word.get(3).unwrap().as_int();
    println!("🔢 Counter value after tx: {}", counter_val);

    println!("✅ Success! The counter was incremented.");
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    Ok(())
}
