#![allow(missing_docs)]
///
use super::*;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES}
};

/// Lending market obligation state
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Manager {
    pub version: u8,
    pub bump_seed: u8,
    pub owner: Pubkey,
}

impl Manager {
    ///
    pub fn new(bump_seed: u8, owner: Pubkey) -> Self {
        Self {
            version: PROGRAM_VERSION,
            bump_seed,
            owner,
        }
    }
}

impl Sealed for Manager {}
impl IsInitialized for Manager {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const MANAGER_PADDING_LEN: usize = 128;
const MANAGER_LEN: usize = 162;

impl Pack for Manager {
    const LEN: usize = MANAGER_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, MANAGER_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            bump_seed,
            owner,
            _padding,
        ) = mut_array_refs![
            output,
            1,
            1,
            PUBKEY_BYTES,
            MANAGER_PADDING_LEN
        ];

        *version = self.version.to_le_bytes();
        *bump_seed = self.bump_seed.to_le_bytes();
        owner.copy_from_slice(self.owner.as_ref());
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, MANAGER_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            bump_seed,
            owner,
            _padding,
        ) = array_refs![
            input,
            1,
            1,
            PUBKEY_BYTES,
            MANAGER_PADDING_LEN
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("Manager version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self{
            version,
            bump_seed: u8::from_le_bytes(*bump_seed),
            owner: Pubkey::new_from_array(*owner),
        })
    }
}