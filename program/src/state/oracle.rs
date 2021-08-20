#![allow(missing_docs)]
///
use super::*;
use crate::error::LendingError;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::Slot, 
    entrypoint::ProgramResult, 
    program_error::ProgramError, 
    program_pack::{IsInitialized, Pack, Sealed}, 
    pubkey::{Pubkey, PUBKEY_BYTES}
};

const MAX_RATE_EXPIRED_SLOT: u64 = 10000000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RateOracle {
    pub version: u8,
    pub owner: Pubkey,
    pub status: bool,
    pub timestamp: Slot,
    pub interest_rate: u64,
    pub borrow_rate: u64,
}

impl RateOracle {
    pub fn feed(&mut self, interest_rate: u64, borrow_rate: u64, slot: Slot) {
        // let unit_secs = DEFAULT_TICKS_PER_SECOND * unit_secs / DEFAULT_TICKS_PER_SLOT;
        self.status = true;
        self.timestamp = slot;
        self.interest_rate = interest_rate;
        self.borrow_rate = borrow_rate;
    }

    pub fn check_valid(&self, slot: Slot) -> ProgramResult {
        if self.status {
            let eplased = slot
                .checked_sub(self.timestamp)
                .ok_or(LendingError::MathOverflow)?;

            if eplased < MAX_RATE_EXPIRED_SLOT {
                return Ok(());
            }
        }

        Err(LendingError::RateOracleNotAvailable.into())
    }

    pub fn mark_stale(&mut self) {
        self.status = false;
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
            status,
            timestamp,
            interest_rate,
            borrow_rate,
        ) = mut_array_refs![
            output,
            1,
            PUBKEY_BYTES,
            1,
            8,
            8,
            8
        ];

        *version = self.version.to_le_bytes();
        owner.copy_from_slice(self.owner.as_ref());
        pack_bool(self.status, status);
        *timestamp = self.timestamp.to_le_bytes();
        *interest_rate = self.interest_rate.to_le_bytes();
        *borrow_rate = self.borrow_rate.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, RATE_ORACLE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            owner,
            status,
            timestamp,
            interest_rate,
            borrow_rate,
        ) = array_refs![
            input,
            1,
            PUBKEY_BYTES,
            1,
            8,
            8,
            8
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("RateOracle version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self{
            version,
            owner: Pubkey::new_from_array(*owner),
            status: unpack_bool(status)?,
            timestamp: u64::from_le_bytes(*timestamp),
            interest_rate: u64::from_le_bytes(*interest_rate),
            borrow_rate: u64::from_le_bytes(*borrow_rate),
        })
    }
}

