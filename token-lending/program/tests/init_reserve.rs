#![cfg(feature = "test-bpf")]

mod helpers;

use helpers::*;
use solana_program_test::*;
use solana_sdk::{
    instruction::InstructionError,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use spl_token_lending::{
    error::LendingError, instruction::init_reserve, processor::process_instruction,
    state::INITIAL_COLLATERAL_RATE,
};

#[tokio::test]
async fn test_success() {
    let mut test = ProgramTest::new(
        "spl_token_lending",
        spl_token_lending::id(),
        processor!(process_instruction),
    );

    let user_accounts_owner = Keypair::new();
    let sol_usdc_dex_market = TestDexMarket::add_sol_usdc_dex_market(&mut test);
    let usdc_mint = add_usdc_mint(&mut test);
    let lending_market = add_lending_market(&mut test, usdc_mint.pubkey);
    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    const RESERVE_AMOUNT: u64 = 42;

    let sol_user_liquidity_account = create_and_mint_to_token_account(
        &mut banks_client,
        spl_token::native_mint::id(),
        None,
        &payer,
        user_accounts_owner.pubkey(),
        RESERVE_AMOUNT,
    )
    .await;

    let sol_reserve = TestReserve::init(
        &mut banks_client,
        &lending_market,
        RESERVE_AMOUNT,
        spl_token::native_mint::id(),
        sol_user_liquidity_account,
        &payer,
        &user_accounts_owner,
        &sol_usdc_dex_market,
    )
    .await;

    sol_reserve.validate_state(&mut banks_client).await;

    let sol_liquidity_supply =
        get_token_balance(&mut banks_client, sol_reserve.liquidity_supply).await;
    assert_eq!(sol_liquidity_supply, RESERVE_AMOUNT);
    let user_sol_balance =
        get_token_balance(&mut banks_client, sol_reserve.user_liquidity_account).await;
    assert_eq!(user_sol_balance, 0);
    let user_sol_collateral_balance =
        get_token_balance(&mut banks_client, sol_reserve.user_collateral_account).await;
    assert_eq!(
        user_sol_collateral_balance,
        INITIAL_COLLATERAL_RATE * RESERVE_AMOUNT
    );
}

#[tokio::test]
async fn test_already_initialized() {
    let mut test = ProgramTest::new(
        "spl_token_lending",
        spl_token_lending::id(),
        processor!(process_instruction),
    );

    let user_accounts_owner = Keypair::new();
    let sol_usdc_dex_market = TestDexMarket::add_sol_usdc_dex_market(&mut test);
    let usdc_mint = add_usdc_mint(&mut test);
    let lending_market = add_lending_market(&mut test, usdc_mint.pubkey);

    let usdc_reserve = add_reserve(
        &mut test,
        &user_accounts_owner,
        &lending_market,
        AddReserveArgs {
            dex_market_pubkey: Some(sol_usdc_dex_market.pubkey),
            liquidity_amount: 42,
            liquidity_mint_decimals: usdc_mint.decimals,
            liquidity_mint_pubkey: usdc_mint.pubkey,
            config: TEST_RESERVE_CONFIG,
            ..AddReserveArgs::default()
        },
    );

    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    let mut transaction = Transaction::new_with_payer(
        &[init_reserve(
            spl_token_lending::id(),
            42,
            TEST_RESERVE_CONFIG,
            usdc_reserve.user_liquidity_account,
            usdc_reserve.user_collateral_account,
            usdc_reserve.pubkey,
            usdc_reserve.liquidity_mint,
            usdc_reserve.liquidity_supply,
            usdc_reserve.collateral_mint,
            usdc_reserve.collateral_supply,
            lending_market.keypair.pubkey(),
            Some(sol_usdc_dex_market.pubkey),
        )],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer, &lending_market.keypair], recent_blockhash);
    assert_eq!(
        banks_client
            .process_transaction(transaction)
            .await
            .unwrap_err()
            .unwrap(),
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(LendingError::AlreadyInitialized as u32)
        )
    );
}
