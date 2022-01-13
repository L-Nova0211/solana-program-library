use {
    crate::{
        check_program_account,
        error::TokenError,
        extension::{
            confidential_transfer::{instruction::*, *},
            StateWithExtensions, StateWithExtensionsMut,
        },
        processor::Processor,
        state::{Account, Mint},
    },
    solana_program::{
        account_info::{next_account_info, AccountInfo},
        entrypoint::ProgramResult,
        instruction::Instruction,
        msg,
        program_error::ProgramError,
        pubkey::Pubkey,
        sysvar::instructions::get_instruction_relative,
    },
    solana_zk_token_sdk::{
        zk_token_elgamal::{ops, pod},
        zk_token_proof_program,
    },
};

fn decode_proof_instruction<T: Pod>(
    expected: ProofInstruction,
    instruction: &Instruction,
) -> Result<&T, ProgramError> {
    if instruction.program_id != zk_token_proof_program::id()
        || ProofInstruction::decode_type(&instruction.data) != Some(expected)
    {
        msg!("Unexpected proof instruction");
        return Err(ProgramError::InvalidInstructionData);
    }

    ProofInstruction::decode_data(&instruction.data).ok_or(ProgramError::InvalidInstructionData)
}

/// Processes an [InitializeMint] instruction.
fn process_initialize_mint(
    accounts: &[AccountInfo],
    ct_mint: &ConfidentialTransferMint,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let mint_info = next_account_info(account_info_iter)?;

    check_program_account(mint_info.owner)?;
    let mint_data = &mut mint_info.data.borrow_mut();
    let mut mint = StateWithExtensionsMut::<Mint>::unpack(mint_data)?;
    *mint.init_extension::<ConfidentialTransferMint>()? = *ct_mint;

    Ok(())
}

/// Processes an [UpdateMint] instruction.
fn process_update_mint(
    accounts: &[AccountInfo],
    new_ct_mint: &ConfidentialTransferMint,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let mint_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let new_authority_info = next_account_info(account_info_iter)?;

    check_program_account(mint_info.owner)?;
    let mint_data = &mut mint_info.data.borrow_mut();
    let mut mint = StateWithExtensionsMut::<Mint>::unpack(mint_data)?;

    if authority_info.is_signer
        && (new_authority_info.is_signer || *new_authority_info.key == Pubkey::default())
    {
        if new_ct_mint.authority == *new_authority_info.key {
            let ct_mint = mint.get_extension_mut::<ConfidentialTransferMint>()?;
            *ct_mint = *new_ct_mint;
            Ok(())
        } else {
            Err(ProgramError::InvalidInstructionData)
        }
    } else {
        Err(ProgramError::MissingRequiredSignature)
    }
}

