use crate::{error::LendingError, state::{pack_bool, unpack_bool}};
use solana_program::{
    clock::Slot,
    program_error::ProgramError,
    program_pack::{Pack, Sealed}
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};

/// Number of slots to consider stale after, about 5s
pub const STALE_AFTER_SLOTS_ELAPSED: u64 = 10;

/// Last update state
/// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!  Remark  !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
/// Considering transaction size limit (1232), we can not pack <refresh-reserves> +
/// <refresh-obligation> + <liquidate/flash liquidation> to one transaction while
/// reserves are too many (> 10). So we have no choice but split those instructions
/// into multi-transactions, which break atomicity. Stale slots eplased confidence
/// is designed as trait for LastUpdate, to restrict timeliness between related
/// transactions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LastUpdate {
    /// Last slot when updated
    pub slot: Slot,
    /// True when marked stale, false when slot updated
    pub stale: bool,
}

impl Sealed for LastUpdate {}
///
pub const LAST_UPDATE_LEN: usize = 9;

impl Pack for LastUpdate {
    const LEN: usize = LAST_UPDATE_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, LAST_UPDATE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            slot,
            stale,
        ) = mut_array_refs![
            output,
            8,
            1
        ];

        *slot = self.slot.to_le_bytes();
        pack_bool(self.stale, stale);
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, LAST_UPDATE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            slot,
            stale,
        ) = array_refs![
            input,
            8,
            1
        ];

        Ok(Self{
            slot: Slot::from_le_bytes(*slot),
            stale: unpack_bool(stale)?,
        })
    }
}

impl LastUpdate {
    /// Create new last update
    pub fn new(slot: Slot) -> Self {
        Self {
            slot,
            stale: true,
        }
    }

    /// Return slots elapsed since given slot
    pub fn slots_elapsed(&self, slot: Slot) -> Result<u64, ProgramError> {
       slot.checked_sub(self.slot).ok_or(LendingError::MathOverflow.into())
    }

    /// Set last update slot
    pub fn update_slot(&mut self, slot: Slot, stale: bool) {
        self.slot = slot;
        self.stale = stale;
    }

    // Set stale to true
    pub fn mark_stale(&mut self) {
        self.stale = true;
    }

    /// Check if marked stale or last update slot is too long ago
    pub fn is_strict_stale(&self, slot: Slot) -> Result<bool, ProgramError> {
        Ok(self.stale || self.slots_elapsed(slot)? > 0)
    }

    pub fn is_lax_stale(&self, slot: Slot) -> Result<bool, ProgramError> {
        Ok(self.stale || self.slots_elapsed(slot)? > STALE_AFTER_SLOTS_ELAPSED)
    }
}
