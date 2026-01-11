use std::{fs, path::Path};

use masm_project_template::common::{
    create_library, create_public_immutable_contract, create_tx_script, delete_keystore_and_store,
    instantiate_client,
};

use miden_client::{
    Word,
    account::{AccountIdAddress, Address, AddressInterface},
    rpc::Endpoint,
    transaction::TransactionRequestBuilder,
};
use miden_objects::account::NetworkId;
use tokio::time::{Duration, sleep};

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
    // STEP 1 – Deploy the counter contract
    // -------------------------------------------------------------------------
    let counter_code = fs::read_to_string(Path::new("./masm/accounts/counter.masm")).unwrap();

    let (counter_contract, counter_seed) =
        create_public_immutable_contract(&mut client, &counter_code).await?;

    client
        .add_account(&counter_contract, Some(counter_seed), false)
        .await
        .unwrap();
    let addr = AccountIdAddress::new(counter_contract.id(), AddressInterface::Unspecified);

    // build address of faucet
    let address = Address::AccountId(addr);
    println!(
        "📄 Counter contract ID: {}",
        address.to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 2 – Compile the increment script
    // -------------------------------------------------------------------------
    let script_code =
        fs::read_to_string(Path::new("./masm/scripts/increment_script.masm")).unwrap();

    let library_path = "external_contract::counter_contract";

    let library = create_library(counter_code.clone(), library_path).unwrap();

    let tx_script = create_tx_script(script_code, Some(library)).unwrap();

    // -------------------------------------------------------------------------
    // STEP 3 – Build & send transaction
    // -------------------------------------------------------------------------
    let tx_increment_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(counter_contract.id(), tx_increment_request)
        .await
        .unwrap();

    let _ = client.submit_transaction(tx_result.clone()).await;

    println!("🚀 Increment transaction submitted – waiting for finality …");
    sleep(Duration::from_secs(7)).await;

    // -------------------------------------------------------------------------
    // STEP 4 – Fetch contract state & verify
    // -------------------------------------------------------------------------

    // Deleting keystore & store to show how to fetch public state
    delete_keystore_and_store().await;

    let endpoint = Endpoint::testnet();
    let mut client = instantiate_client(endpoint).await?;

    client
        .import_account_by_id(counter_contract.id())
        .await
        .unwrap();

    let account_state = client
        .get_account(counter_contract.id())
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