/// Processes a [ConfigureAccount] instruction.
fn process_configure_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ConfigureAccountInstructionData {
        elgamal_pk,
        decryptable_zero_balance,
    }: &ConfigureAccountInstructionData,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let token_account_info = next_account_info(account_info_iter)?;
    let mint_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    check_program_account(token_account_info.owner)?;
    let token_account_data = &mut token_account_info.data.borrow_mut();
    let mut token_account = StateWithExtensionsMut::<Account>::unpack(token_account_data)?;

    if token_account.base.mint != *mint_info.key {
        return Err(TokenError::MintMismatch.into());
    }

    Processor::validate_owner(
        program_id,
        token_account_info.key,
        token_account_info.owner,
        authority_info,
        account_info_iter.as_slice(),
    )?;

    check_program_account(mint_info.owner)?;
    let mint_data = &mut mint_info.data.borrow();
    let mint = StateWithExtensions::<Mint>::unpack(mint_data)?;
    let ct_mint = mint.get_extension::<ConfidentialTransferMint>()?;

    // TODO: Support reallocating the token account (and re-checking rent) if there's insufficient
    // room for the new extension.
    let mut ct_token_account = token_account.init_extension::<ConfidentialTransferAccount>()?;
    ct_token_account.approved = ct_mint.auto_approve_new_accounts;
    ct_token_account.elgamal_pk = *elgamal_pk;
    ct_token_account.decryptable_available_balance = *decryptable_zero_balance;

    /*
        An ElGamal ciphertext is of the form
          ElGamalCiphertext {
            msg_comm: r * H + x * G
            decrypt_handle: r * P
          }

        where
        - G, H: constants for the system (RistrettoPoint)
        - P: ElGamal public key component (RistrettoPoint)
        - r: encryption randomness (Scalar)
        - x: message (Scalar)

        Upon receiving a `ConfigureAccount` instruction, the ZK Token program should encrypt x=0 (i.e.
        Scalar::zero()) and store it as `pending_balance` and `available_balance`.

        For regular encryption, it is important that r is generated from a proper randomness source. But
        for the `ConfigureAccount` instruction, it is already known that x is always 0. So r can just be
        set Scalar::zero().

        This means that the ElGamalCiphertext should simply be
          ElGamalCiphertext {
            msg_comm: 0 * H + 0 * G = 0
            decrypt_handle: 0 * P = 0
          }

        This should just be encoded as [0; 64]
    */
    ct_token_account.pending_balance = pod::ElGamalCiphertext::zeroed();
    ct_token_account.available_balance = pod::ElGamalCiphertext::zeroed();

    Ok(())
}

/// Processes an [ApproveAccount] instruction.
fn process_approve_account(accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let account_to_approve_info = next_account_info(account_info_iter)?;
    let mint_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    check_program_account(account_to_approve_info.owner)?;
    let account_to_approve_data = &mut account_to_approve_info.data.borrow_mut();
    let mut account_to_approve = StateWithExtensionsMut::<Mint>::unpack(account_to_approve_data)?;

    check_program_account(mint_info.owner)?;
    let mint_data = &mint_info.data.borrow_mut();
    let mint = StateWithExtensions::<Mint>::unpack(mint_data)?;
    let ct_mint = mint.get_extension::<ConfidentialTransferMint>()?;

    if authority_info.is_signer && *authority_info.key == ct_mint.authority {
        let mut confidential_transfer_state =
            account_to_approve.get_extension_mut::<ConfidentialTransferAccount>()?;
        confidential_transfer_state.approved = true.into();
        Ok(())
    } else {
        Err(ProgramError::MissingRequiredSignature)
    }
}

/// Processes an [EmptyAccount] instruction.
fn process_empty_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    proof_instruction_offset: i64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let token_account_info = next_account_info(account_info_iter)?;
    let instructions_sysvar_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    check_program_account(token_account_info.owner)?;
    let token_account_data = &mut token_account_info.data.borrow_mut();
    let mut token_account = StateWithExtensionsMut::<Account>::unpack(token_account_data)?;

    Processor::validate_owner(
        program_id,
        token_account_info.key,
        token_account_info.owner,
        authority_info,
        account_info_iter.as_slice(),
    )?;

    let mut ct_token_account = token_account.get_extension_mut::<ConfidentialTransferAccount>()?;

    let previous_instruction =
        get_instruction_relative(proof_instruction_offset, instructions_sysvar_info)?;
    let proof_data = decode_proof_instruction::<CloseAccountData>(
        ProofInstruction::VerifyCloseAccount,
        &previous_instruction,
    )?;

    if ct_token_account.pending_balance != pod::ElGamalCiphertext::zeroed() {
        msg!("Pending balance is not zero");
        return Err(ProgramError::InvalidAccountData);
    }

    if ct_token_account.available_balance != proof_data.balance {
        msg!("Available balance mismatch");
        return Err(ProgramError::InvalidInstructionData);
    }

    ct_token_account.approved()?;
    ct_token_account.available_balance = pod::ElGamalCiphertext::zeroed();
    ct_token_account.closable()?;

    Ok(())
}

