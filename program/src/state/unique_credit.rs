#![allow(missing_docs)]
use super::*;
use std::{cmp::Ordering, convert::TryInto};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    entrypoint::ProgramResult,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

pub struct UniqueCredit {
    /// Version of the struct
    pub version: u8,
    ///
    pub owner: Pubkey,
    ///
    pub reserve: Pubkey,
    ///
    pub borrow_limit: u64,
    ///
    pub acc_borrow_rate_wads: Decimal,
    ///
    pub borrowed_amount_wads: Decimal,
}

impl UniqueCredit {
    pub fn new(
        owner: Pubkey,
        reserve: Pubkey,
        borrow_limit: u64,
    ) -> Self {
        Self {
            version: PROGRAM_VERSION,
            owner,
            reserve,
            borrow_limit,
            acc_borrow_rate_wads: Decimal::one(),
            borrowed_amount_wads: Decimal::zero(),
        }
    }

    pub fn accrue_interest(&mut self, reserve: &MarketReserve) -> ProgramResult {
        match reserve.liquidity_info.acc_borrow_rate_wads.cmp(&self.acc_borrow_rate_wads) {
            Ordering::Less => Err(LendingError::NegativeInterestRate.into()),
            Ordering::Equal => Ok(()),
            Ordering::Greater => {
                let compounded_interest_rate: Rate = reserve.liquidity_info.acc_borrow_rate_wads
                    .try_div(self.acc_borrow_rate_wads)?
                    .try_into()?;

                self.borrowed_amount_wads = self.borrowed_amount_wads.try_mul(compounded_interest_rate)?;
                self.acc_borrow_rate_wads = reserve.liquidity_info.acc_borrow_rate_wads;

                Ok(())
            }
        }
    }

    pub fn borrow_in(
        &mut self,
        amount: u64,
        reserve: &MarketReserve,
    ) -> Result<u64, ProgramError> {
        let amount = calculate_amount(amount, reserve.liquidity_info.available);
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_add(Decimal::from(amount))?;

        if self.borrowed_amount_wads > Decimal::from(self.borrow_limit) {
            return Err(LendingError::InsufficientUniqueCreditLimit.into())
        }

        Ok(amount)
    }

    pub fn repay(
        &mut self,
        amount: u64,
    ) -> Result<RepaySettle, ProgramError> {
        let (amount, amount_decimal) = calculate_amount_and_decimal(amount, self.borrowed_amount_wads)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads
            .try_sub(amount_decimal)
            .map_err(|_| LendingError::RepayTooMuch)?;

        Ok(RepaySettle {
            amount,
            amount_decimal,
        })
    }
}

impl Sealed for UniqueCredit {}
impl IsInitialized for UniqueCredit {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const UNIQUE_CREDIT_PADDING_LEN: usize = 256;
const UNIQUE_CREDIT_LEN: usize = 361;

impl Pack for UniqueCredit {
    const LEN: usize = UNIQUE_CREDIT_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, UNIQUE_CREDIT_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            owner,
            reserve,
            borrow_limit,
            acc_borrow_rate_wads,
            borrowed_amount_wads,
            _padding,
        ) = mut_array_refs![
            output,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            8,
            16,
            16,
            UNIQUE_CREDIT_PADDING_LEN
        ];

        *version = self.version.to_le_bytes();
        owner.copy_from_slice(self.owner.as_ref());
        reserve.copy_from_slice(self.reserve.as_ref());
        *borrow_limit = self.borrow_limit.to_le_bytes();
        pack_decimal(self.acc_borrow_rate_wads, acc_borrow_rate_wads);
        pack_decimal(self.borrowed_amount_wads, borrowed_amount_wads);
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, UNIQUE_CREDIT_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            owner,
            reserve,
            borrow_limit,
            acc_borrow_rate_wads,
            borrowed_amount_wads,
            _padding,
        ) = array_refs![
            input,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            8,
            16,
            16,
            UNIQUE_CREDIT_PADDING_LEN
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("FrObligation version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self{
            version,
            owner: Pubkey::new_from_array(*owner),
            reserve: Pubkey::new_from_array(*reserve),
            borrow_limit: u64::from_le_bytes(*borrow_limit),
            acc_borrow_rate_wads: unpack_decimal(acc_borrow_rate_wads),
            borrowed_amount_wads: unpack_decimal(borrowed_amount_wads),
        })
    }
}
