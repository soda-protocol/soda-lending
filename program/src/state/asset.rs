#![allow(missing_docs)]
///
use super::*;
use crate::{
    error::LendingError,
    math::Rate,
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::Slot,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES}
};

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UserAsset {
    ///
    pub version: u8,
    ///
    pub reserve: Pubkey,
    ///
    pub owner: Pubkey,
    ///
    pub timestamp: Slot,
    ///
    pub principle_amount: u64,
    ///
    pub total_amount: u64,
}

impl Sealed for UserAsset {}
impl IsInitialized for UserAsset {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const USER_ASSET_LEN: usize = 89;

impl Pack for UserAsset {
    const LEN: usize = USER_ASSET_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, USER_ASSET_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            reserve,
            owner,
            timestamp,
            principle_amount,
            total_amount,
        ) = mut_array_refs![
            output,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            8,
            8,
            8
        ];

        *version = self.version.to_le_bytes();
        reserve.copy_from_slice(self.reserve.as_ref());
        owner.copy_from_slice(self.owner.as_ref());
        *timestamp = self.timestamp.to_le_bytes();
        *principle_amount = self.principle_amount.to_le_bytes();
        *total_amount = self.total_amount.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, USER_ASSET_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            reserve,
            owner,
            timestamp,
            principle_amount,
            total_amount,
        ) = array_refs![
            input,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            8,
            8,
            8
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("UserObligation version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self{
            version,
            reserve: Pubkey::new_from_array(*reserve),
            owner: Pubkey::new_from_array(*owner),
            timestamp: Slot::from_le_bytes(*timestamp),
            principle_amount: u64::from_le_bytes(*principle_amount),
            total_amount: u64::from_le_bytes(*total_amount),
        })
    }
}

impl UserAsset {
    ///
    pub fn update_interest(&mut self, slot: Slot, interest_rate: Rate) -> ProgramResult {
        let elapsed = slot
            .checked_sub(self.timestamp)
            .ok_or(LendingError::MathOverflow)?;

        let interest = calculate_compound_interest(self.total_amount, interest_rate, elapsed)?;
        self.total_amount = self.total_amount
            .checked_add(interest)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn deposit(&mut self, amount: u64) -> ProgramResult {
        self.principle_amount = self.principle_amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;
        self.total_amount = self.total_amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;
        
        Ok(())
    }
    ///
    pub fn withdraw(&mut self, amount: u64) -> Result<Fund, ProgramError> {
        let acc_interest = self.total_amount
            .checked_sub(self.principle_amount)
            .ok_or(LendingError::MathOverflow)?;

        self.total_amount = self.total_amount
            .checked_sub(amount)
            .ok_or(LendingError::UserAssetInsufficient)?;

        if amount > acc_interest {
            self.principle_amount = self.total_amount;

            Ok(Fund{
                principal: amount - acc_interest,
                interest: acc_interest,
            })
        } else {
            Ok(Fund{
                principal: 0,
                interest: amount,
            })
        }
    }
    ///
    pub fn change_owner(&mut self, new_owner: Pubkey) {
        self.owner = new_owner;
    }
}