/// Processes a [Deposit] instruction.
fn process_deposit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
    expected_decimals: u8,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let token_account_info = next_account_info(account_info_iter)?;
    let receiver_token_account_info = next_account_info(account_info_iter)?;
    let mint_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    check_program_account(mint_info.owner)?;
    let mint_data = &mint_info.data.borrow_mut();
    let mint = StateWithExtensions::<Mint>::unpack(mint_data)?;

    if expected_decimals != mint.base.decimals {
        return Err(TokenError::MintDecimalsMismatch.into());
    }

    // Process source account
    {
        check_program_account(token_account_info.owner)?;
        let token_account_data = &mut token_account_info.data.borrow_mut();
        let mut token_account = StateWithExtensionsMut::<Account>::unpack(token_account_data)?;

        Processor::validate_owner(
            program_id,
            token_account_info.key,
            token_account_info.owner,
            authority_info,
            account_info_iter.as_slice(),
        )?;

        if token_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }

        if token_account.base.mint != *mint_info.key {
            return Err(TokenError::MintMismatch.into());
        }

        // Wrapped SOL deposits are not supported because lamports cannot be vanished.
        assert!(!token_account.base.is_native());
        token_account.base.amount = token_account
            .base
            .amount
            .checked_sub(amount)
            .ok_or(TokenError::Overflow)?;

        token_account.pack_base();
    }

    //
    // Finished with the source token account at this point. Drop all references to it to avoid a
    // double borrow if the source and destination accounts are the same
    //

    // Process destination account
    {
        check_program_account(receiver_token_account_info.owner)?;
        let receiver_token_account_data = &mut receiver_token_account_info.data.borrow_mut();
        let mut receiver_token_account =
            StateWithExtensionsMut::<Account>::unpack(receiver_token_account_data)?;

        if receiver_token_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }

        if receiver_token_account.base.mint != *mint_info.key {
            return Err(TokenError::MintMismatch.into());
        }

        let mut receiver_ct_token_account =
            receiver_token_account.get_extension_mut::<ConfidentialTransferAccount>()?;
        receiver_ct_token_account.approved()?;

        if !bool::from(&receiver_ct_token_account.allow_balance_credits) {
            return Err(TokenError::ConfidentialTransferDepositsAndTransfersDisabled.into());
        }

        receiver_ct_token_account.pending_balance =
            ops::add_to(&receiver_ct_token_account.pending_balance, amount)
                .ok_or(TokenError::Overflow)?;

        receiver_ct_token_account.pending_balance_credit_counter =
            (u64::from(receiver_ct_token_account.pending_balance_credit_counter)
                .checked_add(1)
                .ok_or(TokenError::Overflow)?)
            .into();
    }

    Ok(())
}

