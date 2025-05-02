// src/main.rs  — cargo run
use std::{convert::TryInto, fs, sync::Arc};

use masm_project_template::common::{create_library, delete_keystore_and_store};
use miden_client::{
    Felt,
    account::{
        AccountBuilder, AccountStorageMode, AccountType, StorageSlot, component::AccountComponent,
    },
    builder::ClientBuilder,
    rpc::{Endpoint, TonicRpcClient},
    transaction::{TransactionKernel, TransactionRequestBuilder},
};
use miden_crypto::Word;
use miden_objects::{assembly::Assembler, transaction::TransactionScript};
use rand::RngCore;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    delete_keystore_and_store().await;

    // -------------------------------------------------------------------------
    // Configure client
    // -------------------------------------------------------------------------
    let endpoint = Endpoint::localhost();
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, /*timeout*/ 10_000));

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api.clone())
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    let sync_summary = client.sync_state().await?;
    println!("⛓  Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1 – Deploy the counter contract
    // -------------------------------------------------------------------------
    let account_code = fs::read_to_string("./masm/accounts/counter.masm")
        .expect("could not read ./masm/accounts/counter.masm");

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    // storage slot 0 := [0,0,0,0]  ⇒ counter = 0
    let counter_component = AccountComponent::compile(
        account_code.clone(),
        assembler,
        vec![StorageSlot::Value([
            Felt::new(0),
            Felt::new(0),
            Felt::new(0),
            Felt::new(0),
        ])],
    )?
    .with_supports_all_types();

    let anchor_block = client.get_latest_epoch_block().await?;
    let mut init_seed = [0u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let (counter_contract, counter_seed) = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into()?)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(counter_component)
        .build()?;

    println!("📄 Counter contract ID: {}", counter_contract.id().to_hex());

    client
        .add_account(&counter_contract, Some(counter_seed), false)
        .await?;

    // -------------------------------------------------------------------------
    // STEP 2 – Compile the increment script
    // -------------------------------------------------------------------------
    let script_code = fs::read_to_string("./masm/scripts/increment_script.masm")
        .expect("could not read ./masm/scripts/increment_script.masm");

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let library = create_library(
        assembler.clone(),
        "external_contract::counter_contract",
        &account_code,
    )?;

    let tx_script = TransactionScript::compile(script_code, [], assembler.with_library(&library)?)?;

    // -------------------------------------------------------------------------
    // STEP 3 – Build & send transaction
    // -------------------------------------------------------------------------
    let tx_request = TransactionRequestBuilder::new()
        .with_custom_script(tx_script)
        .build()?;

    let tx = client
        .new_transaction(counter_contract.id(), tx_request)
        .await?;

    client.submit_transaction(tx).await?;
    println!("🚀 Increment transaction submitted – waiting for finality …");

    // Give the node a moment to include the tx in a block
    sleep(Duration::from_secs(5)).await;

    // -------------------------------------------------------------------------
    // STEP 4 – Fetch contract state & verify
    // -------------------------------------------------------------------------
    delete_keystore_and_store().await; // clear before reloading

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api)
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    client.import_account_by_id(counter_contract.id()).await?;
    client.sync_state().await?;

    let account_state = client
        .get_account(counter_contract.id())
        .await?
        .expect("counter contract not found");

    let word: Word = account_state.account().storage().get_item(0)?.into();
    let counter_val = word.get(3).unwrap().as_int();
    println!("🔢 Counter value after tx: {}", counter_val);

    if counter_val != 1 {
        eprintln!("❌ Expected counter to be 1 but found {counter_val}");
        std::process::exit(1);
    }

    println!("✅ Success! The counter was incremented.");
    Ok(())
}
