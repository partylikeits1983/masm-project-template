use masm_project_template::common::{
    create_basic_account, create_library, delete_keystore_and_store, wait_for_note,
};
use miden_client::{
    ClientError, Felt,
    account::{
        AccountBuilder, AccountStorageMode, AccountType, StorageSlot, component::AccountComponent,
    },
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    note::{
        Note, NoteAssets, NoteExecutionHint, NoteExecutionMode, NoteInputs, NoteMetadata,
        NoteRecipient, NoteScript, NoteTag, NoteType,
    },
    rpc::{Endpoint, TonicRpcClient},
    transaction::{OutputNote, TransactionKernel, TransactionRequestBuilder},
};
use miden_crypto::Word;
use miden_crypto::rand::FeltRng;
use miden_objects::{assembly::Assembler, transaction::TransactionScript};
use rand::RngCore;
use std::{fs, path::Path, sync::Arc};
use tokio::time::{Duration, sleep};

#[tokio::test]
async fn increment_counter_with_script() -> Result<(), ClientError> {
    delete_keystore_and_store().await;

    let endpoint = Endpoint::localhost();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api.clone())
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create counter smart contract
    // -------------------------------------------------------------------------
    let file_path = Path::new("./masm/accounts/counter.masm");
    let account_code = fs::read_to_string(file_path).unwrap();

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    let counter_component = AccountComponent::compile(
        account_code,
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

    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let (counter_contract, counter_seed) = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(counter_component.clone())
        .build()
        .unwrap();

    println!("contract id: {:?}", counter_contract.id().to_hex());

    client
        .add_account(&counter_contract, Some(counter_seed), false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 2: Prepare the Script
    // -------------------------------------------------------------------------
    let script_code =
        fs::read_to_string(Path::new("./masm/scripts/increment_script.masm")).unwrap();

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let file_path = Path::new("./masm/accounts/counter.masm");
    let account_code = fs::read_to_string(file_path).unwrap();

    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::counter_contract",
        &account_code,
    )
    .unwrap();

    let tx_script = TransactionScript::compile(
        script_code,
        [],
        assembler.with_library(&account_component_lib).unwrap(),
    )
    .unwrap();

    // -------------------------------------------------------------------------
    // STEP 3: Build & Submit Transaction
    // -------------------------------------------------------------------------
    let tx_increment_request = TransactionRequestBuilder::new()
        .with_custom_script(tx_script)
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(counter_contract.id(), tx_increment_request)
        .await
        .unwrap();

    let _ = client.submit_transaction(tx_result).await;

    // -------------------------------------------------------------------------
    // STEP 4: Validate Updated State
    // -------------------------------------------------------------------------
    sleep(Duration::from_secs(5)).await;

    delete_keystore_and_store().await;

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api.clone())
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    client
        .import_account_by_id(counter_contract.id())
        .await
        .unwrap();

    client.sync_state().await.unwrap();

    let new_account_state = client.get_account(counter_contract.id()).await.unwrap();

    if let Some(account) = new_account_state.as_ref() {
        let count: Word = account.account().storage().get_item(0).unwrap().into();
        let val = count.get(3).unwrap().as_int();
        assert_eq!(val, 1);
    }

    Ok(())
}

#[tokio::test]
async fn increment_counter_with_note() -> Result<(), ClientError> {
    delete_keystore_and_store().await;

    let endpoint = Endpoint::localhost();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api.clone())
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap();

    // -------------------------------------------------------------------------
    // STEP 1: Create Basic User Account
    // -------------------------------------------------------------------------
    let (alice_account, _) = create_basic_account(&mut client, keystore.clone()).await?;

    // -------------------------------------------------------------------------
    // STEP 2: Create Counter Smart Contract
    // -------------------------------------------------------------------------
    let file_path = Path::new("./masm/accounts/counter.masm");
    let account_code = fs::read_to_string(file_path).unwrap();

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    let counter_component = AccountComponent::compile(
        account_code.clone(),
        assembler.clone(),
        vec![StorageSlot::Value([
            Felt::new(0),
            Felt::new(0),
            Felt::new(0),
            Felt::new(0),
        ])],
    )
    .unwrap()
    .with_supports_all_types();

    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let (counter_contract, counter_seed) = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(counter_component.clone())
        .build()
        .unwrap();

    println!("counter contract id: {:?}", counter_contract.id().to_hex());

    client
        .add_account(&counter_contract, Some(counter_seed), false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 3: Prepare & Create the Note
    // -------------------------------------------------------------------------
    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::counter_contract",
        &account_code,
    )
    .unwrap();

    let assembler = TransactionKernel::assembler()
        .with_library(&account_component_lib)
        .unwrap()
        .with_debug_mode(true);
    let code = fs::read_to_string(Path::new("./masm/notes/increment_note.masm")).unwrap();
    let rng = client.rng();
    let serial_num = rng.draw_word();
    let note_script = NoteScript::compile(code, assembler.clone()).unwrap();
    let note_inputs = NoteInputs::new([].to_vec()).unwrap();
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs.clone());
    let tag = NoteTag::for_public_use_case(0, 0, NoteExecutionMode::Local).unwrap();
    let metadata = NoteMetadata::new(
        alice_account.id(),
        NoteType::Public,
        tag,
        NoteExecutionHint::always(),
        Felt::new(0),
    )?;
    let vault = NoteAssets::new(vec![])?;
    let increment_note = Note::new(vault, metadata, recipient);

    let note_req = TransactionRequestBuilder::new()
        .with_own_output_notes(vec![OutputNote::Full(increment_note.clone())])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(alice_account.id(), note_req)
        .await
        .unwrap();

    let _ = client.submit_transaction(tx_result).await;
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 4: Consume the Note
    // -------------------------------------------------------------------------
    wait_for_note(&mut client, &counter_contract, &increment_note).await?;

    let script_code = fs::read_to_string(Path::new("./masm/scripts/consume_script.masm")).unwrap();
    let tx_script = TransactionScript::compile(script_code, [], assembler).unwrap();

    let consume_custom_req = TransactionRequestBuilder::new()
        .with_authenticated_input_notes([(increment_note.id(), None)])
        .with_custom_script(tx_script)
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(counter_contract.id(), consume_custom_req)
        .await
        .unwrap();
    let _ = client.submit_transaction(tx_result).await;

    // -------------------------------------------------------------------------
    // STEP 5: Validate Updated State
    // -------------------------------------------------------------------------
    sleep(Duration::from_secs(5)).await;

    delete_keystore_and_store().await;

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api.clone())
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    client
        .import_account_by_id(counter_contract.id())
        .await
        .unwrap();

    client.sync_state().await.unwrap();

    let new_account_state = client.get_account(counter_contract.id()).await.unwrap();

    if let Some(account) = new_account_state.as_ref() {
        let count: Word = account.account().storage().get_item(0).unwrap().into();
        let val = count.get(3).unwrap().as_int();
        assert_eq!(val, 1);
    }

    Ok(())
}
