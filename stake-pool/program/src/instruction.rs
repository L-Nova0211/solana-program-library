//! Instruction types

#![allow(clippy::too_many_arguments)]

use {
    borsh::{BorshDeserialize, BorshSchema, BorshSerialize},
    solana_program::{
        instruction::{AccountMeta, Instruction},
        program_error::ProgramError,
        pubkey::Pubkey,
        sysvar,
    },
};

/// Fee rate as a ratio, minted on deposit
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Fee {
    /// denominator of the fee ratio
    pub denominator: u64,
    /// numerator of the fee ratio
    pub numerator: u64,
}

/// Instructions supported by the StakePool program.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum StakePoolInstruction {
    ///   Initializes a new StakePool.
    ///
    ///   0. `[w]` New StakePool to create.
    ///   1. `[s]` Owner
    ///   2. `[w]` Uninitialized validator stake list storage account
    ///   3. `[]` pool token Mint. Must be non zero, owned by withdraw authority.
    ///   4. `[]` Pool Account to deposit the generated fee for owner.
    ///   5. `[]` Clock sysvar
    ///   6. `[]` Rent sysvar
    ///   7. `[]` Token program id
    Initialize {
        /// Deposit fee assessed
        #[allow(dead_code)] // but it's not
        fee: Fee,
        /// Maximum expected number of validators
        #[allow(dead_code)] // but it's not
        max_validators: u32,
    },

    ///   Creates new program account for accumulating stakes for a particular validator
    ///
    ///   0. `[]` Stake pool account this stake will belong to
    ///   1. `[ws]` Funding account (must be a system account)
    ///   2. `[w]` Stake account to be created
    ///   3. `[]` Validator this stake account will vote for
    ///   4. `[]` Stake authority for the new stake account
    ///   5. `[]` Withdraw authority for the new stake account
    ///   6. `[]` Rent sysvar
    ///   7. `[]` System program
    ///   8. `[]` Stake program
    CreateValidatorStakeAccount,

    ///   Adds validator stake account to the pool
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[s]` Owner
    ///   2. `[]` Stake pool deposit authority
    ///   3. `[]` Stake pool withdraw authority
    ///   4. `[w]` Validator stake list storage account
    ///   5. `[w]` Stake account to add to the pool, its withdraw authority should be set to stake pool deposit
    ///   6. `[w]` User account to receive pool tokens
    ///   7. `[w]` Pool token mint account
    ///   8. `[]` Clock sysvar (required)
    ///   9. '[]' Sysvar stake history account
    ///  10. `[]` Pool token program id,
    ///  11. `[]` Stake program id,
    AddValidatorToPool,

    ///   Removes validator stake account from the pool
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[s]` Owner
    ///   2. `[]` Stake pool withdraw authority
    ///   3. `[]` New withdraw/staker authority to set in the stake account
    ///   4. `[w]` Validator stake list storage account
    ///   5. `[w]` Stake account to remove from the pool
    ///   6. `[w]` User account with pool tokens to burn from
    ///   7. `[w]` Pool token mint account
    ///   8. '[]' Sysvar clock account (required)
    ///   9. `[]` Pool token program id
    ///  10. `[]` Stake program id,
    RemoveValidatorFromPool,

    ///   Updates balances of validator stake accounts in the pool
    ///
    ///   0. `[w]` Validator stake list storage account
    ///   1. `[]` Sysvar clock account
    ///   2. ..2+N ` [] N validator stake accounts to update balances
    UpdateValidatorListBalance,

    ///   Updates total pool balance based on balances in validator stake account list storage
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[]` Validator stake list storage account
    ///   2. `[]` Sysvar clock account
    UpdateStakePoolBalance,

    ///   Deposit some stake into the pool.  The output is a "pool" token representing ownership
    ///   into the pool. Inputs are converted to the current ratio.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[w]` Validator stake list storage account
    ///   2. `[]` Stake pool deposit authority
    ///   3. `[]` Stake pool withdraw authority
    ///   4. `[w]` Stake account to join the pool (withdraw should be set to stake pool deposit)
    ///   5. `[w]` Validator stake account for the stake account to be merged with
    ///   6. `[w]` User account to receive pool tokens
    ///   7. `[w]` Account to receive pool fee tokens
    ///   8. `[w]` Pool token mint account
    ///   9. '[]' Sysvar clock account (required)
    ///   10. '[]' Sysvar stake history account
    ///   11. `[]` Pool token program id,
    ///   12. `[]` Stake program id,
    Deposit,

    ///   Withdraw the token from the pool at the current ratio.
    ///   The amount withdrawn is the MIN(u64, stake size)
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[w]` Validator stake list storage account
    ///   2. `[]` Stake pool withdraw authority
    ///   3. `[w]` Validator stake account to split
    ///   4. `[w]` Unitialized stake account to receive withdrawal
    ///   5. `[]` User account to set as a new withdraw authority
    ///   6. `[w]` User account with pool tokens to burn from
    ///   7. `[w]` Pool token mint account
    ///   8. '[]' Sysvar clock account (required)
    ///   9. `[]` Pool token program id
    ///   10. `[]` Stake program id,
    ///   userdata: amount to withdraw
    Withdraw(u64),

    ///   Update owner
    ///
    ///   0. `[w]` StakePool
    ///   1. `[s]` Owner
    ///   2. '[]` New owner pubkey
    ///   3. '[]` New owner fee account
    SetOwner,
}

