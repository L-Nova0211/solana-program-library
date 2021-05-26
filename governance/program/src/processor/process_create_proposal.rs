//! Program state processor

use borsh::BorshSerialize;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};

use crate::{
    error::GovernanceError,
    state::{
        enums::{GovernanceAccountType, ProposalState},
        governance::deserialize_governance_raw,
        proposal::{get_proposal_address_seeds, Proposal},
        token_owner_record::deserialize_token_owner_record_for_realm_and_governing_mint,
    },
    tools::{
        account::create_and_serialize_account_signed,
        asserts::assert_token_owner_or_delegate_is_signer,
    },
};

/// Processes CreateProposal instruction
pub fn process_create_proposal(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    name: String,
    description_link: String,
    governing_token_mint: Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let proposal_info = next_account_info(account_info_iter)?; // 0
    let governance_info = next_account_info(account_info_iter)?; // 1

    let token_owner_record_info = next_account_info(account_info_iter)?; // 2
    let governance_authority_info = next_account_info(account_info_iter)?; // 3

    let payer_info = next_account_info(account_info_iter)?; // 4
    let system_info = next_account_info(account_info_iter)?; // 5

    let rent_sysvar_info = next_account_info(account_info_iter)?; // 6
    let rent = &Rent::from_account_info(rent_sysvar_info)?;

    let clock_info = next_account_info(account_info_iter)?; // 7
    let clock = Clock::from_account_info(clock_info)?;

    if !proposal_info.data_is_empty() {
        return Err(GovernanceError::ProposalAlreadyExists.into());
    }

    let mut governance_data = deserialize_governance_raw(governance_info)?;

    let token_owner_record_data = deserialize_token_owner_record_for_realm_and_governing_mint(
        &token_owner_record_info,
        &governance_data.config.realm,
        &governing_token_mint,
    )?;

    // proposal_owner must be either governing token owner or governance_delegate and must sign this transaction
    assert_token_owner_or_delegate_is_signer(&token_owner_record_data, governance_authority_info)?;

    if token_owner_record_data.governing_token_deposit_amount
        < governance_data.config.min_tokens_to_create_proposal as u64
    {
        return Err(GovernanceError::NotEnoughTokensToCreateProposal.into());
    }

    let proposal_data = Proposal {
        account_type: GovernanceAccountType::Proposal,
        governance: *governance_info.key,
        governing_token_mint,
        state: ProposalState::Draft,
        token_owner_record: *token_owner_record_info.key,

        signatories_count: 0,
        signatories_signed_off_count: 0,

        name,
        description_link,

        draft_at: clock.slot,
        signing_off_at: None,
        voting_at: None,
        voting_completed_at: None,
        executing_at: None,
        closed_at: None,

        number_of_executed_instructions: 0,
        number_of_instructions: 0,
    };

    create_and_serialize_account_signed::<Proposal>(
        payer_info,
        proposal_info,
        &proposal_data,
        &get_proposal_address_seeds(
            governance_info.key,
            &governing_token_mint,
            &governance_data.proposals_count.to_le_bytes(),
        ),
        program_id,
        system_info,
        rent,
    )?;

    governance_data.proposals_count = governance_data.proposals_count.checked_add(1).unwrap();
    governance_data.serialize(&mut *governance_info.data.borrow_mut())?;

    Ok(())
}
