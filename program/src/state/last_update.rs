use crate::{error::LendingError, state::{pack_bool, unpack_bool}};
use solana_program::{
    clock::Slot,
    program_error::ProgramError,
    program_pack::{Pack, Sealed}
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use std::cmp::Ordering;

/// Number of slots to consider stale after
pub const STALE_AFTER_SLOTS_ELAPSED: u64 = 1;

/// Last update state
#[derive(Clone, Debug, Default)]
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
        Self { slot, stale: false }
    }

    /// Return slots elapsed since given slot
    pub fn slots_elapsed(&self, slot: Slot) -> Result<u64, ProgramError> {
       slot.checked_sub(self.slot).ok_or(LendingError::MathOverflow.into())
    }

    /// Set last update slot
    pub fn update_slot(&mut self, slot: Slot) {
        self.slot = slot;
        self.stale = false;
    }

    /// Set stale to true
    pub fn mark_stale(&mut self) {
        self.stale = true;
    }

    /// Check if marked stale or last update slot is too long ago
    pub fn is_stale(&self, slot: Slot) -> Result<bool, ProgramError> {
        Ok(self.stale || self.slots_elapsed(slot)? <= STALE_AFTER_SLOTS_ELAPSED)
    }
}

impl PartialEq for LastUpdate {
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot
    }
}

impl PartialOrd for LastUpdate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.slot.partial_cmp(&other.slot)
    }
}