/// Creates an 'initialize' instruction.
pub fn initialize(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    owner: &Pubkey,
    validator_list: &Pubkey,
    pool_mint: &Pubkey,
    owner_pool_account: &Pubkey,
    token_program_id: &Pubkey,
    fee: Fee,
    max_validators: u32,
) -> Result<Instruction, ProgramError> {
    let init_data = StakePoolInstruction::Initialize {
        fee,
        max_validators,
    };
    let data = init_data.try_to_vec()?;
    let accounts = vec![
        AccountMeta::new(*stake_pool, true),
        AccountMeta::new_readonly(*owner, true),
        AccountMeta::new(*validator_list, false),
        AccountMeta::new_readonly(*pool_mint, false),
        AccountMeta::new_readonly(*owner_pool_account, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates `CreateValidatorStakeAccount` instruction (create new stake account for the validator)
pub fn create_validator_stake_account(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    funder: &Pubkey,
    stake_account: &Pubkey,
    validator: &Pubkey,
    stake_authority: &Pubkey,
    withdraw_authority: &Pubkey,
    system_program_id: &Pubkey,
    stake_program_id: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new(*funder, true),
        AccountMeta::new(*stake_account, false),
        AccountMeta::new_readonly(*validator, false),
        AccountMeta::new_readonly(*stake_authority, false),
        AccountMeta::new_readonly(*withdraw_authority, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(*system_program_id, false),
        AccountMeta::new_readonly(*stake_program_id, false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::CreateValidatorStakeAccount.try_to_vec()?,
    })
}

/// Creates `AddValidatorToPool` instruction (add new validator stake account to the pool)
pub fn add_validator_to_pool(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    owner: &Pubkey,
    stake_pool_deposit: &Pubkey,
    stake_pool_withdraw: &Pubkey,
    validator_list: &Pubkey,
    stake_account: &Pubkey,
    pool_tokens_to: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    stake_program_id: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*owner, true),
        AccountMeta::new_readonly(*stake_pool_deposit, false),
        AccountMeta::new_readonly(*stake_pool_withdraw, false),
        AccountMeta::new(*validator_list, false),
        AccountMeta::new(*stake_account, false),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(*stake_program_id, false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::AddValidatorToPool.try_to_vec()?,
    })
}

/// Creates `RemoveValidatorFromPool` instruction (remove validator stake account from the pool)
pub fn remove_validator_from_pool(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    owner: &Pubkey,
    stake_pool_withdraw: &Pubkey,
    new_stake_authority: &Pubkey,
    validator_list: &Pubkey,
    stake_account: &Pubkey,
    burn_from: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    stake_program_id: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*owner, true),
        AccountMeta::new_readonly(*stake_pool_withdraw, false),
        AccountMeta::new_readonly(*new_stake_authority, false),
        AccountMeta::new(*validator_list, false),
        AccountMeta::new(*stake_account, false),
        AccountMeta::new(*burn_from, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(*stake_program_id, false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::RemoveValidatorFromPool.try_to_vec()?,
    })
}

/// Creates `UpdateValidatorListBalance` instruction (update validator stake account balances)
pub fn update_validator_list_balance(
    program_id: &Pubkey,
    validator_list_storage: &Pubkey,
    validator_list: &[Pubkey],
) -> Result<Instruction, ProgramError> {
    let mut accounts: Vec<AccountMeta> = validator_list
        .iter()
        .map(|pubkey| AccountMeta::new_readonly(*pubkey, false))
        .collect();
    accounts.insert(0, AccountMeta::new(*validator_list_storage, false));
    accounts.insert(1, AccountMeta::new_readonly(sysvar::clock::id(), false));
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::UpdateValidatorListBalance.try_to_vec()?,
    })
}

/// Creates `UpdateStakePoolBalance` instruction (pool balance from the stake account list balances)
pub fn update_stake_pool_balance(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    validator_list_storage: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::UpdateStakePoolBalance.try_to_vec()?,
    })
}

/// Creates a 'Deposit' instruction.
pub fn deposit(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    validator_list_storage: &Pubkey,
    stake_pool_deposit: &Pubkey,
    stake_pool_withdraw: &Pubkey,
    stake_to_join: &Pubkey,
    validator_stake_accont: &Pubkey,
    pool_tokens_to: &Pubkey,
    pool_fee_to: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    stake_program_id: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(*stake_pool_deposit, false),
        AccountMeta::new_readonly(*stake_pool_withdraw, false),
        AccountMeta::new(*stake_to_join, false),
        AccountMeta::new(*validator_stake_accont, false),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new(*pool_fee_to, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(*stake_program_id, false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::Deposit.try_to_vec()?,
    })
}

/// Creates a 'withdraw' instruction.
pub fn withdraw(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    validator_list_storage: &Pubkey,
    stake_pool_withdraw: &Pubkey,
    stake_to_split: &Pubkey,
    stake_to_receive: &Pubkey,
    user_withdrawer: &Pubkey,
    burn_from: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    stake_program_id: &Pubkey,
    amount: u64,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(*stake_pool_withdraw, false),
        AccountMeta::new(*stake_to_split, false),
        AccountMeta::new(*stake_to_receive, false),
        AccountMeta::new_readonly(*user_withdrawer, false),
        AccountMeta::new(*burn_from, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(*stake_program_id, false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::Withdraw(amount).try_to_vec()?,
    })
}

/// Creates a 'set owner' instruction.
pub fn set_owner(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    stake_pool_owner: &Pubkey,
    stake_pool_new_owner: &Pubkey,
    stake_pool_new_fee_receiver: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_owner, true),
        AccountMeta::new_readonly(*stake_pool_new_owner, false),
        AccountMeta::new_readonly(*stake_pool_new_fee_receiver, false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::SetOwner.try_to_vec()?,
    })
}
