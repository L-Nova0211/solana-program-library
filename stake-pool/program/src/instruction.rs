//! Instruction types

#![allow(clippy::too_many_arguments)]

use {
    crate::stake_program,
    borsh::{BorshDeserialize, BorshSchema, BorshSerialize},
    solana_program::{
        instruction::{AccountMeta, Instruction},
        program_error::ProgramError,
        pubkey::Pubkey,
        system_program, sysvar,
    },
};

/// Fee rate as a ratio, minted on `UpdateStakePoolBalance` as a proportion of
/// the rewards
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
    ///   1. `[s]` Manager
    ///   2. `[]` Staker
    ///   3. `[w]` Uninitialized validator stake list storage account
    ///   4. `[]` Reserve stake account must be initialized, have zero balance,
    ///       and staker / withdrawer authority set to pool withdraw authority.
    ///   5. `[]` Pool token mint. Must have zero supply, owned by withdraw authority.
    ///   6. `[]` Pool account to deposit the generated fee for manager.
    ///   7. `[]` Clock sysvar
    ///   8. `[]` Rent sysvar
    ///   9. `[]` Token program id
    Initialize {
        /// Fee assessed as percentage of perceived rewards
        #[allow(dead_code)] // but it's not
        fee: Fee,
        /// Maximum expected number of validators
        #[allow(dead_code)] // but it's not
        max_validators: u32,
    },

    ///   (Staker only) Creates new program account for accumulating stakes for
    ///   a particular validator
    ///
    ///   0. `[]` Stake pool account this stake will belong to
    ///   1. `[s]` Staker
    ///   2. `[ws]` Funding account (must be a system account)
    ///   3. `[w]` Stake account to be created
    ///   4. `[]` Validator this stake account will vote for
    ///   5. `[]` Rent sysvar
    ///   6. `[]` System program
    ///   7. `[]` Stake program
    CreateValidatorStakeAccount,

    ///   (Staker only) Adds stake account delegated to validator to the pool's
    ///   list of managed validators
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[s]` Staker
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

    ///   (Staker only) Removes validator from the pool
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[s]` Staker
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

    /// (Staker only) Decrease active stake on a validator, eventually moving it to the reserve
    ///
    /// Internally, this instruction splits a validator stake account into its
    /// corresponding transient stake account and deactivates it.
    ///
    /// In order to rebalance the pool without taking custody, the staker needs
    /// a way of reducing the stake on a stake account. This instruction splits
    /// some amount of stake, up to the total activated stake, from the canonical
    /// validator stake account, into its "transient" stake account, defined by:
    ///
    /// ```ignore
    /// Pubkey::find_program_address(
    ///     &[&stake_account_address.to_bytes()[..32],], program_id,
    /// )
    /// ```
    ///
    /// The instruction only succeeds if the transient stake account does not
    /// exist. The amount of lamports to move must be at least rent-exemption
    /// plus 1 lamport.
    ///
    ///  0. `[]` Stake pool
    ///  1. `[s]` Stake pool staker
    ///  2. `[]` Validator list
    ///  3. `[]` Stake pool withdraw authority
    ///  5. `[w]` Canonical stake account to split from
    ///  5. `[w]` Transient stake account to receive split
    ///  6. `[]` Clock sysvar
    ///  7. `[]` Rent sysvar
    ///  8. `[]` System program
    ///  9. `[]` Stake program
    ///  userdata: amount of lamports to split
    DecreaseValidatorStake(u64),

    /// (Staker only) Increase stake on a validator from the reserve account
    ///
    /// Internally, this instruction splits reserve stake into a transient stake
    /// account and delegate to the appropriate validator. `UpdateValidatorListBalance`
    /// will do the work of merging once it's ready.
    ///
    /// This instruction only succeeds if the transient stake account does not exist.
    /// The minimum amount to move is rent-exemption plus 1 SOL in order to avoid
    /// issues on credits observed when merging active stakes later.
    ///
    ///  0. `[]` Stake pool
    ///  1. `[s]` Stake pool staker
    ///  2. `[]` Validator list
    ///  3. `[]` Stake pool withdraw authority
    ///  4. `[w]` Stake pool reserve stake
    ///  5. `[w]` Transient stake account
    ///  6. `[]` Canonical stake account
    ///  7. '[]' Clock sysvar
    ///  8. `[]` Stake program
    IncreaseValidatorStake(u64),

    ///  Updates balances of validator and transient stake accounts in the pool
    ///
    ///  While going through the pairs of validator and transient stake accounts,
    ///  if the transient stake is inactive, it is merged into the reserve stake
    ///  account.  If the transient stake is active and has matching credits
    ///  observed, it is merged into the canonical validator stake account. In
    ///  all other states, nothing is done, and the balance is simply added to
    ///  the canonical stake account balance.
    ///
    ///  0. `[]` Stake pool
    ///  1. `[w]` Validator stake list storage account
    ///  2. `[w]` Reserve stake account
    ///  3. `[]` Stake pool withdraw authority
    ///  4. `[]` Sysvar clock account
    ///  5. `[]` Stake program
    ///  6. ..6+N ` [] N pairs of validator and transient stake accounts
    UpdateValidatorListBalance,

    ///   Updates total pool balance based on balances in the reserve and validator list
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[]` Validator stake list storage account
    ///   2. `[]` Reserve stake account
    ///   3. `[]` Stake pool withdraw authority
    ///   4. `[w]` Account to receive pool fee tokens
    ///   5. `[w]` Pool mint account
    ///   6. `[]` Sysvar clock account
    ///   7. `[]` Pool token program
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
    ///   8. `[w]` Pool token mint account
    ///   9. '[]' Sysvar clock account (required)
    ///   10. '[]' Sysvar stake history account
    ///   11. `[]` Pool token program id,
    ///   12. `[]` Stake program id,
    Deposit,

    ///   Withdraw the token from the pool at the current ratio.
    ///   The amount withdrawn is the MIN(u64, stake size)
    ///
    ///   A validator stake account can be withdrawn from freely, and the reserve
    ///   can only be drawn from if there is no active stake left, where all
    ///   validator accounts are left with 1 lamport.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[w]` Validator stake list storage account
    ///   2. `[]` Stake pool withdraw authority
    ///   3. `[w]` Validator or reserve stake account to split
    ///   4. `[w]` Unitialized stake account to receive withdrawal
    ///   5. `[]` User account to set as a new withdraw authority
    ///   6. `[w]` User account with pool tokens to burn from
    ///   7. `[w]` Pool token mint account
    ///   8. '[]' Sysvar clock account (required)
    ///   9. `[]` Pool token program id
    ///   10. `[]` Stake program id,
    ///   userdata: amount to withdraw
    Withdraw(u64),

    ///  (Manager only) Update manager
    ///
    ///  0. `[w]` StakePool
    ///  1. `[s]` Manager
    ///  2. '[]` New manager pubkey
    ///  3. '[]` New manager fee account
    SetManager,

    ///  (Manager or staker only) Update staker
    ///
    ///  0. `[w]` StakePool
    ///  1. `[s]` Manager or current staker
    ///  2. '[]` New staker pubkey
    SetStaker,
}