/// Processes a [Withdraw] instruction.
fn process_withdraw(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
    expected_decimals: u8,
    new_decryptable_available_balance: pod::AeCiphertext,
    proof_instruction_offset: i64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let token_account_info = next_account_info(account_info_iter)?;
    let receiver_token_account_info = next_account_info(account_info_iter)?;
    let mint_info = next_account_info(account_info_iter)?;
    let instructions_sysvar_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    check_program_account(mint_info.owner)?;
    let mint_data = &mint_info.data.borrow_mut();
    let mint = StateWithExtensions::<Mint>::unpack(mint_data)?;

    if expected_decimals != mint.base.decimals {
        return Err(TokenError::MintDecimalsMismatch.into());
    }

    let previous_instruction =
        get_instruction_relative(proof_instruction_offset, instructions_sysvar_info)?;

    let proof_data = decode_proof_instruction::<WithdrawData>(
        ProofInstruction::VerifyWithdraw,
        &previous_instruction,
    )?;

    // Process source account
    {
        check_program_account(token_account_info.owner)?;
        let token_account_data = &mut token_account_info.data.borrow_mut();
        let mut token_account = StateWithExtensionsMut::<Account>::unpack(token_account_data)?;

        Processor::validate_owner(
            program_id,
            token_account_info.key,
            token_account_info.owner,
            authority_info,
            account_info_iter.as_slice(),
        )?;

        if token_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }

        if token_account.base.mint != *mint_info.key {
            return Err(TokenError::MintMismatch.into());
        }

        let mut ct_token_account =
            token_account.get_extension_mut::<ConfidentialTransferAccount>()?;
        ct_token_account.approved()?;

        ct_token_account.available_balance =
            ops::subtract_from(&ct_token_account.available_balance, amount)
                .ok_or(TokenError::Overflow)?;

        if ct_token_account.available_balance != proof_data.final_balance_ct {
            return Err(TokenError::ConfidentialTransferAvailableBalanceMismatch.into());
        }

        ct_token_account.decryptable_available_balance = new_decryptable_available_balance;
    }

    //
    // Finished with the source token account at this point. Drop all references to it to avoid a
    // double borrow if the source and destination accounts are the same
    //

    // Process destination account
    {
        check_program_account(receiver_token_account_info.owner)?;
        let receiver_token_account_data = &mut receiver_token_account_info.data.borrow_mut();
        let mut receiver_token_account =
            StateWithExtensionsMut::<Account>::unpack(receiver_token_account_data)?;

        if receiver_token_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }

        if receiver_token_account.base.mint != *mint_info.key {
            return Err(TokenError::MintMismatch.into());
        }

        // Wrapped SOL withdrawals are not supported because lamports cannot be apparated.
        assert!(!receiver_token_account.base.is_native());
        receiver_token_account.base.amount = receiver_token_account
            .base
            .amount
            .checked_add(amount)
            .ok_or(TokenError::Overflow)?;

        receiver_token_account.pack_base();
    }

    Ok(())
}

