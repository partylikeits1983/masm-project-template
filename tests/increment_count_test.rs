use masm_project_template::common::{
    create_basic_account, create_library, create_public_immutable_contract, create_public_note,
    create_tx_script, delete_keystore_and_store, instantiate_client, wait_for_note, wait_for_tx,
};
use miden_client::{
    ClientError, Word, keystore::FilesystemKeyStore, note::NoteAssets, rpc::Endpoint,
    transaction::TransactionRequestBuilder,
};
use miden_protocol::address::NetworkId;
use std::{fs, path::Path, sync::Arc};
use tokio::time::{Duration, sleep};

#[tokio::test]
async fn increment_counter_with_script() -> Result<(), ClientError> {
    delete_keystore_and_store().await;

    let endpoint = Endpoint::localhost();
    let mut client = instantiate_client(endpoint.clone()).await.unwrap();

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create counter smart contract
    // -------------------------------------------------------------------------
    let counter_code = fs::read_to_string(Path::new("./masm/accounts/counter.masm")).unwrap();

    let counter_contract = create_public_immutable_contract(&mut client, &counter_code)
        .await
        .unwrap();
    println!("contract id: {:?}", counter_contract.id().to_hex());

    client.add_account(&counter_contract, false).await.unwrap();

    // -------------------------------------------------------------------------
    // STEP 2: Prepare the Script
    // -------------------------------------------------------------------------
    let script_code =
        fs::read_to_string(Path::new("./masm/scripts/increment_script.masm")).unwrap();

    let library_path = "external_contract::counter_contract";

    let library = create_library(counter_code, library_path).unwrap();

    let tx_script = create_tx_script(script_code, Some(library)).unwrap();

    // -------------------------------------------------------------------------
    // STEP 3: Build & Submit Transaction
    // -------------------------------------------------------------------------
    let tx_increment_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_id = client
        .submit_new_transaction(counter_contract.id(), tx_increment_request)
        .await
        .unwrap();

    wait_for_tx(&mut client, tx_id).await.unwrap();

    // -------------------------------------------------------------------------
    // STEP 4: Validate Updated State
    // -------------------------------------------------------------------------
    sleep(Duration::from_secs(7)).await;

    delete_keystore_and_store().await;

    let mut client = instantiate_client(endpoint).await.unwrap();

    client
        .import_account_by_id(counter_contract.id())
        .await
        .unwrap();

    let new_account_record = client
        .get_account(counter_contract.id())
        .await
        .unwrap()
        .unwrap();

    let count: Word = new_account_record
        .account_data()
        .storage()
        .get_item(0)
        .unwrap()
        .into();
    let val = count.get(3).unwrap().as_int();
    assert_eq!(val, 1);

    Ok(())
}

#[tokio::test]
async fn increment_counter_with_note() -> Result<(), ClientError> {
    delete_keystore_and_store().await;

    let endpoint = Endpoint::localhost();
    let mut client = instantiate_client(endpoint.clone()).await.unwrap();

    let keystore_path = std::path::PathBuf::from("./keystore");
    let keystore = Arc::new(FilesystemKeyStore::new(keystore_path).unwrap());

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create Basic User Account
    // -------------------------------------------------------------------------
    let (alice_account, _) = create_basic_account(&mut client, &keystore).await.unwrap();

    println!(
        "alice account id: {:?}",
        alice_account.id().to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 2: Create Counter Smart Contract
    // -------------------------------------------------------------------------
    let counter_code = fs::read_to_string(Path::new("./masm/accounts/counter.masm")).unwrap();

    let counter_contract = create_public_immutable_contract(&mut client, &counter_code)
        .await
        .unwrap();

    println!(
        "contract id: {:?}",
        counter_contract.id().to_bech32(NetworkId::Testnet)
    );

    client.add_account(&counter_contract, false).await.unwrap();

    // -------------------------------------------------------------------------
    // STEP 3: Prepare & Create the Note
    // -------------------------------------------------------------------------
    let note_code = fs::read_to_string(Path::new("./masm/notes/increment_note.masm")).unwrap();

    let note_assets = NoteAssets::new(vec![]).unwrap();

    let increment_note = create_public_note(&mut client, note_code, alice_account, note_assets)
        .await
        .unwrap();

    println!("increment note created, waiting for onchain commitment");

    // -------------------------------------------------------------------------
    // STEP 4: Consume the Note
    // -------------------------------------------------------------------------
    wait_for_note(&mut client, None, &increment_note)
        .await
        .unwrap();

    let script_code = fs::read_to_string(Path::new("./masm/scripts/nop_script.masm")).unwrap();
    let tx_script = create_tx_script(script_code, None).unwrap();

    let consume_custom_req = TransactionRequestBuilder::new()
        .input_notes(vec![(increment_note, None)])
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_id = client
        .submit_new_transaction(counter_contract.id(), consume_custom_req)
        .await
        .unwrap();

    wait_for_tx(&mut client, tx_id).await.unwrap();

    // -------------------------------------------------------------------------
    // STEP 5: Validate Updated State
    // -------------------------------------------------------------------------
    sleep(Duration::from_secs(5)).await;

    delete_keystore_and_store().await;

    let mut client = instantiate_client(endpoint).await.unwrap();

    client
        .import_account_by_id(counter_contract.id())
        .await
        .unwrap();

    let new_account_record = client.get_account(counter_contract.id()).await.unwrap();

    if let Some(account_record) = new_account_record.as_ref() {
        let count: Word = account_record
            .account_data()
            .storage()
            .get_item(0)
            .unwrap()
            .into();
        let val = count.get(3).unwrap().as_int();
        assert_eq!(val, 1);
    }

    Ok(())
}