/// Creates an 'initialize' instruction.
pub fn initialize(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    staker: &Pubkey,
    validator_list: &Pubkey,
    pool_mint: &Pubkey,
    manager_pool_account: &Pubkey,
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
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*staker, false),
        AccountMeta::new(*validator_list, false),
        AccountMeta::new_readonly(*pool_mint, false),
        AccountMeta::new_readonly(*manager_pool_account, false),
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
    staker: &Pubkey,
    funder: &Pubkey,
    stake_account: &Pubkey,
    validator: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*staker, true),
        AccountMeta::new(*funder, true),
        AccountMeta::new(*stake_account, false),
        AccountMeta::new_readonly(*validator, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(stake_program::config_id(), false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(stake_program::id(), false),
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
    staker: &Pubkey,
    stake_pool_deposit: &Pubkey,
    stake_pool_withdraw: &Pubkey,
    validator_list: &Pubkey,
    stake_account: &Pubkey,
    pool_token_receiver: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*staker, true),
        AccountMeta::new_readonly(*stake_pool_deposit, false),
        AccountMeta::new_readonly(*stake_pool_withdraw, false),
        AccountMeta::new(*validator_list, false),
        AccountMeta::new(*stake_account, false),
        AccountMeta::new(*pool_token_receiver, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(stake_program::id(), false),
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
    staker: &Pubkey,
    stake_pool_withdraw: &Pubkey,
    new_stake_authority: &Pubkey,
    validator_list: &Pubkey,
    stake_account: &Pubkey,
    burn_from: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*staker, true),
        AccountMeta::new_readonly(*stake_pool_withdraw, false),
        AccountMeta::new_readonly(*new_stake_authority, false),
        AccountMeta::new(*validator_list, false),
        AccountMeta::new(*stake_account, false),
        AccountMeta::new(*burn_from, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(stake_program::id(), false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::RemoveValidatorFromPool.try_to_vec()?,
    })
}

/// Creates `DecreaseValidatorStake` instruction (rebalance from validator account to
/// transient account)
pub fn decrease_validator_stake() -> Result<Instruction, ProgramError> {
    Err(ProgramError::IncorrectProgramId)
}

/// Creates `IncreaseValidatorStake` instruction (rebalance from reserve account to
/// transient account)
pub fn increase_validator_stake() -> Result<Instruction, ProgramError> {
    Err(ProgramError::IncorrectProgramId)
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
    withdraw_authority: &Pubkey,
    manager_fee_account: &Pubkey,
    stake_pool_mint: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(*withdraw_authority, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*stake_pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
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
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(*stake_pool_deposit, false),
        AccountMeta::new_readonly(*stake_pool_withdraw, false),
        AccountMeta::new(*stake_to_join, false),
        AccountMeta::new(*validator_stake_accont, false),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(stake_program::id(), false),
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
        AccountMeta::new_readonly(stake_program::id(), false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::Withdraw(amount).try_to_vec()?,
    })
}

/// Creates a 'set manager' instruction.
pub fn set_manager(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    new_manager: &Pubkey,
    new_fee_receiver: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*new_manager, false),
        AccountMeta::new_readonly(*new_fee_receiver, false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::SetManager.try_to_vec()?,
    })
}

/// Creates a 'set staker' instruction.
pub fn set_staker(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    set_staker_authority: &Pubkey,
    new_staker: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*set_staker_authority, true),
        AccountMeta::new_readonly(*new_staker, false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::SetStaker.try_to_vec()?,
    })
}