/// Processes an [Transfer] instruction.
fn process_transfer(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_source_decryptable_available_balance: pod::AeCiphertext,
    proof_instruction_offset: i64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let token_account_info = next_account_info(account_info_iter)?;
    let receiver_token_account_info = next_account_info(account_info_iter)?;
    let mint_info = next_account_info(account_info_iter)?;
    let instructions_sysvar_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    check_program_account(mint_info.owner)?;
    let mint_data = &mint_info.data.borrow_mut();
    let mint = StateWithExtensions::<Mint>::unpack(mint_data)?;
    let ct_mint = mint.get_extension::<ConfidentialTransferMint>()?;

    let previous_instruction =
        get_instruction_relative(proof_instruction_offset, instructions_sysvar_info)?;
    let proof_data = decode_proof_instruction::<TransferData>(
        ProofInstruction::VerifyTransfer,
        &previous_instruction,
    )?;

    if proof_data.transfer_public_keys.auditor_pk != ct_mint.auditor_pk {
        return Err(TokenError::ConfidentialTransferElGamalPubkeyMismatch.into());
    }

    // Process source account
    {
        check_program_account(token_account_info.owner)?;
        let token_account_data = &mut token_account_info.data.borrow_mut();
        let mut token_account = StateWithExtensionsMut::<Account>::unpack(token_account_data)?;

        Processor::validate_owner(
            program_id,
            token_account_info.key,
            token_account_info.owner,
            authority_info,
            account_info_iter.as_slice(),
        )?;

        if token_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }

        if token_account.base.mint != *mint_info.key {
            return Err(TokenError::MintMismatch.into());
        }

        let mut ct_token_account =
            token_account.get_extension_mut::<ConfidentialTransferAccount>()?;
        ct_token_account.approved()?;
        if proof_data.transfer_public_keys.source_pk != ct_token_account.elgamal_pk {
            return Err(TokenError::ConfidentialTransferElGamalPubkeyMismatch.into());
        }

        let new_source_available_balance = {
            // Combine commitments and handles
            let source_lo_ct = pod::ElGamalCiphertext::from((
                proof_data.encrypted_transfer_amount.amount_comm_lo,
                proof_data
                    .encrypted_transfer_amount
                    .decrypt_handles_lo
                    .source,
            ));
            let source_hi_ct = pod::ElGamalCiphertext::from((
                proof_data.encrypted_transfer_amount.amount_comm_hi,
                proof_data
                    .encrypted_transfer_amount
                    .decrypt_handles_hi
                    .source,
            ));

            ops::subtract_with_lo_hi(
                &ct_token_account.available_balance,
                &source_lo_ct,
                &source_hi_ct,
            )
            .ok_or(ProgramError::InvalidInstructionData)?
        };

        ct_token_account.available_balance = new_source_available_balance;
        ct_token_account.decryptable_available_balance = new_source_decryptable_available_balance;
    }

    //
    // Finished with the source token account at this point. Drop all references to it to avoid a
    // double borrow if the source and destination accounts are the same
    //

    // Process destination account
    {
        check_program_account(receiver_token_account_info.owner)?;
        let receiver_token_account_data = &mut receiver_token_account_info.data.borrow_mut();
        let mut receiver_token_account =
            StateWithExtensionsMut::<Account>::unpack(receiver_token_account_data)?;

        if receiver_token_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }

        if receiver_token_account.base.mint != *mint_info.key {
            return Err(TokenError::MintMismatch.into());
        }

        let mut receiver_ct_token_account =
            receiver_token_account.get_extension_mut::<ConfidentialTransferAccount>()?;
        receiver_ct_token_account.approved()?;

        if !bool::from(&receiver_ct_token_account.allow_balance_credits) {
            return Err(TokenError::ConfidentialTransferDepositsAndTransfersDisabled.into());
        }

        if proof_data.transfer_public_keys.dest_pk != receiver_ct_token_account.elgamal_pk {
            return Err(TokenError::ConfidentialTransferElGamalPubkeyMismatch.into());
        }

        let new_receiver_pending_balance = {
            let dest_lo_ct = pod::ElGamalCiphertext::from((
                proof_data.encrypted_transfer_amount.amount_comm_lo,
                proof_data.encrypted_transfer_amount.decrypt_handles_lo.dest,
            ));

            let dest_hi_ct = pod::ElGamalCiphertext::from((
                proof_data.encrypted_transfer_amount.amount_comm_hi,
                proof_data.encrypted_transfer_amount.decrypt_handles_hi.dest,
            ));

            ops::add_with_lo_hi(
                &receiver_ct_token_account.pending_balance,
                &dest_lo_ct,
                &dest_hi_ct,
            )
            .ok_or(ProgramError::InvalidInstructionData)?
        };

        let new_receiver_pending_balance_credit_counter =
            (u64::from(receiver_ct_token_account.pending_balance_credit_counter) + 1).into();

        receiver_ct_token_account.pending_balance = new_receiver_pending_balance;
        receiver_ct_token_account.pending_balance_credit_counter =
            new_receiver_pending_balance_credit_counter;
    }

    Ok(())
}

/// Processes an [ApplyPendingBalance] instruction.
fn process_apply_pending_balance(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ApplyPendingBalanceData {
        expected_pending_balance_credit_counter,
        new_decryptable_available_balance,
    }: &ApplyPendingBalanceData,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let token_account_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    check_program_account(token_account_info.owner)?;
    let token_account_data = &mut token_account_info.data.borrow_mut();
    let mut token_account = StateWithExtensionsMut::<Account>::unpack(token_account_data)?;

    Processor::validate_owner(
        program_id,
        token_account_info.key,
        token_account_info.owner,
        authority_info,
        account_info_iter.as_slice(),
    )?;

    let mut ct_token_account = token_account.get_extension_mut::<ConfidentialTransferAccount>()?;
    ct_token_account.approved()?;

    ct_token_account.available_balance = ops::add(
        &ct_token_account.available_balance,
        &ct_token_account.pending_balance,
    )
    .ok_or(ProgramError::InvalidInstructionData)?;

    ct_token_account.actual_pending_balance_credit_counter =
        ct_token_account.pending_balance_credit_counter;
    ct_token_account.expected_pending_balance_credit_counter =
        *expected_pending_balance_credit_counter;
    ct_token_account.decryptable_available_balance = *new_decryptable_available_balance;
    ct_token_account.pending_balance = pod::ElGamalCiphertext::zeroed();

    Ok(())
}

