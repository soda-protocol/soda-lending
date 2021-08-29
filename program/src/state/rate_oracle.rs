#![allow(missing_docs)]
///
use super::*;
use std::any::Any;
use crate::{error::LendingError, math::WAD};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::{Slot, DEFAULT_TICKS_PER_SECOND, DEFAULT_TICKS_PER_SLOT, SECONDS_PER_DAY},
    entrypoint::ProgramResult, 
    program_error::ProgramError, 
    program_pack::{IsInitialized, Pack, Sealed}, 
    pubkey::{Pubkey, PUBKEY_BYTES}
};

const SLOTS_PER_YEAR: u64 = DEFAULT_TICKS_PER_SECOND * SECONDS_PER_DAY * 365 / DEFAULT_TICKS_PER_SLOT;

/// never expire in current version
const MAX_RATE_EXPIRED_SLOT: u64 = u64::MAX;

#[derive(Clone, Debug, Copy, Default, PartialEq)]
pub struct RateOracleConfig {
    pub a: u64,
    pub c: u64,
    pub l_u: u8,
    pub k_u: u128,
}

impl Param for RateOracleConfig {
    fn is_valid(&self) -> ProgramResult {
        if self.c < WAD && self.l_u < 100 {
            Ok(())
        } else {
            Err(LendingError::InvalidRateOracleConfig.into())
        }
    }
}

///
#[derive(Clone, Copy, Debug)]
pub struct Pause();

impl Param for Pause {
    fn is_valid(&self) -> ProgramResult {
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RateOracle {
    pub version: u8,
    pub owner: Pubkey,
    pub available: bool,
    pub last_slot: Slot,
    pub config: RateOracleConfig,
}

impl<P: Any + Param + Copy> Operator<P> for RateOracle {
    fn operate(&mut self, param: P) -> ProgramResult {
        if let Some(config) = <dyn Any>::downcast_ref::<RateOracleConfig>(&param) {
            self.config = *config;
            return Ok(());
        }

        if let Some(_pause) = <dyn Any>::downcast_ref::<Pause>(&param) {
            self.available = false;
            return Ok(());
        }

        panic!("unexpected param type");
    }
}

impl RateOracle {
    fn eplased_slots(&self, slot: Slot) -> Result<Slot, ProgramError> {
        slot
            .checked_sub(self.last_slot)
            .ok_or(LendingError::MathOverflow.into())
    }

    pub fn calculate_borrow_rate(&self, slot: Slot, utilization_rate: Rate) -> Result<Rate, ProgramError> {
        if !self.available || self.eplased_slots(slot)? >= MAX_RATE_EXPIRED_SLOT {
            return Err(LendingError::RateOracleNotAvailable.into());
        }

        let utilization_threshold = Rate::from_percent(self.config.l_u);
        let a = Rate::from_scaled_val(self.config.a);
        let c = Rate::from_scaled_val(self.config.c);
        let k_u = Rate::from_raw_val(self.config.k_u);

        let borrow_rate_per_year = if utilization_rate <= utilization_threshold {
            utilization_rate
                .try_mul(a)?
                .try_add(c)?
        } else {
            let z1 = utilization_threshold.try_mul(a)?;
            let z2 = utilization_rate
                .try_sub(utilization_threshold)?
                .try_mul(a)?
                .try_mul(k_u)?;
            
            z1.try_add(z2)?.try_add(c)?
        };

        borrow_rate_per_year.try_div(SLOTS_PER_YEAR)
    }
}

impl Sealed for RateOracle {}
impl IsInitialized for RateOracle {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

///
const RATE_ORACLE_RESERVE_LEN: usize = 256;
const RATE_ORACLE_LEN: usize = 331;

impl Pack for RateOracle {
    const LEN: usize = RATE_ORACLE_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, RATE_ORACLE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            owner,
            available,
            last_slot,
            a,
            c,
            l_u,
            k_u,
            _rest,
        ) = mut_array_refs![
            output,
            1,
            PUBKEY_BYTES,
            1,
            8,
            8,
            8,
            1,
            16,
            RATE_ORACLE_RESERVE_LEN
        ];

        *version = self.version.to_le_bytes();
        owner.copy_from_slice(self.owner.as_ref());
        pack_bool(self.available, available);
        *last_slot = self.last_slot.to_le_bytes();

        *a = self.config.a.to_le_bytes();
        *c = self.config.c.to_le_bytes();
        *l_u = self.config.l_u.to_le_bytes();
        *k_u = self.config.k_u.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, RATE_ORACLE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            owner,
            available,
            last_slot,
            a,
            c,
            l_u,
            k_u,
            _rest,
        ) = array_refs![
            input,
            1,
            PUBKEY_BYTES,
            1,
            8,
            8,
            8,
            1,
            16,
            RATE_ORACLE_RESERVE_LEN
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("RateOracle version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self{
            version,
            owner: Pubkey::new_from_array(*owner),
            available: unpack_bool(available)?,
            last_slot: u64::from_le_bytes(*last_slot),
            config: RateOracleConfig {
                a: u64::from_le_bytes(*a),
                c: u64::from_le_bytes(*c),
                l_u: u8::from_le_bytes(*l_u),
                k_u: u128::from_le_bytes(*k_u),
            },
        })
    }
}

