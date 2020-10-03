//! Instruction types

use crate::state::{Policies, User};
use bn::{Fr, G1};
use borsh::{BorshDeserialize, BorshSerialize};
use elgamal_bn::public::PublicKey;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
};

/// Instructions supported by the Themis program.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub enum ThemisInstruction {
    /// Initialize a new user account
    ///
    /// The `InitializeUserAccount` instruction requires no signers and MUST be included within
    /// the same Transaction as the system program's `CreateInstruction` that creates the account
    /// being initialized.  Otherwise another party can acquire ownership of the uninitialized account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable]` The account to initialize.
    InitializeUserAccount {
        /// Public key for all encrypted interations
        public_key: PublicKey,
    },

    /// Initialize a new policies account
    ///
    /// The `InitializePoliciesAccount` instruction requires no signers and MUST be included within
    /// the same Transaction as the system program's `CreateInstruction` that creates the account
    /// being initialized.  Otherwise another party can acquire ownership of the uninitialized account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable]` The account to initialize.
    InitializePoliciesAccount {
        /// Number of policies to be added
        num_scalars: u8,
    },

    /// Store policies
    ///
    /// The `StorePolices` instruction is used to set individual policies.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable, signer]` The policies account.
    StorePolicies {
        /// Policies to be added
        scalars: Vec<(u8, Fr)>,
    },

    /// Calculate aggregate. The length of the `input` vector must equal the
    /// number of policies.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable, signer]`  The user account
    ///   1. `[]`  The policies account
    SubmitInteractions {
        /// Encrypted interactions
        encrypted_interactions: Vec<(u8, (G1, G1))>,
    },

    /// Submit proof decryption
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable, signer]`  The user account
    SubmitProofDecryption {
        /// plaintext
        plaintext: G1,

        /// (announcement_g, announcement_ctx)
        announcement: Box<(G1, G1)>,

        /// response
        response: Fr,
    },

    /// Request a payment
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable, signer]`  The user account
    RequestPayment {
        /// Encrypted aggregate
        encrypted_aggregate: Box<(G1, G1)>,

        /// Decrypted aggregate
        decrypted_aggregate: G1,

        /// Proof correct decryption
        proof_correct_decryption: G1,
    },
}

impl ThemisInstruction {
    pub fn serialize(&self) -> Result<Vec<u8>, ProgramError> {
        self.try_to_vec()
            .map_err(|_| ProgramError::AccountDataTooSmall)
    }

    pub(crate) fn deserialize(data: &[u8]) -> Result<Self, ProgramError> {
        Self::try_from_slice(&data).map_err(|_| ProgramError::InvalidInstructionData)
    }
}

/// Return an `InitializeUserAccount` instruction.
fn initialize_user_account(user_pubkey: &Pubkey, public_key: PublicKey) -> Instruction {
    let data = ThemisInstruction::InitializeUserAccount { public_key };

    let accounts = vec![AccountMeta::new(*user_pubkey, false)];

    Instruction {
        program_id: crate::id(),
        accounts,
        data: data.serialize().unwrap(),
    }
}

/// Return two instructions that create and initialize a user account.
pub fn create_user_account(
    from: &Pubkey,
    user_pubkey: &Pubkey,
    lamports: u64,
    public_key: PublicKey,
) -> Vec<Instruction> {
    let space = User::default().try_to_vec().unwrap().len() as u64;
    vec![
        system_instruction::create_account(from, user_pubkey, lamports, space, &crate::id()),
        initialize_user_account(user_pubkey, public_key),
    ]
}

/// Return an `InitializePoliciesAccount` instruction.
fn initialize_policies_account(policies_pubkey: &Pubkey, num_scalars: u8) -> Instruction {
    let data = ThemisInstruction::InitializePoliciesAccount { num_scalars };
    let accounts = vec![AccountMeta::new(*policies_pubkey, false)];
    Instruction {
        program_id: crate::id(),
        accounts,
        data: data.serialize().unwrap(),
    }
}

/// Return two instructions that create and initialize a policies account.
pub fn create_policies_account(
    from: &Pubkey,
    policies_pubkey: &Pubkey,
    lamports: u64,
    num_scalars: u8,
) -> Vec<Instruction> {
    let space = Policies::new(num_scalars).try_to_vec().unwrap().len() as u64;
    vec![
        system_instruction::create_account(from, policies_pubkey, lamports, space, &crate::id()),
        initialize_policies_account(policies_pubkey, num_scalars),
    ]
}

/// Return an `InitializePoliciesAccount` instruction.
pub fn store_policies(policies_pubkey: &Pubkey, scalars: Vec<(u8, Fr)>) -> Instruction {
    let data = ThemisInstruction::StorePolicies { scalars };
    let accounts = vec![AccountMeta::new(*policies_pubkey, true)];
    Instruction {
        program_id: crate::id(),
        accounts,
        data: data.serialize().unwrap(),
    }
}

/// Return a `SubmitInteractions` instruction.
pub fn submit_interactions(
    user_pubkey: &Pubkey,
    policies_pubkey: &Pubkey,
    encrypted_interactions: Vec<(u8, (G1, G1))>,
) -> Instruction {
    let data = ThemisInstruction::SubmitInteractions {
        encrypted_interactions,
    };
    let accounts = vec![
        AccountMeta::new(*user_pubkey, true),
        AccountMeta::new_readonly(*policies_pubkey, false),
    ];
    Instruction {
        program_id: crate::id(),
        accounts,
        data: data.serialize().unwrap(),
    }
}

/// Return a `SubmitProofDecryption` instruction.
pub fn submit_proof_decryption(
    user_pubkey: &Pubkey,
    plaintext: G1,
    announcement_g: G1,
    announcement_ctx: G1,
    response: Fr,
) -> Instruction {
    let data = ThemisInstruction::SubmitProofDecryption {
        plaintext,
        announcement: Box::new((announcement_g, announcement_ctx)),
        response,
    };
    let accounts = vec![AccountMeta::new(*user_pubkey, true)];
    Instruction {
        program_id: crate::id(),
        accounts,
        data: data.serialize().unwrap(),
    }
}

/// Return a `RequestPayment` instruction.
pub fn request_payment(
    user_pubkey: &Pubkey,
    encrypted_aggregate: (G1, G1),
    decrypted_aggregate: G1,
    proof_correct_decryption: G1,
) -> Instruction {
    let data = ThemisInstruction::RequestPayment {
        encrypted_aggregate: Box::new(encrypted_aggregate),
        decrypted_aggregate,
        proof_correct_decryption,
    };
    let accounts = vec![AccountMeta::new(*user_pubkey, true)];
    Instruction {
        program_id: crate::id(),
        accounts,
        data: data.serialize().unwrap(),
    }
}
