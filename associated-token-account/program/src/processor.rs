//! Program state processor

use crate::*;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    info,
    log::sol_log_compute_units,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};

/// Instruction processor
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _input: &[u8],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let funder_info = next_account_info(account_info_iter)?;
    let associated_token_account_info = next_account_info(account_info_iter)?;
    let wallet_account_info = next_account_info(account_info_iter)?;
    let spl_token_mint_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let spl_token_program_info = next_account_info(account_info_iter)?;
    let rent_sysvar_info = next_account_info(account_info_iter)?;

    let (associated_token_address, bump_seed) = get_associated_token_address_and_bump_seed(
        &wallet_account_info.key,
        &spl_token_mint_info.key,
        program_id,
    );
    if associated_token_address != *associated_token_account_info.key {
        info!("Error: Associated address does not match seed derivation");
        return Err(ProgramError::InvalidSeeds);
    }

    let associated_token_account_signer_seeds: &[&[_]] = &[
        &wallet_account_info.key.to_bytes(),
        &spl_token::id().to_bytes(),
        &spl_token_mint_info.key.to_bytes(),
        &[bump_seed],
    ];

    sol_log_compute_units();

    // Fund the associated token account with the minimum balance to be rent exempt
    let rent = &Rent::from_account_info(rent_sysvar_info)?;
    let required_lamports = rent
        .minimum_balance(spl_token::state::Account::LEN)
        .max(1)
        .saturating_sub(associated_token_account_info.lamports());

    if required_lamports > 0 {
        invoke(
            &system_instruction::transfer(
                &funder_info.key,
                associated_token_account_info.key,
                required_lamports,
            ),
            &[
                funder_info.clone(),
                associated_token_account_info.clone(),
                system_program_info.clone(),
            ],
        )?;
    }

    // Allocate space for the associated token account
    invoke_signed(
        &system_instruction::allocate(
            associated_token_account_info.key,
            spl_token::state::Account::LEN as u64,
        ),
        &[
            associated_token_account_info.clone(),
            system_program_info.clone(),
        ],
        &[&associated_token_account_signer_seeds],
    )?;

    // Assign the associated token account to the SPL Token program
    invoke_signed(
        &system_instruction::assign(associated_token_account_info.key, &spl_token::id()),
        &[
            associated_token_account_info.clone(),
            system_program_info.clone(),
        ],
        &[&associated_token_account_signer_seeds],
    )?;

    // Initialize the associated token account
    invoke(
        &spl_token::instruction::initialize_account(
            &spl_token::id(),
            associated_token_account_info.key,
            spl_token_mint_info.key,
            wallet_account_info.key,
        )?,
        &[
            associated_token_account_info.clone(),
            spl_token_mint_info.clone(),
            wallet_account_info.clone(),
            rent_sysvar_info.clone(),
            spl_token_program_info.clone(),
        ],
    )
}
