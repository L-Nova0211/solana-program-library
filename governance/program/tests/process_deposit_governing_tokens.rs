#![cfg(feature = "test-bpf")]

use solana_program::instruction::AccountMeta;
use solana_program_test::*;

mod program_test;

use program_test::*;
use solana_sdk::signature::{Keypair, Signer};
use spl_governance::{error::GovernanceError, instruction::deposit_governing_tokens};

#[tokio::test]
async fn test_deposit_initial_community_tokens() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;
    let realm_cookie = governance_test.with_realm().await;

    // Act
    let voter_record_cookie = governance_test
        .with_initial_community_token_deposit(&realm_cookie)
        .await;

    // Assert

    let voter_record = governance_test
        .get_voter_record_account(&voter_record_cookie.address)
        .await;

    assert_eq!(voter_record_cookie.account, voter_record);

    let source_account = governance_test
        .get_token_account(&voter_record_cookie.token_source)
        .await;

    assert_eq!(
        voter_record_cookie.token_source_amount - voter_record_cookie.account.token_deposit_amount,
        source_account.amount
    );

    let holding_account = governance_test
        .get_token_account(&realm_cookie.community_token_holding_account)
        .await;

    assert_eq!(voter_record.token_deposit_amount, holding_account.amount);
}

#[tokio::test]
async fn test_deposit_initial_council_tokens() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;
    let realm_cookie = governance_test.with_realm().await;

    let council_token_holding_account = realm_cookie.council_token_holding_account.unwrap();

    // Act
    let voter_record_cookie = governance_test
        .with_initial_council_token_deposit(&realm_cookie)
        .await;

    // Assert
    let voter_record = governance_test
        .get_voter_record_account(&voter_record_cookie.address)
        .await;

    assert_eq!(voter_record_cookie.account, voter_record);

    let source_account = governance_test
        .get_token_account(&voter_record_cookie.token_source)
        .await;

    assert_eq!(
        voter_record_cookie.token_source_amount - voter_record_cookie.account.token_deposit_amount,
        source_account.amount
    );

    let holding_account = governance_test
        .get_token_account(&council_token_holding_account)
        .await;

    assert_eq!(voter_record.token_deposit_amount, holding_account.amount);
}

#[tokio::test]
async fn test_deposit_subsequent_community_tokens() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;
    let realm_cookie = governance_test.with_realm().await;

    let voter_record_cookie = governance_test
        .with_initial_community_token_deposit(&realm_cookie)
        .await;

    let deposit_amount = 5;
    let total_deposit_amount = voter_record_cookie.account.token_deposit_amount + deposit_amount;

    // Act
    governance_test
        .with_community_token_deposit(&realm_cookie, &voter_record_cookie, deposit_amount)
        .await;

    // Assert
    let voter_record = governance_test
        .get_voter_record_account(&voter_record_cookie.address)
        .await;

    assert_eq!(total_deposit_amount, voter_record.token_deposit_amount);

    let holding_account = governance_test
        .get_token_account(&realm_cookie.community_token_holding_account)
        .await;

    assert_eq!(total_deposit_amount, holding_account.amount);
}

#[tokio::test]
async fn test_deposit_subsequent_council_tokens() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;
    let realm_cookie = governance_test.with_realm().await;

    let council_token_holding_account = realm_cookie.council_token_holding_account.unwrap();

    let voter_record_cookie = governance_test
        .with_initial_council_token_deposit(&realm_cookie)
        .await;

    let deposit_amount = 5;
    let total_deposit_amount = voter_record_cookie.account.token_deposit_amount + deposit_amount;

    // Act
    governance_test
        .with_council_token_deposit(&realm_cookie, &voter_record_cookie, deposit_amount)
        .await;

    // Assert
    let voter_record = governance_test
        .get_voter_record_account(&voter_record_cookie.address)
        .await;

    assert_eq!(total_deposit_amount, voter_record.token_deposit_amount);

    let holding_account = governance_test
        .get_token_account(&council_token_holding_account)
        .await;

    assert_eq!(total_deposit_amount, holding_account.amount);
}

#[tokio::test]
async fn test_deposit_initial_community_tokens_with_owner_must_sign_error() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;
    let realm_cookie = governance_test.with_realm().await;

    let token_owner = Keypair::new();
    let transfer_authority = Keypair::new();
    let token_source = Keypair::new();

    governance_test
        .create_token_account_with_transfer_authority(
            &token_source,
            &realm_cookie.account.community_mint,
            &realm_cookie.community_mint_authority,
            10,
            &token_owner,
            &transfer_authority.pubkey(),
        )
        .await;

    let mut instruction = deposit_governing_tokens(
        &realm_cookie.address,
        &token_source.pubkey(),
        &token_owner.pubkey(),
        &transfer_authority.pubkey(),
        &governance_test.payer.pubkey(),
        &realm_cookie.account.community_mint,
    );

    instruction.accounts[3] = AccountMeta::new_readonly(token_owner.pubkey(), false);

    // // Act

    let error = governance_test
        .process_transaction(&[instruction], Some(&[&transfer_authority]))
        .await
        .err()
        .unwrap();

    // Assert
    assert_eq!(error, GovernanceError::GoverningTokenOwnerMustSign.into());
}
#[tokio::test]
async fn test_deposit_initial_community_tokens_with_invalid_owner_error() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;
    let realm_cookie = governance_test.with_realm().await;

    let token_owner = Keypair::new();
    let transfer_authority = Keypair::new();
    let token_source = Keypair::new();

    let invalid_owner = Keypair::new();

    governance_test
        .create_token_account_with_transfer_authority(
            &token_source,
            &realm_cookie.account.community_mint,
            &realm_cookie.community_mint_authority,
            10,
            &token_owner,
            &transfer_authority.pubkey(),
        )
        .await;

    let instruction = deposit_governing_tokens(
        &realm_cookie.address,
        &token_source.pubkey(),
        &invalid_owner.pubkey(),
        &transfer_authority.pubkey(),
        &governance_test.payer.pubkey(),
        &realm_cookie.account.community_mint,
    );

    // // Act

    let error = governance_test
        .process_transaction(&[instruction], Some(&[&transfer_authority, &invalid_owner]))
        .await
        .err()
        .unwrap();

    // Assert
    assert_eq!(error, GovernanceError::GoverningTokenOwnerMustSign.into());
}
