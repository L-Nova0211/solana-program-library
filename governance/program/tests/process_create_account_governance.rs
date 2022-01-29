#![cfg(feature = "test-bpf")]
mod program_test;

use solana_program_test::*;

use program_test::*;
use spl_governance::{error::GovernanceError, state::enums::VoteThresholdPercentage};
use spl_governance_tools::error::GovernanceToolsError;

#[tokio::test]
async fn test_create_account_governance() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;

    let realm_cookie = governance_test.with_realm().await;
    let governed_account_cookie = governance_test.with_governed_account().await;

    let token_owner_record_cookie = governance_test
        .with_community_token_deposit(&realm_cookie)
        .await
        .unwrap();

    // Act
    let account_governance_cookie = governance_test
        .with_account_governance(
            &realm_cookie,
            &governed_account_cookie,
            &token_owner_record_cookie,
        )
        .await
        .unwrap();

    // Assert
    let account_governance_account = governance_test
        .get_governance_account(&account_governance_cookie.address)
        .await;

    assert_eq!(
        account_governance_cookie.account,
        account_governance_account
    );
}

#[tokio::test]
async fn test_create_account_governance_with_invalid_realm_error() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;

    let mut realm_cookie = governance_test.with_realm().await;
    let governed_account_cookie = governance_test.with_governed_account().await;

    let token_owner_record_cookie = governance_test
        .with_community_token_deposit(&realm_cookie)
        .await
        .unwrap();

    let account_governance_cookie = governance_test
        .with_account_governance(
            &realm_cookie,
            &governed_account_cookie,
            &token_owner_record_cookie,
        )
        .await
        .unwrap();

    realm_cookie.address = account_governance_cookie.address;

    // Act
    let err = governance_test
        .with_account_governance(
            &realm_cookie,
            &governed_account_cookie,
            &token_owner_record_cookie,
        )
        .await
        .err()
        .unwrap();

    // Assert

    assert_eq!(err, GovernanceToolsError::InvalidAccountType.into());
}

#[tokio::test]
async fn test_create_account_governance_with_invalid_config_error() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;

    let realm_cookie = governance_test.with_realm().await;
    let governed_account_cookie = governance_test.with_governed_account().await;

    let token_owner_record_cookie = governance_test
        .with_community_token_deposit(&realm_cookie)
        .await
        .unwrap();

    // Arrange
    let mut config = governance_test.get_default_governance_config();
    config.vote_threshold_percentage = VoteThresholdPercentage::YesVote(0); // below 1% threshold

    // Act
    let err = governance_test
        .with_account_governance_using_config(
            &realm_cookie,
            &governed_account_cookie,
            &token_owner_record_cookie,
            &config,
        )
        .await
        .err()
        .unwrap();

    // Assert

    assert_eq!(err, GovernanceError::InvalidVoteThresholdPercentage.into());

    // Arrange
    let mut config = governance_test.get_default_governance_config();
    config.vote_threshold_percentage = VoteThresholdPercentage::YesVote(101); // Above 100% threshold

    // Act
    let err = governance_test
        .with_account_governance_using_config(
            &realm_cookie,
            &governed_account_cookie,
            &token_owner_record_cookie,
            &config,
        )
        .await
        .err()
        .unwrap();

    // Assert

    assert_eq!(err, GovernanceError::InvalidVoteThresholdPercentage.into());
}

#[tokio::test]
async fn test_create_account_governance_with_not_enough_community_tokens_error() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;

    let realm_cookie = governance_test.with_realm().await;
    let governed_account_cookie = governance_test.with_governed_account().await;

    // Set token deposit amount below the required threshold
    let token_amount = 4;

    let token_owner_record_cookie = governance_test
        .with_community_token_deposit_amount(&realm_cookie, token_amount)
        .await
        .unwrap();

    // Act
    let err = governance_test
        .with_account_governance(
            &realm_cookie,
            &governed_account_cookie,
            &token_owner_record_cookie,
        )
        .await
        .err()
        .unwrap();

    // Assert
    assert_eq!(
        err,
        GovernanceError::NotEnoughTokensToCreateGovernance.into()
    );
}

#[tokio::test]
async fn test_create_account_governance_with_not_enough_council_tokens_error() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;

    let realm_cookie = governance_test.with_realm().await;
    let governed_account_cookie = governance_test.with_governed_account().await;

    // Set token deposit amount below the required threshold
    let token_amount: u64 = 0;

    let token_owner_record_cookie = governance_test
        .with_council_token_deposit_amount(&realm_cookie, token_amount)
        .await
        .unwrap();

    // Act
    let err = governance_test
        .with_account_governance(
            &realm_cookie,
            &governed_account_cookie,
            &token_owner_record_cookie,
        )
        .await
        .err()
        .unwrap();

    // Assert
    assert_eq!(
        err,
        GovernanceError::NotEnoughTokensToCreateGovernance.into()
    );
}

#[tokio::test]
async fn test_create_account_governance_using_realm_authority() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;

    let realm_cookie = governance_test.with_realm().await;
    let governed_account_cookie = governance_test.with_governed_account().await;

    let config = governance_test.get_default_governance_config();
    let realm_authority = realm_cookie.realm_authority.as_ref().unwrap();

    // Act
    let account_governance_cookie = governance_test
        .with_account_governance_impl(
            &realm_cookie,
            &governed_account_cookie,
            None,
            &realm_authority,
            None,
            &config,
            None,
        )
        .await
        .unwrap();

    // Assert
    let account_governance_account = governance_test
        .get_governance_account(&account_governance_cookie.address)
        .await;

    assert_eq!(
        account_governance_cookie.account,
        account_governance_account
    );
}

#[tokio::test]
async fn test_create_account_governance_using_realm_authority_with_authority_must_sign_error() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;

    let realm_cookie = governance_test.with_realm().await;
    let governed_account_cookie = governance_test.with_governed_account().await;

    let config = governance_test.get_default_governance_config();
    let realm_authority = realm_cookie.realm_authority.as_ref().unwrap();

    // Act
    let err = governance_test
        .with_account_governance_impl(
            &realm_cookie,
            &governed_account_cookie,
            None,
            &realm_authority,
            None,
            &config,
            Some(&[]),
        )
        .await
        .err()
        .unwrap();

    // Assert
    assert_eq!(err, GovernanceError::RealmAuthorityMustSign.into());
}
