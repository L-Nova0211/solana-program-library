#![cfg(feature = "test-bpf")]

mod program_test;
use {
    program_test::{TestContext, TokenContext},
    solana_program_test::tokio,
    solana_sdk::{
        instruction::InstructionError, signature::Signer, signer::keypair::Keypair,
        transaction::TransactionError, transport::TransportError,
    },
    spl_token_2022::{
        extension::{
            confidential_transfer::{ConfidentialTransferAccount, ConfidentialTransferMint},
            ExtensionType,
        },
        solana_zk_token_sdk::encryption::{auth_encryption::*, elgamal::*},
    },
    spl_token_client::token::{ExtensionInitializationParams, TokenError as TokenClientError},
    std::convert::TryInto,
};

#[tokio::test]
async fn ct_initialize_and_update_mint() {
    let wrong_keypair = Keypair::new();

    let ct_mint_authority = Keypair::new();
    let ct_mint = ConfidentialTransferMint {
        authority: ct_mint_authority.pubkey(),
        ..ConfidentialTransferMint::default()
    };

    let mut context = TestContext::new().await;
    context
        .init_token_with_mint(vec![
            ExtensionInitializationParams::ConfidentialTransferMint { ct_mint },
        ])
        .await
        .unwrap();

    let TokenContext { token, .. } = context.token_context.unwrap();

    let state = token.get_mint_info().await.unwrap();
    let extension = state.get_extension::<ConfidentialTransferMint>().unwrap();
    assert_eq!(*extension, ct_mint);

    // Change the authority
    let new_ct_mint_authority = Keypair::new();
    let new_ct_mint = ConfidentialTransferMint {
        authority: new_ct_mint_authority.pubkey(),
        ..ConfidentialTransferMint::default()
    };

    let err = token
        .confidential_transfer_update_mint(
            &wrong_keypair,
            new_ct_mint,
            Some(&new_ct_mint_authority),
        )
        .await
        .unwrap_err();
    assert_eq!(
        err,
        TokenClientError::Client(Box::new(TransportError::TransactionError(
            TransactionError::InstructionError(0, InstructionError::MissingRequiredSignature)
        )))
    );
    token
        .confidential_transfer_update_mint(
            &ct_mint_authority,
            new_ct_mint,
            Some(&new_ct_mint_authority),
        )
        .await
        .unwrap();

    let state = token.get_mint_info().await.unwrap();
    let extension = state.get_extension::<ConfidentialTransferMint>().unwrap();
    assert_eq!(*extension, new_ct_mint);

    // Clear the authority
    let new_ct_mint = ConfidentialTransferMint::default();
    token
        .confidential_transfer_update_mint(&new_ct_mint_authority, new_ct_mint, None)
        .await
        .unwrap();

    let state = token.get_mint_info().await.unwrap();
    let extension = state.get_extension::<ConfidentialTransferMint>().unwrap();
    assert_eq!(*extension, new_ct_mint);
}

#[tokio::test]
async fn ct_configure_token_account() {
    let ct_mint_authority = Keypair::new();
    let ct_mint = ConfidentialTransferMint {
        authority: ct_mint_authority.pubkey(),
        ..ConfidentialTransferMint::default()
    };

    let mut context = TestContext::new().await;
    context
        .init_token_with_mint(vec![
            ExtensionInitializationParams::ConfidentialTransferMint { ct_mint },
        ])
        .await
        .unwrap();

    let TokenContext { token, alice, .. } = context.token_context.unwrap();

    let alice_token_account = token
        .create_auxiliary_token_account_with_extension_space(
            &alice,
            &alice.pubkey(),
            vec![ExtensionType::ConfidentialTransferAccount],
        )
        .await
        .unwrap();

    let alice_elgamal_keypair = ElGamalKeypair::new_rand();
    let alice_ae_key = AeKey::new(&alice, &alice_token_account).unwrap();

    token
        .confidential_transfer_configure_token_account(
            dbg!(&alice_token_account),
            &alice,
            alice_elgamal_keypair.public,
            alice_ae_key.encrypt(0_u64),
        )
        .await
        .unwrap();

    let state = token.get_account_info(&alice_token_account).await.unwrap();
    let extension = state
        .get_extension::<ConfidentialTransferAccount>()
        .unwrap();
    assert!(!bool::from(&extension.approved));
    assert_eq!(
        extension.elgamal_pubkey,
        alice_elgamal_keypair.public.into()
    );
    assert_eq!(
        alice_ae_key
            .decrypt(&(extension.decryptable_available_balance.try_into().unwrap()))
            .unwrap(),
        0
    );

    token
        .confidential_transfer_approve_token_account(&alice_token_account, &ct_mint_authority)
        .await
        .unwrap();

    let state = token.get_account_info(&alice_token_account).await.unwrap();
    let extension = state
        .get_extension::<ConfidentialTransferAccount>()
        .unwrap();
    assert!(bool::from(&extension.approved));
}
