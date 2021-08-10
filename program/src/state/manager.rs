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
    ///
    pub version: u8,
    /// Bump seed for derived authority address
    pub bump_seed: u8,
    /// Quote currency token mint
    pub quote_decimal: u8,
    /// Owner authority which can add new reserves
    pub owner: Pubkey,
    /// Token program id
    pub token_program_id: Pubkey,
}

impl Sealed for Manager {}
impl IsInitialized for Manager {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const MANAGER_LEN: usize = 67;

impl Pack for Manager {
    const LEN: usize = MANAGER_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, MANAGER_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            bump_seed,
            quote_decimal,
            owner,
            token_program_id,
        ) = mut_array_refs![
            output,
            1,
            1,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES
        ];

        *version = self.version.to_le_bytes();
        *bump_seed = self.bump_seed.to_le_bytes();
        *quote_decimal = self.quote_decimal.to_le_bytes();
        owner.copy_from_slice(self.owner.as_ref());
        token_program_id.copy_from_slice(self.token_program_id.as_ref());
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, MANAGER_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            bump_seed,
            quote_decimal,
            owner,
            token_program_id,
        ) = array_refs![
            input,
            1,
            1,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("Manager version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self{
            version,
            bump_seed: u8::from_le_bytes(*bump_seed),
            quote_decimal: u8::from_le_bytes(*quote_decimal),
            owner: Pubkey::new_from_array(*owner),
            token_program_id: Pubkey::new_from_array(*token_program_id),
        })
    }
}