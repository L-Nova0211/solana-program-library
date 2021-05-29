use std::borrow::Borrow;

use borsh::BorshDeserialize;
use solana_program::{
    borsh::try_from_slice_unchecked,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    instruction::Instruction,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
};

use bincode::deserialize;

use solana_program_test::ProgramTest;
use solana_program_test::*;

use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_governance::{
    instruction::{
        add_signatory, cancel_proposal, cast_vote, create_account_governance,
        create_program_governance, create_proposal, create_realm, deposit_governing_tokens,
        finalize_vote, relinquish_vote, remove_signatory, set_governance_delegate,
        sign_off_proposal, withdraw_governing_tokens, Vote,
    },
    processor::process_instruction,
    state::{
        enums::{GovernanceAccountType, ProposalState, VoteWeight},
        governance::{
            get_account_governance_address, get_program_governance_address, Governance,
            GovernanceConfig,
        },
        proposal::{get_proposal_address, Proposal},
        realm::{get_governing_token_holding_address, get_realm_address, Realm},
        signatory_record::{get_signatory_record_address, SignatoryRecord},
        token_owner_record::{get_token_owner_record_address, TokenOwnerRecord},
        vote_record::{get_vote_record_address, VoteRecord},
    },
    tools::bpf_loader_upgradeable::get_program_data_address,
};

pub mod cookies;
use crate::program_test::{cookies::SignatoryRecordCookie, tools::clone_keypair};

use self::{
    cookies::{
        GovernanceCookie, GovernedAccountCookie, GovernedProgramCookie, ProposalCookie,
        RealmCookie, TokeOwnerRecordCookie, VoteRecordCookie,
    },
    tools::NopOverride,
};

pub mod tools;
use self::tools::map_transaction_error;

pub struct GovernanceProgramTest {
    pub context: ProgramTestContext,
    pub rent: Rent,
    pub next_realm_id: u8,
}

impl GovernanceProgramTest {
    pub async fn start_new() -> Self {
        let program_test = ProgramTest::new(
            "spl_governance",
            spl_governance::id(),
            processor!(process_instruction),
        );

        let mut context = program_test.start_with_context().await;
        let rent = context.banks_client.get_rent().await.unwrap();

        Self {
            context,
            rent,
            next_realm_id: 0,
        }
    }

