use miden_client::{
    Client as MidenClient, ClientError, Felt, Word,
    account::{
        Account, AccountBuilder, AccountComponent, AccountId, AccountStorageMode, AccountType,
        StorageSlot,
    },
    assembly::CodeBuilder,
    auth::AuthSecretKey,
    builder::ClientBuilder,
    crypto::{FeltRng, rpo_falcon512::SecretKey as RpoFalcon512SecretKey},
    keystore::FilesystemKeyStore,
    note::{
        Note, NoteAssets, NoteInputs, NoteMetadata, NoteRecipient, NoteScript, NoteTag, NoteType,
    },
    rpc::{Endpoint, GrpcClient},
    store::{InputNoteRecord, NoteFilter, TransactionFilter},
    transaction::{
        OutputNote, TransactionId, TransactionKernel, TransactionRequestBuilder, TransactionScript,
        TransactionStatus,
    },
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_protocol::{
    account::AccountComponentCode,
    assembly::{Assembler, DefaultSourceManager, Library, Module, ModuleKind},
};
use miden_standards::account::{auth::AuthFalcon512Rpo, wallets::BasicWallet};
use rand::RngCore;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::time::{Duration, sleep};

type Client = MidenClient<FilesystemKeyStore>;

// Clears keystore & default sqlite file
pub async fn delete_keystore_and_store() {
    let store_path = "./store.sqlite3";
    if tokio::fs::metadata(store_path).await.is_ok() {
        if let Err(e) = tokio::fs::remove_file(store_path).await {
            eprintln!("failed to remove {}: {}", store_path, e);
        } else {
            println!("cleared sqlite store: {}", store_path);
        }
    } else {
        println!("store not found: {}", store_path);
    }

    let keystore_dir = "./keystore";
    match tokio::fs::read_dir(keystore_dir).await {
        Ok(mut dir) => {
            while let Ok(Some(entry)) = dir.next_entry().await {
                let file_path = entry.path();
                if let Err(e) = tokio::fs::remove_file(&file_path).await {
                    eprintln!("failed to remove {}: {}", file_path.display(), e);
                } else {
                    println!("removed file: {}", file_path.display());
                }
            }
        }
        Err(e) => eprintln!("failed to read directory {}: {}", keystore_dir, e),
    }
}

// Helper to instantiate Client
pub async fn instantiate_client(endpoint: Endpoint) -> Result<Client, Box<dyn std::error::Error>> {
    let timeout_ms = 10_000;
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, timeout_ms));

    let keystore_path = PathBuf::from("./keystore");
    let keystore = Arc::new(FilesystemKeyStore::new(keystore_path)?);

    let store_path = PathBuf::from("./store.sqlite3");

    let client = ClientBuilder::new()
        .rpc(rpc_client)
        .sqlite_store(store_path)
        .authenticator(keystore)
        .in_debug_mode(true.into())
        .build()
        .await?;

    Ok(client)
}

// Creates library
pub fn create_library(
    account_code: String,
    library_path: &str,
) -> Result<Library, Box<dyn std::error::Error>> {
    let assembler: Assembler = TransactionKernel::assembler();
    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        library_path,
        account_code,
        source_manager.clone() as Arc<dyn miden_protocol::assembly::SourceManager>,
    )?;
    let library = assembler.clone().assemble_library([module])?;
    Ok(library)
}

// Creates public note
pub async fn create_public_note(
    client: &mut Client,
    note_code: String,
    creator_account: Account,
    assets: NoteAssets,
) -> Result<Note, Box<dyn std::error::Error>> {
    let assembler = TransactionKernel::assembler();
    let rng = client.rng();
    let serial_num = rng.draw_word();
    let program = assembler.clone().assemble_program(note_code)?;
    let note_script = NoteScript::new(program);
    let note_inputs = NoteInputs::new([].to_vec())?;
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs.clone());
    let tag = NoteTag::new(0);
    let metadata = NoteMetadata::new(creator_account.id(), NoteType::Public, tag);

    let note = Note::new(assets, metadata, recipient);

    let note_req = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(note.clone())])
        .build()?;

    let tx_id = client
        .submit_new_transaction(creator_account.id(), note_req)
        .await?;

    wait_for_tx(client, tx_id).await?;

    Ok(note)
}

