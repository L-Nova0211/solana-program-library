// Mark this test as BPF-only due to current `ProgramTest` limitations when CPIing into the system program
#![cfg(feature = "test-bpf")]

mod program_test;

use solana_program::{
    instruction::*, program_pack::Pack, pubkey::Pubkey, system_instruction, sysvar,
};
use solana_program_test::*;
use solana_sdk::{
    signature::Signer,
    transaction::{Transaction, TransactionError},
};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};

#[allow(deprecated)]
use spl_associated_token_account::create_associated_token_account as deprecated_create_associated_token_account;

use program_test::program_test;

#[tokio::test]
async fn test_associated_token_address() {
    let wallet_address = Pubkey::new_unique();
    let token_mint_address = Pubkey::new_unique();
    let associated_token_address =
        get_associated_token_address(&wallet_address, &token_mint_address);

    let (mut banks_client, payer, recent_blockhash) =
        program_test(token_mint_address, true).start().await;
    let rent = banks_client.get_rent().await.unwrap();
    let expected_token_account_balance = rent.minimum_balance(spl_token::state::Account::LEN);

    // Associated account does not exist
    assert_eq!(
        banks_client
            .get_account(associated_token_address)
            .await
            .expect("get_account"),
        None,
    );

    let mut transaction = Transaction::new_with_payer(
        &[create_associated_token_account(
            &payer.pubkey(),
            &wallet_address,
            &token_mint_address,
            &spl_token::id(),
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Associated account now exists
    let associated_account = banks_client
        .get_account(associated_token_address)
        .await
        .expect("get_account")
        .expect("associated_account not none");
    assert_eq!(
        associated_account.data.len(),
        spl_token::state::Account::LEN
    );
    assert_eq!(associated_account.owner, spl_token::id());
    assert_eq!(associated_account.lamports, expected_token_account_balance);
}

#[tokio::test]
async fn test_create_with_fewer_lamports() {
    let wallet_address = Pubkey::new_unique();
    let token_mint_address = Pubkey::new_unique();
    let associated_token_address =
        get_associated_token_address(&wallet_address, &token_mint_address);

    let (mut banks_client, payer, recent_blockhash) =
        program_test(token_mint_address, true).start().await;
    let rent = banks_client.get_rent().await.unwrap();
    let expected_token_account_balance = rent.minimum_balance(spl_token::state::Account::LEN);

    // Transfer lamports into `associated_token_address` before creating it - enough to be
    // rent-exempt for 0 data, but not for an initialized token account
    let mut transaction = Transaction::new_with_payer(
        &[system_instruction::transfer(
            &payer.pubkey(),
            &associated_token_address,
            rent.minimum_balance(0),
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    assert_eq!(
        banks_client
            .get_balance(associated_token_address)
            .await
            .unwrap(),
        rent.minimum_balance(0)
    );

    // Check that the program adds the extra lamports
    let mut transaction = Transaction::new_with_payer(
        &[create_associated_token_account(
            &payer.pubkey(),
            &wallet_address,
            &token_mint_address,
            &spl_token::id(),
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    assert_eq!(
        banks_client
            .get_balance(associated_token_address)
            .await
            .unwrap(),
        expected_token_account_balance,
    );
}

#[tokio::test]
async fn test_create_with_excess_lamports() {
    let wallet_address = Pubkey::new_unique();
    let token_mint_address = Pubkey::new_unique();
    let associated_token_address =
        get_associated_token_address(&wallet_address, &token_mint_address);

    let (mut banks_client, payer, recent_blockhash) =
        program_test(token_mint_address, true).start().await;
    let rent = banks_client.get_rent().await.unwrap();
    let expected_token_account_balance = rent.minimum_balance(spl_token::state::Account::LEN);

    // Transfer 1 lamport into `associated_token_address` before creating it
    let mut transaction = Transaction::new_with_payer(
        &[system_instruction::transfer(
            &payer.pubkey(),
            &associated_token_address,
            expected_token_account_balance + 1,
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    assert_eq!(
        banks_client
            .get_balance(associated_token_address)
            .await
            .unwrap(),
        expected_token_account_balance + 1
    );

    // Check that the program doesn't add any lamports
    let mut transaction = Transaction::new_with_payer(
        &[create_associated_token_account(
            &payer.pubkey(),
            &wallet_address,
            &token_mint_address,
            &spl_token::id(),
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    assert_eq!(
        banks_client
            .get_balance(associated_token_address)
            .await
            .unwrap(),
        expected_token_account_balance + 1
    );
}

#[tokio::test]
async fn test_create_account_mismatch() {
    let wallet_address = Pubkey::new_unique();
    let token_mint_address = Pubkey::new_unique();
    let _associated_token_address =
        get_associated_token_address(&wallet_address, &token_mint_address);

    let (mut banks_client, payer, recent_blockhash) =
        program_test(token_mint_address, true).start().await;

    let mut instruction = create_associated_token_account(
        &payer.pubkey(),
        &wallet_address,
        &token_mint_address,
        &spl_token::id(),
    );
    instruction.accounts[1] = AccountMeta::new(Pubkey::default(), false); // <-- Invalid associated_account_address

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);
    assert_eq!(
        banks_client
            .process_transaction(transaction)
            .await
            .unwrap_err()
            .unwrap(),
        TransactionError::InstructionError(0, InstructionError::InvalidSeeds)
    );

    let mut instruction = create_associated_token_account(
        &payer.pubkey(),
        &wallet_address,
        &token_mint_address,
        &spl_token::id(),
    );
    instruction.accounts[2] = AccountMeta::new(Pubkey::default(), false); // <-- Invalid wallet_address

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);
    assert_eq!(
        banks_client
            .process_transaction(transaction)
            .await
            .unwrap_err()
            .unwrap(),
        TransactionError::InstructionError(0, InstructionError::InvalidSeeds)
    );

    let mut instruction = create_associated_token_account(
        &payer.pubkey(),
        &wallet_address,
        &token_mint_address,
        &spl_token::id(),
    );
    instruction.accounts[3] = AccountMeta::new(Pubkey::default(), false); // <-- Invalid token_mint_address

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);
    assert_eq!(
        banks_client
            .process_transaction(transaction)
            .await
            .unwrap_err()
            .unwrap(),
        TransactionError::InstructionError(0, InstructionError::InvalidSeeds)
    );
}

#[tokio::test]
async fn test_create_associated_token_account_using_legacy_implicit_instruction() {
    let wallet_address = Pubkey::new_unique();
    let token_mint_address = Pubkey::new_unique();
    let associated_token_address =
        get_associated_token_address(&wallet_address, &token_mint_address);

    let (mut banks_client, payer, recent_blockhash) =
        program_test(token_mint_address, true).start().await;
    let rent = banks_client.get_rent().await.unwrap();
    let expected_token_account_balance = rent.minimum_balance(spl_token::state::Account::LEN);

    // Associated account does not exist
    assert_eq!(
        banks_client
            .get_account(associated_token_address)
            .await
            .expect("get_account"),
        None,
    );

    let mut create_associated_token_account_ix = create_associated_token_account(
        &payer.pubkey(),
        &wallet_address,
        &token_mint_address,
        &spl_token::id(),
    );

    // Use implicit  instruction and rent account to replicate the legacy invocation
    create_associated_token_account_ix.data = vec![];
    create_associated_token_account_ix
        .accounts
        .push(AccountMeta::new_readonly(sysvar::rent::id(), false));

    let mut transaction =
        Transaction::new_with_payer(&[create_associated_token_account_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Associated account now exists
    let associated_account = banks_client
        .get_account(associated_token_address)
        .await
        .expect("get_account")
        .expect("associated_account not none");
    assert_eq!(
        associated_account.data.len(),
        spl_token::state::Account::LEN
    );
    assert_eq!(associated_account.owner, spl_token::id());
    assert_eq!(associated_account.lamports, expected_token_account_balance);
}

#[tokio::test]
async fn test_create_associated_token_account_using_deprecated_instruction_creator() {
    let wallet_address = Pubkey::new_unique();
    let token_mint_address = Pubkey::new_unique();
    let associated_token_address =
        get_associated_token_address(&wallet_address, &token_mint_address);

    let (mut banks_client, payer, recent_blockhash) =
        program_test(token_mint_address, true).start().await;
    let rent = banks_client.get_rent().await.unwrap();
    let expected_token_account_balance = rent.minimum_balance(spl_token::state::Account::LEN);

    // Associated account does not exist
    assert_eq!(
        banks_client
            .get_account(associated_token_address)
            .await
            .expect("get_account"),
        None,
    );

    // Use legacy instruction creator
    #[allow(deprecated)]
    let create_associated_token_account_ix = deprecated_create_associated_token_account(
        &payer.pubkey(),
        &wallet_address,
        &token_mint_address,
    );

    let mut transaction =
        Transaction::new_with_payer(&[create_associated_token_account_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Associated account now exists
    let associated_account = banks_client
        .get_account(associated_token_address)
        .await
        .expect("get_account")
        .expect("associated_account not none");
    assert_eq!(
        associated_account.data.len(),
        spl_token::state::Account::LEN
    );
    assert_eq!(associated_account.owner, spl_token::id());
    assert_eq!(associated_account.lamports, expected_token_account_balance);
}
