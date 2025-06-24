use masm_project_template::common::create_network_note;

use miden_client_tools::{create_basic_account, create_library, delete_keystore_and_store};

use miden_assembly::{Assembler, diagnostics::NamedSource};
use miden_client::{
    ClientError, Felt,
    account::{
        AccountBuilder, AccountStorageMode, AccountType, StorageSlot, component::AccountComponent,
    },
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    note::NoteAssets,
    rpc::{Endpoint, TonicRpcClient},
    transaction::{TransactionKernel, TransactionRequestBuilder, TransactionScript},
};
use miden_crypto::Word;
use miden_objects::account::NetworkId;
use rand::RngCore;
use std::sync::Arc;
use std::{fs, path::Path, vec};
use tokio::time::{Duration, sleep};

#[tokio::test]
async fn increment_counter_with_note() -> Result<(), ClientError> {
    delete_keystore_and_store(None).await;

    let endpoint = Endpoint::localhost();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api)
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap();

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create a basic counter contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating counter contract.");

    // Load the MASM file for the counter contract
    let counter_path = Path::new("./masm/accounts/counter.masm");
    let counter_code = fs::read_to_string(counter_path).unwrap();

    // Prepare assembler
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Compile the account code into `AccountComponent` with one storage slot
    let counter_component = AccountComponent::compile(
        counter_code.clone(),
        assembler,
        vec![StorageSlot::Value([
            Felt::new(0),
            Felt::new(0),
            Felt::new(0),
            Felt::new(0),
        ])],
    )
    .unwrap()
    .with_supports_all_types();

    // Init seed for the counter contract
    let mut seed = [0_u8; 32];
    client.rng().fill_bytes(&mut seed);

    // Anchor block of the account
    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    // Build the new `Account` with the component
    let (counter_contract, counter_seed) = AccountBuilder::new(seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Network)
        .with_component(counter_component.clone())
        .build()
        .unwrap();

    println!(
        "counter_contract id: {:?}",
        counter_contract.id().to_bech32(NetworkId::Testnet)
    );
    println!("counter_contract storage: {:?}", counter_contract.storage());

    client
        .add_account(&counter_contract.clone(), Some(counter_seed), false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 2: Deploy the Counter Contract with a script
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Deploy Counter Contract With Script");

    // Load the MASM script referencing the increment procedure
    let script_path = Path::new("./masm/scripts/increment_script.masm");
    let script_code = fs::read_to_string(script_path).unwrap();

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    let library_path = "external_contract::counter_contract";
    let account_component_lib = assembler
        .clone()
        .assemble_library([NamedSource::new(library_path, counter_code)])
        .unwrap();

    let tx_script = TransactionScript::compile(
        script_code,
        [],
        assembler.with_library(&account_component_lib).unwrap(),
    )
    .unwrap();

    // Build a transaction request with the custom script
    let tx_increment_request = TransactionRequestBuilder::new()
        .with_custom_script(tx_script)
        .build()
        .unwrap();

    // Execute the transaction locally
    let tx_result = client
        .new_transaction(counter_contract.id(), tx_increment_request)
        .await
        .unwrap();

    let tx_id = tx_result.executed_transaction().id();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    // Submit transaction to the network
    let _ = client.submit_transaction(tx_result).await;

    client.sync_state().await.unwrap();

    // Retrieve updated contract data to see the incremented counter
    let account = client.get_account(counter_contract.id()).await.unwrap();
    println!(
        "counter contract storage: {:?}",
        account.unwrap().account().storage().get_item(0)
    );

    // -------------------------------------------------------------------------
    // STEP 3: Create Basic User Account
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Create Basic User Account");

    let (alice_account, _) = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 4: Create Network Increment Note
    // -------------------------------------------------------------------------
    println!("\n[STEP 4]  Create Network Increment Note");

    let note_code = fs::read_to_string(Path::new("./masm/notes/increment_note.masm")).unwrap();

    let account_code = fs::read_to_string(Path::new("./masm/accounts/counter.masm")).unwrap();
    let library_path = "external_contract::counter_contract";
    let library = create_library(account_code, library_path).unwrap();
    let note_assets: NoteAssets = NoteAssets::new(vec![]).unwrap();

    let _increment_note =
        create_network_note(&mut client, note_code, library, alice_account, note_assets)
            .await
            .unwrap();

    // -------------------------------------------------------------------------
    // STEP 5: Validate Updated State
    // -------------------------------------------------------------------------
    println!("\n[STEP 5]  Validate Updated State");
    sleep(Duration::from_secs(5)).await;

    delete_keystore_and_store(None).await;

    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api)
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    client
        .import_account_by_id(counter_contract.id())
        .await
        .unwrap();

    let new_account_state = client.get_account(counter_contract.id()).await.unwrap();

    if let Some(account) = new_account_state.as_ref() {
        let count: Word = account.account().storage().get_item(0).unwrap().into();
        let val = count.get(3).unwrap().as_int();
        assert_eq!(val, 1);
    }

    Ok(())
}
