#![cfg(feature = "test-bpf")]

use solana_program::pubkey::Pubkey;
use solana_program_test::*;

mod program_test;

use program_test::*;

#[tokio::test]
async fn test_create_realm_with_voter_weight_addin() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_with_voter_weight_addin().await;

    // Act

    let realm_cookie = governance_test.with_realm().await;

    // Assert

    let realm_account_data = governance_test
        .get_realm_account(&realm_cookie.address)
        .await;

    assert!(realm_account_data.config.use_community_voter_weight_addin);

    let realm_config_cookie = realm_cookie.realm_config.unwrap();

    let realm_config_data = governance_test
        .get_realm_config_data(&realm_config_cookie.address)
        .await;

    assert_eq!(realm_config_cookie.account, realm_config_data);
}

#[tokio::test]
async fn test_set_realm_voter_weight_addin_for_realm_without_addins() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_with_voter_weight_addin().await;

    let mut realm_config_args = governance_test.get_default_realm_config_args();
    realm_config_args.use_community_voter_weight_addin = false;

    let mut realm_cookie = governance_test
        .with_realm_using_config_args(&realm_config_args)
        .await;

    realm_config_args.use_community_voter_weight_addin = true;

    // Act

    governance_test
        .set_realm_config(&mut realm_cookie, &realm_config_args)
        .await
        .unwrap();

    // Assert

    let realm_account_data = governance_test
        .get_realm_account(&realm_cookie.address)
        .await;

    assert!(realm_account_data.config.use_community_voter_weight_addin);

    let realm_config_cookie = realm_cookie.realm_config.unwrap();

    let realm_config_data = governance_test
        .get_realm_config_data(&realm_config_cookie.address)
        .await;

    assert_eq!(realm_config_cookie.account, realm_config_data);
}

#[tokio::test]
async fn test_set_realm_voter_weight_addin_for_realm_without_council_and_addins() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_with_voter_weight_addin().await;

    let mut realm_config_args = governance_test.get_default_realm_config_args();
    realm_config_args.use_community_voter_weight_addin = false;
    realm_config_args.use_council_mint = false;

    let mut realm_cookie = governance_test
        .with_realm_using_config_args(&realm_config_args)
        .await;

    realm_config_args.use_community_voter_weight_addin = true;

    // Act

    governance_test
        .set_realm_config(&mut realm_cookie, &realm_config_args)
        .await
        .unwrap();

    // Assert

    let realm_account_data = governance_test
        .get_realm_account(&realm_cookie.address)
        .await;

    assert!(realm_account_data.config.use_community_voter_weight_addin);

    let realm_config_cookie = realm_cookie.realm_config.unwrap();

    let realm_config_data = governance_test
        .get_realm_config_data(&realm_config_cookie.address)
        .await;

    assert_eq!(realm_config_cookie.account, realm_config_data);
}

#[tokio::test]
async fn test_set_realm_voter_weight_addin_for_realm_with_existing_voter_weight_addin() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_with_voter_weight_addin().await;

    let mut realm_cookie = governance_test.with_realm().await;

    let mut realm_config_args = governance_test.get_default_realm_config_args();
    realm_config_args.use_community_voter_weight_addin = true;

    let community_voter_weight_addin_address = Pubkey::new_unique();

    // Act

    governance_test
        .set_realm_config_using_instruction(
            &mut realm_cookie,
            &realm_config_args,
            |i| i.accounts[7].pubkey = community_voter_weight_addin_address,
            None,
        )
        .await
        .unwrap();

    // Assert

    let realm_account_data = governance_test
        .get_realm_account(&realm_cookie.address)
        .await;

    assert!(realm_account_data.config.use_community_voter_weight_addin);

    let realm_config_cookie = realm_cookie.realm_config.unwrap();

    let realm_config_data = governance_test
        .get_realm_config_data(&realm_config_cookie.address)
        .await;

    assert_eq!(realm_config_cookie.account, realm_config_data);
    assert_eq!(
        realm_config_data.community_voter_weight_addin,
        Some(community_voter_weight_addin_address)
    );
}

#[tokio::test]
async fn test_set_realm_config_with_no_voter_weight_addin_for_realm_without_addins() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_with_voter_weight_addin().await;

    let mut realm_config_args = governance_test.get_default_realm_config_args();
    realm_config_args.use_community_voter_weight_addin = false;

    let mut realm_cookie = governance_test
        .with_realm_using_config_args(&realm_config_args)
        .await;

    realm_config_args.use_community_voter_weight_addin = false;

    // Act

    governance_test
        .set_realm_config(&mut realm_cookie, &realm_config_args)
        .await
        .unwrap();

    // Assert

    let realm_account_data = governance_test
        .get_realm_account(&realm_cookie.address)
        .await;

    assert!(!realm_account_data.config.use_community_voter_weight_addin);
}

#[tokio::test]
async fn test_set_realm_config_with_no_voter_weight_addin_for_realm_with_existing_addin() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_with_voter_weight_addin().await;
    let mut realm_cookie = governance_test.with_realm().await;

    let mut realm_config_args = governance_test.get_default_realm_config_args();
    realm_config_args.use_community_voter_weight_addin = false;

    // Act

    governance_test
        .set_realm_config(&mut realm_cookie, &realm_config_args)
        .await
        .unwrap();

    // Assert

    let realm_account_data = governance_test
        .get_realm_account(&realm_cookie.address)
        .await;

    assert!(!realm_account_data.config.use_community_voter_weight_addin);

    let realm_config_data = governance_test
        .get_realm_config_data(&realm_cookie.realm_config.unwrap().address)
        .await;

    assert!(realm_config_data.community_voter_weight_addin.is_none());
}
