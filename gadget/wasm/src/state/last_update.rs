use super::*;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};

// Number of slots to consider stale after
pub const STALE_AFTER_SLOTS_ELAPSED: u64 = 1;

/// Last update state
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LastUpdate {
    /// Last slot when updated
    pub slot: u64,
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

    fn unpack_from_slice(input: &[u8]) -> Result<Self, SodaError> {
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
            slot: u64::from_le_bytes(*slot),
            stale: unpack_bool(stale)?,
        })
    }
}