// Creates basic account
pub async fn create_basic_account(
    client: &mut Client,
    keystore: &Arc<FilesystemKeyStore>,
) -> Result<(Account, RpoFalcon512SecretKey), Box<dyn std::error::Error>> {
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = AuthSecretKey::Falcon512Rpo(RpoFalcon512SecretKey::new());

    let builder = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthFalcon512Rpo::new(key_pair.public_key().to_commitment()))
        .with_component(BasicWallet);

    let account = builder.build()?;

    client.add_account(&account, false).await?;
    keystore.add_key(&key_pair)?;

    let key = match key_pair {
        AuthSecretKey::Falcon512Rpo(k) => k,
        _ => unreachable!(),
    };

    Ok((account, key))
}

pub async fn create_no_auth_component() -> Result<AccountComponent, Box<dyn std::error::Error>> {
    let assembler: Assembler = TransactionKernel::assembler();
    let no_auth_code = fs::read_to_string(Path::new("./masm/auth/no_auth.masm"))?;

    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        "no_auth",
        no_auth_code,
        source_manager.clone() as Arc<dyn miden_protocol::assembly::SourceManager>,
    )?;
    let library = assembler.clone().assemble_library([module])?;
    let code = AccountComponentCode::from(library);

    let no_auth_component = AccountComponent::new(code, vec![])?.with_supports_all_types();

    Ok(no_auth_component)
}

// Contract builder helper function
pub async fn create_public_immutable_contract(
    client: &mut Client,
    account_code: &String,
) -> Result<Account, Box<dyn std::error::Error>> {
    let assembler: Assembler = TransactionKernel::assembler();

    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        "counter",
        account_code.clone(),
        source_manager.clone() as Arc<dyn miden_protocol::assembly::SourceManager>,
    )?;

    let library = assembler.clone().assemble_library([module])?;
    let code = AccountComponentCode::from(library);

    let counter_component = AccountComponent::new(
        code,
        vec![StorageSlot::with_value(
            "counter_slot".parse()?,
            Word::new([Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(0)]),
        )],
    )?
    .with_supports_all_types();

    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let no_auth_component = create_no_auth_component().await?;

    let counter_contract = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(no_auth_component)
        .with_component(counter_component.clone())
        .build()?;

    Ok(counter_contract)
}

pub fn create_tx_script(
    script_code: String,
    library: Option<Library>,
) -> Result<TransactionScript, Box<dyn std::error::Error>> {
    if let Some(lib) = library {
        return Ok(CodeBuilder::new()
            .with_dynamically_linked_library(&lib)?
            .compile_tx_script(script_code)?);
    };

    Ok(CodeBuilder::new().compile_tx_script(script_code)?)
}

// Waits for transaction to be committed
pub async fn wait_for_tx(client: &mut Client, tx_id: TransactionId) -> Result<(), ClientError> {
    loop {
        client.sync_state().await?;

        let txs = client
            .get_transactions(TransactionFilter::Ids(vec![tx_id]))
            .await?;

        let committed = txs
            .get(0)
            .is_some_and(|tx| matches!(tx.status, TransactionStatus::Committed { .. }));

        if committed {
            println!("✅ Transaction {} committed", tx_id.to_hex());
            return Ok(());
        }

        println!(
            "Transaction {} not yet committed. Waiting...",
            tx_id.to_hex()
        );
        sleep(Duration::from_secs(2)).await;
    }
}

// Waits for note
pub async fn wait_for_note(
    client: &mut Client,
    account_id: Option<AccountId>,
    expected: &Note,
) -> Result<(), ClientError> {
    loop {
        client.sync_state().await?;

        // Notes that can be consumed right now
        let consumable = client.get_consumable_notes(account_id).await?;

        // Notes submitted that are now committed
        let committed: Vec<InputNoteRecord> = client.get_input_notes(NoteFilter::Committed).await?;

        // Check both vectors
        let found = consumable.iter().any(|(rec, _)| rec.id() == expected.id())
            || committed.iter().any(|rec| rec.id() == expected.id());

        if found {
            println!("✅ note found {}", expected.id().to_hex());
            break;
        }

        println!("Note {} not found. Waiting...", expected.id().to_hex());
        sleep(Duration::from_secs(2)).await;
    }

    Ok(())
}
