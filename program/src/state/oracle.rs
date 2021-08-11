#![allow(missing_docs)]
///
use super::*;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::Slot,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES}
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RateOracle {
    pub version: u8,
    pub owner: Pubkey,
    pub interest_rate: u64,
    pub borrow_rate: u64,
    pub last_update: LastUpdate,
}

impl RateOracle {
    pub fn feed(&mut self, interest_rate: u64, borrow_rate: u64, slot: Slot) {
        self.interest_rate = interest_rate;
        self.borrow_rate = borrow_rate;
        self.last_update.update_slot(slot);
    }
}

impl Sealed for RateOracle {}
impl IsInitialized for RateOracle {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}
///
pub const RATE_ORACLE_LEN: usize = 58;

impl Pack for RateOracle {
    const LEN: usize = RATE_ORACLE_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, RATE_ORACLE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            owner,
            interest_rate,
            borrow_rate,
            last_update,
        ) = mut_array_refs![
            output,
            1,
            PUBKEY_BYTES,
            8,
            8,
            LAST_UPDATE_LEN
        ];

        *version = self.version.to_le_bytes();
        owner.copy_from_slice(self.owner.as_ref());
        *interest_rate = self.interest_rate.to_le_bytes();
        *borrow_rate = self.borrow_rate.to_le_bytes();
        self.last_update.pack_into_slice(last_update);
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, RATE_ORACLE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            owner,
            interest_rate,
            borrow_rate,
            last_update,
        ) = array_refs![
            input,
            1,
            PUBKEY_BYTES,
            8,
            8,
            LAST_UPDATE_LEN
        ];

        let last_update = LastUpdate::unpack_from_slice(last_update)?;

        Ok(Self{
            version: u8::from_le_bytes(*version),
            owner: Pubkey::new_from_array(*owner),
            interest_rate: u64::from_le_bytes(*interest_rate),
            borrow_rate: u64::from_le_bytes(*borrow_rate),
            last_update,
        })
    }
}