/// Processes an [DisableBalanceCredits] or [EnableBalanceCredits] instruction.
fn process_allow_balance_credits(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    allow_balance_credits: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let token_account_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    check_program_account(token_account_info.owner)?;
    let token_account_data = &mut token_account_info.data.borrow_mut();
    let mut token_account = StateWithExtensionsMut::<Account>::unpack(token_account_data)?;

    Processor::validate_owner(
        program_id,
        token_account_info.key,
        token_account_info.owner,
        authority_info,
        account_info_iter.as_slice(),
    )?;

    let mut ct_token_account = token_account.get_extension_mut::<ConfidentialTransferAccount>()?;
    ct_token_account.approved()?;
    ct_token_account.allow_balance_credits = allow_balance_credits.into();

    Ok(())
}

pub(crate) fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    check_program_account(program_id)?;

    match decode_instruction_type(input)? {
        ConfidentialTransferInstruction::InitializeMint => {
            msg!("ConfidentialTransferInstruction::InitializeMint");
            process_initialize_mint(
                accounts,
                decode_instruction_data::<ConfidentialTransferMint>(input)?,
            )
        }
        ConfidentialTransferInstruction::UpdateMint => {
            msg!("ConfidentialTransferInstruction::UpdateMint");
            process_update_mint(
                accounts,
                decode_instruction_data::<ConfidentialTransferMint>(input)?,
            )
        }
        ConfidentialTransferInstruction::ConfigureAccount => {
            msg!("ConfidentialTransferInstruction::ConfigureAccount");
            process_configure_account(
                program_id,
                accounts,
                decode_instruction_data::<ConfigureAccountInstructionData>(input)?,
            )
        }
        ConfidentialTransferInstruction::ApproveAccount => {
            msg!("ConfidentialTransferInstruction::ApproveAccount");
            process_approve_account(accounts)
        }
        ConfidentialTransferInstruction::EmptyAccount => {
            msg!("ConfidentialTransferInstruction::EmptyAccount");
            let data = decode_instruction_data::<EmptyAccountInstructionData>(input)?;
            process_empty_account(program_id, accounts, data.proof_instruction_offset as i64)
        }
        ConfidentialTransferInstruction::Deposit => {
            msg!("ConfidentialTransferInstruction::Deposit");
            let data = decode_instruction_data::<DepositInstructionData>(input)?;
            process_deposit(program_id, accounts, data.amount.into(), data.decimals)
        }
        ConfidentialTransferInstruction::Withdraw => {
            msg!("ConfidentialTransferInstruction::Withdraw");
            let data = decode_instruction_data::<WithdrawInstructionData>(input)?;
            process_withdraw(
                program_id,
                accounts,
                data.amount.into(),
                data.decimals,
                data.new_decryptable_available_balance,
                data.proof_instruction_offset as i64,
            )
        }
        ConfidentialTransferInstruction::Transfer => {
            msg!("ConfidentialTransferInstruction::Transfer");
            let data = decode_instruction_data::<TransferInstructionData>(input)?;
            process_transfer(
                program_id,
                accounts,
                data.new_source_decryptable_available_balance,
                data.proof_instruction_offset as i64,
            )
        }
        ConfidentialTransferInstruction::ApplyPendingBalance => {
            msg!("ConfidentialTransferInstruction::ApplyPendingBalance");
            process_apply_pending_balance(
                program_id,
                accounts,
                decode_instruction_data::<ApplyPendingBalanceData>(input)?,
            )
        }
        ConfidentialTransferInstruction::DisableBalanceCredits => {
            msg!("ConfidentialTransferInstruction::DisableBalanceCredits");
            process_allow_balance_credits(program_id, accounts, false)
        }
        ConfidentialTransferInstruction::EnableBalanceCredits => {
            msg!("ConfidentialTransferInstruction::EnableBalanceCredits");
            process_allow_balance_credits(program_id, accounts, true)
        }
    }
}