    pub async fn process_transaction(
        &mut self,
        instructions: &[Instruction],
        signers: Option<&[&Keypair]>,
    ) -> Result<(), ProgramError> {
        let mut transaction =
            Transaction::new_with_payer(&instructions, Some(&self.context.payer.pubkey()));

        let mut all_signers = vec![&self.context.payer];

        if let Some(signers) = signers {
            all_signers.extend_from_slice(signers);
        }

        let recent_blockhash = self
            .context
            .banks_client
            .get_recent_blockhash()
            .await
            .unwrap();

        transaction.sign(&all_signers, recent_blockhash);

        self.context
            .banks_client
            .process_transaction(transaction)
            .await
            .map_err(map_transaction_error)?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn with_realm(&mut self) -> RealmCookie {
        let name = format!("Realm #{}", self.next_realm_id).to_string();
        self.next_realm_id = self.next_realm_id + 1;

        let realm_address = get_realm_address(&name);

        let community_token_mint_keypair = Keypair::new();
        let community_token_mint_authority = Keypair::new();

        let community_token_holding_address = get_governing_token_holding_address(
            &realm_address,
            &community_token_mint_keypair.pubkey(),
        );

        self.create_mint(
            &community_token_mint_keypair,
            &community_token_mint_authority.pubkey(),
        )
        .await;

        let council_token_mint_keypair = Keypair::new();
        let council_token_mint_authority = Keypair::new();

        let council_token_holding_address = get_governing_token_holding_address(
            &realm_address,
            &council_token_mint_keypair.pubkey(),
        );

        self.create_mint(
            &council_token_mint_keypair,
            &council_token_mint_authority.pubkey(),
        )
        .await;

        let create_realm_instruction = create_realm(
            &community_token_mint_keypair.pubkey(),
            &self.context.payer.pubkey(),
            Some(council_token_mint_keypair.pubkey()),
            name.clone(),
        );

        self.process_transaction(&[create_realm_instruction], None)
            .await
            .unwrap();

        let account = Realm {
            account_type: GovernanceAccountType::Realm,
            community_mint: community_token_mint_keypair.pubkey(),
            council_mint: Some(council_token_mint_keypair.pubkey()),
            name,
        };

        RealmCookie {
            address: realm_address,
            account,

            community_mint_authority: community_token_mint_authority,
            community_token_holding_account: community_token_holding_address,

            council_token_holding_account: Some(council_token_holding_address),
            council_mint_authority: Some(council_token_mint_authority),
        }
    }

    #[allow(dead_code)]
    pub async fn with_realm_using_mints(&mut self, realm_cookie: &RealmCookie) -> RealmCookie {
        let name = format!("Realm #{}", self.next_realm_id).to_string();
        self.next_realm_id = self.next_realm_id + 1;

        let realm_address = get_realm_address(&name);
        let council_mint = realm_cookie.account.council_mint.unwrap();

        let create_realm_instruction = create_realm(
            &realm_cookie.account.community_mint,
            &self.context.payer.pubkey(),
            Some(council_mint),
            name.clone(),
        );

        self.process_transaction(&[create_realm_instruction], None)
            .await
            .unwrap();

        let account = Realm {
            account_type: GovernanceAccountType::Realm,
            community_mint: realm_cookie.account.community_mint,
            council_mint: Some(council_mint),
            name,
        };

        let community_token_holding_address = get_governing_token_holding_address(
            &realm_address,
            &realm_cookie.account.community_mint,
        );

        let council_token_holding_address =
            get_governing_token_holding_address(&realm_address, &council_mint);

        RealmCookie {
            address: realm_address,
            account,

            community_mint_authority: clone_keypair(&realm_cookie.community_mint_authority),
            community_token_holding_account: community_token_holding_address,

            council_token_holding_account: Some(council_token_holding_address),
            council_mint_authority: Some(clone_keypair(
                &realm_cookie.council_mint_authority.as_ref().unwrap(),
            )),
        }
    }

    #[allow(dead_code)]
    pub async fn with_initial_community_token_deposit(
        &mut self,
        realm_cookie: &RealmCookie,
    ) -> TokeOwnerRecordCookie {
        self.with_initial_governing_token_deposit(
            &realm_cookie.address,
            &realm_cookie.account.community_mint,
            &realm_cookie.community_mint_authority,
            100,
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn with_initial_community_token_deposit_amount(
        &mut self,
        realm_cookie: &RealmCookie,
        amount: u64,
    ) -> TokeOwnerRecordCookie {
        self.with_initial_governing_token_deposit(
            &realm_cookie.address,
            &realm_cookie.account.community_mint,
            &realm_cookie.community_mint_authority,
            amount,
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn with_community_token_deposit(
        &mut self,
        realm_cookie: &RealmCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
        amount: u64,
    ) {
        self.with_governing_token_deposit(
            &realm_cookie.address,
            &realm_cookie.account.community_mint,
            &realm_cookie.community_mint_authority,
            token_owner_record_cookie,
            amount,
        )
        .await;
    }

    #[allow(dead_code)]
    pub async fn with_council_token_deposit(
        &mut self,
        realm_cookie: &RealmCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
        amount: u64,
    ) {
        self.with_governing_token_deposit(
            &realm_cookie.address,
            &realm_cookie.account.council_mint.unwrap(),
            &realm_cookie.council_mint_authority.as_ref().unwrap(),
            token_owner_record_cookie,
            amount,
        )
        .await;
    }

    #[allow(dead_code)]
    pub async fn with_initial_council_token_deposit(
        &mut self,
        realm_cookie: &RealmCookie,
    ) -> TokeOwnerRecordCookie {
        self.with_initial_governing_token_deposit(
            &realm_cookie.address,
            &realm_cookie.account.council_mint.unwrap(),
            &realm_cookie.council_mint_authority.as_ref().unwrap(),
            100,
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn with_initial_governing_token_deposit(
        &mut self,
        realm_address: &Pubkey,
        governing_mint: &Pubkey,
        governing_mint_authority: &Keypair,
        amount: u64,
    ) -> TokeOwnerRecordCookie {
        let token_owner = Keypair::new();
        let token_source = Keypair::new();

        let transfer_authority = Keypair::new();

        self.create_token_account_with_transfer_authority(
            &token_source,
            governing_mint,
            governing_mint_authority,
            amount,
            &token_owner,
            &transfer_authority.pubkey(),
        )
        .await;

        let deposit_governing_tokens_instruction = deposit_governing_tokens(
            realm_address,
            &token_source.pubkey(),
            &token_owner.pubkey(),
            &token_owner.pubkey(),
            &self.context.payer.pubkey(),
            governing_mint,
        );

        self.process_transaction(
            &[deposit_governing_tokens_instruction],
            Some(&[&token_owner]),
        )
        .await
        .unwrap();

        let token_owner_record_address =
            get_token_owner_record_address(realm_address, &governing_mint, &token_owner.pubkey());

        let account = TokenOwnerRecord {
            account_type: GovernanceAccountType::TokenOwnerRecord,
            realm: *realm_address,
            governing_token_mint: *governing_mint,
            governing_token_owner: token_owner.pubkey(),
            governing_token_deposit_amount: amount,
            governance_delegate: None,
            unrelinquished_votes_count: 0,
            total_votes_count: 0,
        };

        let governance_delegate = Keypair::from_base58_string(&token_owner.to_base58_string());

        TokeOwnerRecordCookie {
            address: token_owner_record_address,
            account,

            token_source_amount: amount,
            token_source: token_source.pubkey(),
            token_owner,
            governance_authority: None,
            governance_delegate: governance_delegate,
        }
    }

    #[allow(dead_code)]
    pub async fn mint_community_tokens(&mut self, realm_cookie: &RealmCookie, amount: u64) {
        let token_account_keypair = Keypair::new();

        self.create_empty_token_account(
            &token_account_keypair,
            &realm_cookie.account.community_mint,
            &self.context.payer.pubkey(),
        )
        .await;

        self.mint_tokens(
            &realm_cookie.account.community_mint,
            &realm_cookie.community_mint_authority,
            &token_account_keypair.pubkey(),
            amount,
        )
        .await;
    }

    #[allow(dead_code)]
    async fn with_governing_token_deposit(
        &mut self,
        realm: &Pubkey,
        governing_token_mint: &Pubkey,
        governing_token_mint_authority: &Keypair,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
        amount: u64,
    ) {
        self.mint_tokens(
            governing_token_mint,
            governing_token_mint_authority,
            &token_owner_record_cookie.token_source,
            amount,
        )
        .await;

        let deposit_governing_tokens_instruction = deposit_governing_tokens(
            realm,
            &token_owner_record_cookie.token_source,
            &token_owner_record_cookie.token_owner.pubkey(),
            &token_owner_record_cookie.token_owner.pubkey(),
            &self.context.payer.pubkey(),
            governing_token_mint,
        );

        self.process_transaction(
            &[deposit_governing_tokens_instruction],
            Some(&[&token_owner_record_cookie.token_owner]),
        )
        .await
        .unwrap();
    }

    #[allow(dead_code)]
    pub async fn with_community_governance_delegate(
        &mut self,
        realm_cookie: &RealmCookie,
        token_owner_record_cookie: &mut TokeOwnerRecordCookie,
    ) {
        self.with_governing_token_governance_delegate(
            &realm_cookie,
            &realm_cookie.account.community_mint,
            token_owner_record_cookie,
        )
        .await;
    }

    #[allow(dead_code)]
    pub async fn with_council_governance_delegate(
        &mut self,
        realm_cookie: &RealmCookie,
        token_owner_record_cookie: &mut TokeOwnerRecordCookie,
    ) {
        self.with_governing_token_governance_delegate(
            &realm_cookie,
            &realm_cookie.account.council_mint.unwrap(),
            token_owner_record_cookie,
        )
        .await;
    }

    #[allow(dead_code)]
    pub async fn with_governing_token_governance_delegate(
        &mut self,
        realm_cookie: &RealmCookie,
        governing_token_mint: &Pubkey,
        token_owner_record_cookie: &mut TokeOwnerRecordCookie,
    ) {
        let new_governance_delegate = Keypair::new();

        self.set_governance_delegate(
            realm_cookie,
            token_owner_record_cookie,
            &token_owner_record_cookie.token_owner,
            governing_token_mint,
            &Some(new_governance_delegate.pubkey()),
        )
        .await;

        token_owner_record_cookie.governance_delegate = new_governance_delegate;
    }

    #[allow(dead_code)]
    pub async fn set_governance_delegate(
        &mut self,
        realm_cookie: &RealmCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
        signing_governance_authority: &Keypair,
        governing_token_mint: &Pubkey,
        new_governance_delegate: &Option<Pubkey>,
    ) {
        let set_governance_delegate_instruction = set_governance_delegate(
            &signing_governance_authority.pubkey(),
            &realm_cookie.address,
            governing_token_mint,
            &token_owner_record_cookie.token_owner.pubkey(),
            new_governance_delegate,
        );

        self.process_transaction(
            &[set_governance_delegate_instruction],
            Some(&[&signing_governance_authority]),
        )
        .await
        .unwrap();
    }

    #[allow(dead_code)]
    pub async fn withdraw_community_tokens(
        &mut self,
        realm_cookie: &RealmCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
    ) -> Result<(), ProgramError> {
        self.withdraw_governing_tokens(
            realm_cookie,
            token_owner_record_cookie,
            &realm_cookie.account.community_mint,
            &token_owner_record_cookie.token_owner,
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn withdraw_council_tokens(
        &mut self,
        realm_cookie: &RealmCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
    ) -> Result<(), ProgramError> {
        self.withdraw_governing_tokens(
            realm_cookie,
            token_owner_record_cookie,
            &realm_cookie.account.council_mint.unwrap(),
            &token_owner_record_cookie.token_owner,
        )
        .await
    }

    #[allow(dead_code)]
    async fn withdraw_governing_tokens(
        &mut self,
        realm_cookie: &RealmCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
        governing_token_mint: &Pubkey,

        governing_token_owner: &Keypair,
    ) -> Result<(), ProgramError> {
        let deposit_governing_tokens_instruction = withdraw_governing_tokens(
            &realm_cookie.address,
            &token_owner_record_cookie.token_source,
            &governing_token_owner.pubkey(),
            governing_token_mint,
        );

        self.process_transaction(
            &[deposit_governing_tokens_instruction],
            Some(&[&governing_token_owner]),
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn with_governed_account(&mut self) -> GovernedAccountCookie {
        GovernedAccountCookie {
            address: Pubkey::new_unique(),
        }
    }

    pub fn get_default_governance_config(
        &mut self,
        realm_cookie: &RealmCookie,
        governed_account_cookie: &GovernedAccountCookie,
    ) -> GovernanceConfig {
        GovernanceConfig {
            realm: realm_cookie.address,
            governed_account: governed_account_cookie.address,
            yes_vote_threshold_percentage: 60,
            min_tokens_to_create_proposal: 5,
            min_instruction_hold_up_time: 10,
            max_voting_time: 10,
        }
    }

    #[allow(dead_code)]
    pub async fn with_account_governance(
        &mut self,
        realm_cookie: &RealmCookie,
        governed_account_cookie: &GovernedAccountCookie,
    ) -> Result<GovernanceCookie, ProgramError> {
        let config = self.get_default_governance_config(realm_cookie, governed_account_cookie);
        self.with_account_governance_using_config(realm_cookie, governed_account_cookie, &config)
            .await
    }

    #[allow(dead_code)]
    pub async fn with_account_governance_using_config(
        &mut self,
        realm_cookie: &RealmCookie,
        governed_account_cookie: &GovernedAccountCookie,
        governance_config: &GovernanceConfig,
    ) -> Result<GovernanceCookie, ProgramError> {
        let create_account_governance_instruction =
            create_account_governance(&self.context.payer.pubkey(), governance_config.clone());

        let account = Governance {
            account_type: GovernanceAccountType::AccountGovernance,
            config: governance_config.clone(),
            proposals_count: 0,
        };

        self.process_transaction(&[create_account_governance_instruction], None)
            .await?;

        let account_governance_address =
            get_account_governance_address(&realm_cookie.address, &governed_account_cookie.address);

        Ok(GovernanceCookie {
            address: account_governance_address,
            account,
            next_proposal_index: 0,
        })
    }

    #[allow(dead_code)]
    pub async fn with_governed_program(&mut self) -> GovernedProgramCookie {
        let program_keypair = Keypair::new();
        let program_buffer_keypair = Keypair::new();
        let program_upgrade_authority_keypair = Keypair::new();

        let program_data_address = get_program_data_address(&program_keypair.pubkey());

        // Load solana_bpf_rust_upgradeable program taken from solana test programs
        let path_buf = find_file("solana_bpf_rust_upgradeable.so").unwrap();
        let program_data = read_file(path_buf);

        let program_buffer_rent = self
            .rent
            .minimum_balance(UpgradeableLoaderState::programdata_len(program_data.len()).unwrap());

        let mut instructions = bpf_loader_upgradeable::create_buffer(
            &self.context.payer.pubkey(),
            &program_buffer_keypair.pubkey(),
            &program_upgrade_authority_keypair.pubkey(),
            program_buffer_rent,
            program_data.len(),
        )
        .unwrap();

        let chunk_size = 800;

        for (chunk, i) in program_data.chunks(chunk_size).zip(0..) {
            instructions.push(bpf_loader_upgradeable::write(
                &program_buffer_keypair.pubkey(),
                &program_upgrade_authority_keypair.pubkey(),
                (i * chunk_size) as u32,
                chunk.to_vec(),
            ));
        }

        let program_account_rent = self
            .rent
            .minimum_balance(UpgradeableLoaderState::program_len().unwrap());

        let deploy_instructions = bpf_loader_upgradeable::deploy_with_max_program_len(
            &self.context.payer.pubkey(),
            &program_keypair.pubkey(),
            &program_buffer_keypair.pubkey(),
            &program_upgrade_authority_keypair.pubkey(),
            program_account_rent,
            program_data.len(),
        )
        .unwrap();

        instructions.extend_from_slice(&deploy_instructions);

        self.process_transaction(
            &instructions[..],
            Some(&[
                &program_upgrade_authority_keypair,
                &program_keypair,
                &program_buffer_keypair,
            ]),
        )
        .await
        .unwrap();

        GovernedProgramCookie {
            address: program_keypair.pubkey(),
            upgrade_authority: program_upgrade_authority_keypair,
            data_address: program_data_address,
            transfer_upgrade_authority: true,
        }
    }

    #[allow(dead_code)]
    pub async fn with_program_governance(
        &mut self,
        realm_cookie: &RealmCookie,
        governed_program_cookie: &GovernedProgramCookie,
    ) -> Result<GovernanceCookie, ProgramError> {
        self.with_program_governance_using_instruction(
            realm_cookie,
            governed_program_cookie,
            NopOverride,
            None,
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn with_program_governance_using_instruction<F: Fn(&mut Instruction)>(
        &mut self,
        realm_cookie: &RealmCookie,
        governed_program_cookie: &GovernedProgramCookie,
        instruction_override: F,
        signers_override: Option<&[&Keypair]>,
    ) -> Result<GovernanceCookie, ProgramError> {
        let config = GovernanceConfig {
            realm: realm_cookie.address,
            governed_account: governed_program_cookie.address,
            min_tokens_to_create_proposal: 5,
            min_instruction_hold_up_time: 10,
            max_voting_time: 100,
            yes_vote_threshold_percentage: 60,
        };

        let mut create_program_governance_instruction = create_program_governance(
            &governed_program_cookie.upgrade_authority.pubkey(),
            &self.context.payer.pubkey(),
            config.clone(),
            governed_program_cookie.transfer_upgrade_authority,
        );

        instruction_override(&mut create_program_governance_instruction);

        let default_signers = &[&governed_program_cookie.upgrade_authority];
        let singers = signers_override.unwrap_or(default_signers);

        self.process_transaction(&[create_program_governance_instruction], Some(singers))
            .await?;

        let account = Governance {
            account_type: GovernanceAccountType::ProgramGovernance,
            config,
            proposals_count: 0,
        };

        let program_governance_address =
            get_program_governance_address(&realm_cookie.address, &governed_program_cookie.address);

        Ok(GovernanceCookie {
            address: program_governance_address,
            account,
            next_proposal_index: 0,
        })
    }

    #[allow(dead_code)]
    pub async fn with_proposal(
        &mut self,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
        governance_cookie: &mut GovernanceCookie,
    ) -> Result<ProposalCookie, ProgramError> {
        self.with_proposal_using_instruction(
            token_owner_record_cookie,
            governance_cookie,
            NopOverride,
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn with_signed_off_proposal(
        &mut self,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
        governance_cookie: &mut GovernanceCookie,
    ) -> Result<ProposalCookie, ProgramError> {
        let proposal_cookie = self
            .with_proposal(&token_owner_record_cookie, governance_cookie)
            .await?;

        let signatory_record_cookie = self
            .with_signatory(&proposal_cookie, &token_owner_record_cookie)
            .await?;

        self.sign_off_proposal(&proposal_cookie, &signatory_record_cookie)
            .await?;

        Ok(proposal_cookie)
    }

    #[allow(dead_code)]
    pub async fn with_proposal_using_instruction<F: Fn(&mut Instruction)>(
        &mut self,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
        governance_cookie: &mut GovernanceCookie,
        instruction_override: F,
    ) -> Result<ProposalCookie, ProgramError> {
        let proposal_index = governance_cookie.next_proposal_index;
        governance_cookie.next_proposal_index = governance_cookie.next_proposal_index + 1;

        let name = format!("Proposal #{}", proposal_index);

        let description_link = "Proposal Description".to_string();

        let governance_authority = token_owner_record_cookie.get_governance_authority();

        let mut create_proposal_instruction = create_proposal(
            &governance_cookie.address,
            &token_owner_record_cookie.token_owner.pubkey(),
            &governance_authority.pubkey(),
            &self.context.payer.pubkey(),
            &governance_cookie.account.config.realm,
            name.clone(),
            description_link.clone(),
            &token_owner_record_cookie.account.governing_token_mint,
            proposal_index,
        );

        instruction_override(&mut create_proposal_instruction);

        self.process_transaction(
            &[create_proposal_instruction],
            Some(&[&governance_authority]),
        )
        .await?;

        let account = Proposal {
            account_type: GovernanceAccountType::Proposal,
            description_link,
            name: name.clone(),
            governance: governance_cookie.address,
            governing_token_mint: token_owner_record_cookie.account.governing_token_mint,
            state: ProposalState::Draft,
            signatories_count: 0,
            // Clock always returns 1 when running under the test
            draft_at: 1,
            signing_off_at: None,
            voting_at: None,
            voting_completed_at: None,
            executing_at: None,
            closed_at: None,
            number_of_executed_instructions: 0,
            number_of_instructions: 0,
            token_owner_record: token_owner_record_cookie.address,
            signatories_signed_off_count: 0,
            yes_votes_count: 0,
            no_votes_count: 0,
        };

        let proposal_address = get_proposal_address(
            &governance_cookie.address,
            &token_owner_record_cookie.account.governing_token_mint,
            &proposal_index.to_le_bytes(),
        );

        Ok(ProposalCookie {
            address: proposal_address,
            account,
            proposal_owner: governance_authority.pubkey(),
        })
    }

    #[allow(dead_code)]
    pub async fn with_signatory(
        &mut self,
        proposal_cookie: &ProposalCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
    ) -> Result<SignatoryRecordCookie, ProgramError> {
        let signatory = Keypair::new();

        let add_signatory_instruction = add_signatory(
            &proposal_cookie.address,
            &token_owner_record_cookie.address,
            &token_owner_record_cookie.token_owner.pubkey(),
            &self.context.payer.pubkey(),
            &signatory.pubkey(),
        );

        self.process_transaction(
            &[add_signatory_instruction],
            Some(&[&token_owner_record_cookie.token_owner]),
        )
        .await?;

        let signatory_record_address =
            get_signatory_record_address(&proposal_cookie.address, &signatory.pubkey());

        let signatory_record_data = SignatoryRecord {
            account_type: GovernanceAccountType::SignatoryRecord,
            proposal: proposal_cookie.address,
            signatory: signatory.pubkey(),
            signed_off: false,
        };

        let signatory_record_cookie = SignatoryRecordCookie {
            address: signatory_record_address,
            account: signatory_record_data,
            signatory: signatory,
        };

        Ok(signatory_record_cookie)
    }

    #[allow(dead_code)]
    pub async fn remove_signatory(
        &mut self,
        proposal_cookie: &ProposalCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
        signatory_record_cookie: &SignatoryRecordCookie,
    ) -> Result<(), ProgramError> {
        let remove_signatory_instruction = remove_signatory(
            &proposal_cookie.address,
            &token_owner_record_cookie.address,
            &token_owner_record_cookie.token_owner.pubkey(),
            &signatory_record_cookie.account.signatory,
            &token_owner_record_cookie.token_owner.pubkey(),
        );

        self.process_transaction(
            &[remove_signatory_instruction],
            Some(&[&token_owner_record_cookie.token_owner]),
        )
        .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn sign_off_proposal(
        &mut self,
        proposal_cookie: &ProposalCookie,
        signatory_record_cookie: &SignatoryRecordCookie,
    ) -> Result<(), ProgramError> {
        let sign_off_proposal_instruction = sign_off_proposal(
            &proposal_cookie.address,
            &signatory_record_cookie.signatory.pubkey(),
        );

        self.process_transaction(
            &[sign_off_proposal_instruction],
            Some(&[&signatory_record_cookie.signatory]),
        )
        .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn finalize_vote(
        &mut self,
        proposal_cookie: &ProposalCookie,
    ) -> Result<(), ProgramError> {
        let sign_off_proposal_instruction = finalize_vote(
            &proposal_cookie.account.governance,
            &proposal_cookie.address,
            &proposal_cookie.account.governing_token_mint,
        );

        self.process_transaction(&[sign_off_proposal_instruction], None)
            .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn relinquish_vote(
        &mut self,
        proposal_cookie: &ProposalCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
    ) -> Result<(), ProgramError> {
        self.relinquish_vote_using_instruction(
            proposal_cookie,
            token_owner_record_cookie,
            NopOverride,
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn relinquish_vote_using_instruction<F: Fn(&mut Instruction)>(
        &mut self,
        proposal_cookie: &ProposalCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
        instruction_override: F,
    ) -> Result<(), ProgramError> {
        let mut relinquish_vote_instruction = relinquish_vote(
            &proposal_cookie.account.governance,
            &proposal_cookie.address,
            &token_owner_record_cookie.address,
            &proposal_cookie.account.governing_token_mint,
            Some(token_owner_record_cookie.token_owner.pubkey()),
            Some(self.context.payer.pubkey()),
        );

        instruction_override(&mut relinquish_vote_instruction);

        self.process_transaction(
            &[relinquish_vote_instruction],
            Some(&[&token_owner_record_cookie.token_owner]),
        )
        .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn cancel_proposal(
        &mut self,
        proposal_cookie: &ProposalCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
    ) -> Result<(), ProgramError> {
        let cancel_proposal_instruction = cancel_proposal(
            &proposal_cookie.address,
            &token_owner_record_cookie.address,
            &token_owner_record_cookie.token_owner.pubkey(),
        );

        self.process_transaction(
            &[cancel_proposal_instruction],
            Some(&[&token_owner_record_cookie.token_owner]),
        )
        .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn with_cast_vote(
        &mut self,
        proposal_cookie: &ProposalCookie,
        token_owner_record_cookie: &TokeOwnerRecordCookie,
        vote: Vote,
    ) -> Result<VoteRecordCookie, ProgramError> {
        let vote_instruction = cast_vote(
            &proposal_cookie.account.governance,
            &proposal_cookie.address,
            &token_owner_record_cookie.address,
            &token_owner_record_cookie.token_owner.pubkey(),
            &proposal_cookie.account.governing_token_mint,
            &self.context.payer.pubkey(),
            vote.clone(),
        );

        self.process_transaction(
            &[vote_instruction],
            Some(&[&token_owner_record_cookie.token_owner]),
        )
        .await?;

        let vote_amount = token_owner_record_cookie
            .account
            .governing_token_deposit_amount;

        let vote_weight = match vote {
            Vote::Yes => VoteWeight::Yes(vote_amount),
            Vote::No => VoteWeight::No(vote_amount),
        };

        let account = VoteRecord {
            account_type: GovernanceAccountType::VoteRecord,
            proposal: proposal_cookie.address,
            governing_token_owner: token_owner_record_cookie.token_owner.pubkey(),
            vote_weight,
            is_relinquished: false,
        };

        let vote_record_cookie = VoteRecordCookie {
            address: get_vote_record_address(
                &proposal_cookie.address,
                &token_owner_record_cookie.address,
            ),
            account,
        };

        Ok(vote_record_cookie)
    }

    #[allow(dead_code)]
    pub async fn get_token_owner_record_account(&mut self, address: &Pubkey) -> TokenOwnerRecord {
        self.get_borsh_account::<TokenOwnerRecord>(address).await
    }

    #[allow(dead_code)]
    pub async fn get_realm_account(&mut self, root_governance_address: &Pubkey) -> Realm {
        self.get_borsh_account::<Realm>(root_governance_address)
            .await
    }

    #[allow(dead_code)]
    pub async fn get_governance_account(
        &mut self,
        program_governance_address: &Pubkey,
    ) -> Governance {
        self.get_borsh_account::<Governance>(program_governance_address)
            .await
    }

    #[allow(dead_code)]
    pub async fn get_proposal_account(&mut self, proposal_address: &Pubkey) -> Proposal {
        self.get_borsh_account::<Proposal>(proposal_address).await
    }

    #[allow(dead_code)]
    pub async fn get_vote_record_account(&mut self, vote_record_address: &Pubkey) -> VoteRecord {
        self.get_borsh_account::<VoteRecord>(vote_record_address)
            .await
    }

    #[allow(dead_code)]
    pub async fn get_signatory_record_account(
        &mut self,
        proposal_address: &Pubkey,
    ) -> SignatoryRecord {
        self.get_borsh_account::<SignatoryRecord>(proposal_address)
            .await
    }

    #[allow(dead_code)]
    async fn get_packed_account<T: Pack + IsInitialized>(&mut self, address: &Pubkey) -> T {
        self.context
            .banks_client
            .get_packed_account_data::<T>(*address)
            .await
            .unwrap()
    }

    #[allow(dead_code)]
    pub async fn get_bincode_account<T: serde::de::DeserializeOwned>(
        &mut self,
        address: &Pubkey,
    ) -> T {
        self.context
            .banks_client
            .get_account(*address)
            .await
            .unwrap()
            .map(|a| deserialize::<T>(&a.data.borrow()).unwrap())
            .expect(format!("GET-TEST-ACCOUNT-ERROR: Account {}", address).as_str())
    }

    #[allow(dead_code)]
    pub async fn get_upgradable_loader_account(
        &mut self,
        address: &Pubkey,
    ) -> UpgradeableLoaderState {
        self.get_bincode_account(address).await
    }

    /// TODO: Add to SDK
    pub async fn get_borsh_account<T: BorshDeserialize>(&mut self, address: &Pubkey) -> T {
        self.get_account(address)
            .await
            .map(|a| try_from_slice_unchecked(&a.data).unwrap())
            .expect(format!("GET-TEST-ACCOUNT-ERROR: Account {} not found", address).as_str())
    }

    #[allow(dead_code)]
    pub async fn get_account(&mut self, address: &Pubkey) -> Option<Account> {
        self.context
            .banks_client
            .get_account(*address)
            .await
            .unwrap()
    }

    #[allow(dead_code)]
    pub async fn get_token_account(&mut self, address: &Pubkey) -> spl_token::state::Account {
        self.get_packed_account(address).await
    }

    pub async fn create_mint(&mut self, mint_keypair: &Keypair, mint_authority: &Pubkey) {
        let mint_rent = self.rent.minimum_balance(spl_token::state::Mint::LEN);

        let instructions = [
            system_instruction::create_account(
                &self.context.payer.pubkey(),
                &mint_keypair.pubkey(),
                mint_rent,
                spl_token::state::Mint::LEN as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_mint(
                &spl_token::id(),
                &mint_keypair.pubkey(),
                &mint_authority,
                None,
                0,
            )
            .unwrap(),
        ];

        self.process_transaction(&instructions, Some(&[&mint_keypair]))
            .await
            .unwrap();
    }

    #[allow(dead_code)]
    pub async fn create_empty_token_account(
        &mut self,
        token_account_keypair: &Keypair,
        token_mint: &Pubkey,
        owner: &Pubkey,
    ) {
        let create_account_instruction = system_instruction::create_account(
            &self.context.payer.pubkey(),
            &token_account_keypair.pubkey(),
            self.rent
                .minimum_balance(spl_token::state::Account::get_packed_len()),
            spl_token::state::Account::get_packed_len() as u64,
            &spl_token::id(),
        );

        let initialize_account_instruction = spl_token::instruction::initialize_account(
            &spl_token::id(),
            &token_account_keypair.pubkey(),
            token_mint,
            &owner,
        )
        .unwrap();

        self.process_transaction(
            &[create_account_instruction, initialize_account_instruction],
            Some(&[&token_account_keypair]),
        )
        .await
        .unwrap();
    }

    #[allow(dead_code)]
    pub async fn create_token_account_with_transfer_authority(
        &mut self,
        token_account_keypair: &Keypair,
        token_mint: &Pubkey,
        token_mint_authority: &Keypair,
        amount: u64,
        owner: &Keypair,
        transfer_authority: &Pubkey,
    ) {
        let create_account_instruction = system_instruction::create_account(
            &self.context.payer.pubkey(),
            &token_account_keypair.pubkey(),
            self.rent
                .minimum_balance(spl_token::state::Account::get_packed_len()),
            spl_token::state::Account::get_packed_len() as u64,
            &spl_token::id(),
        );

        let initialize_account_instruction = spl_token::instruction::initialize_account(
            &spl_token::id(),
            &token_account_keypair.pubkey(),
            token_mint,
            &owner.pubkey(),
        )
        .unwrap();

        let mint_instruction = spl_token::instruction::mint_to(
            &spl_token::id(),
            token_mint,
            &token_account_keypair.pubkey(),
            &token_mint_authority.pubkey(),
            &[],
            amount,
        )
        .unwrap();

        let approve_instruction = spl_token::instruction::approve(
            &spl_token::id(),
            &token_account_keypair.pubkey(),
            transfer_authority,
            &owner.pubkey(),
            &[],
            amount,
        )
        .unwrap();

        self.process_transaction(
            &[
                create_account_instruction,
                initialize_account_instruction,
                mint_instruction,
                approve_instruction,
            ],
            Some(&[&token_account_keypair, &token_mint_authority, &owner]),
        )
        .await
        .unwrap();
    }

    pub async fn mint_tokens(
        &mut self,
        token_mint: &Pubkey,
        token_mint_authority: &Keypair,
        token_account: &Pubkey,
        amount: u64,
    ) {
        let mint_instruction = spl_token::instruction::mint_to(
            &spl_token::id(),
            &token_mint,
            &token_account,
            &token_mint_authority.pubkey(),
            &[],
            amount,
        )
        .unwrap();

        self.process_transaction(&[mint_instruction], Some(&[&token_mint_authority]))
            .await
            .unwrap();
    }
}
