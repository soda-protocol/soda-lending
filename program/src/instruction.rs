//! Instruction types

use crate::{
    error::LendingError,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar
};
use std::{convert::TryInto, mem::size_of};

/// Instructions supported by the lending program.
#[derive(Clone, Debug, PartialEq)]
pub enum LendingInstruction {
    /// 0
    InitManager {
        ///
        quote_decimal: u8,
    },
    /// 1
    InitMarketReserveWithoutLiquidity {
        ///
        liquidate_fee_rate: u64,
        ///
        liquidate_limit_rate: u64,
    },
    /// 2
    InitMarketReserveWithLiquidity {
        ///
        liquidate_fee_rate: u64,
        ///
        liquidate_limit_rate: u64,
        ///
        min_borrow_utilization_rate: u64,
        ///
        max_borrow_utilization_rate: u64,
        ///
        interest_fee_rate: u64, 
    },
    /// 3
    InitUserObligation,
    /// 4
    InitUserAsset,
    /// 5
    DepositLiquidity {
        ///
        amount: u64,
    },
    /// 6
    WithdrawLiquidity {
        ///
        amount: u64,
    },
    /// 7
    DepositCollateral {
        ///
        amount: u64,
    },
    /// 8
    BorrowLiquidity {
        ///
        amount: u64,
    },
}

impl LendingInstruction {
    /// Unpacks a byte buffer into a [LendingInstruction](enum.LendingInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(LendingError::InstructionUnpackError)?;
        Ok(match tag {
            0 => {
                let (quote_decimal, _rest) = Self::unpack_u8(rest)?;
                Self::InitManager { quote_decimal }
            }
            1 => {
                let (liquidate_fee_rate, rest) = Self::unpack_u64(rest)?;
                let (liquidate_limit_rate, _rest) = Self::unpack_u64(rest)?;

                Self::InitMarketReserveWithoutLiquidity {
                    liquidate_fee_rate,
                    liquidate_limit_rate,
                }
            }
            2 => {
                let (liquidate_fee_rate, rest) = Self::unpack_u64(rest)?;
                let (liquidate_limit_rate, rest) = Self::unpack_u64(rest)?;
                let (min_borrow_utilization_rate, rest) = Self::unpack_u64(rest)?;
                let (max_borrow_utilization_rate, rest) = Self::unpack_u64(rest)?;
                let (interest_fee_rate, _rest) = Self::unpack_u64(rest)?;

                Self::InitMarketReserveWithLiquidity {
                    liquidate_fee_rate,
                    liquidate_limit_rate,
                    min_borrow_utilization_rate,
                    max_borrow_utilization_rate,
                    interest_fee_rate,
                }
            }
            3 => Self::InitUserObligation,
            4 => Self::InitUserAsset,
            5 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::DepositLiquidity { amount }
            }
            6 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::WithdrawLiquidity { amount }
            },
            7 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::DepositCollateral { amount }
            },
            8 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::BorrowLiquidity { amount }
            }
            _ => {
                msg!("Instruction cannot be unpacked");
                return Err(LendingError::InstructionUnpackError.into());
            }
        })
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        if input.len() < 8 {
            msg!("u64 cannot be unpacked");
            return Err(LendingError::InstructionUnpackError.into());
        }
        let (amount, rest) = input.split_at(8);
        let amount = amount
            .get(..8)
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(LendingError::InstructionUnpackError)?;
        Ok((amount, rest))
    }

    fn unpack_u8(input: &[u8]) -> Result<(u8, &[u8]), ProgramError> {
        if input.is_empty() {
            msg!("u8 cannot be unpacked");
            return Err(LendingError::InstructionUnpackError.into());
        }
        let (amount, rest) = input.split_at(1);
        let amount = amount
            .get(..1)
            .and_then(|slice| slice.try_into().ok())
            .map(u8::from_le_bytes)
            .ok_or(LendingError::InstructionUnpackError)?;
        Ok((amount, rest))
    }

    /// Packs a [LendingInstruction](enum.LendingInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::InitManager { quote_decimal } => {
                buf.push(0);
                buf.extend_from_slice(&quote_decimal.to_le_bytes());
            }
            Self::InitMarketReserveWithoutLiquidity { 
                liquidate_fee_rate,
                liquidate_limit_rate,
            } => {
                buf.push(1);
                buf.extend_from_slice(&liquidate_fee_rate.to_le_bytes());
                buf.extend_from_slice(&liquidate_limit_rate.to_le_bytes());
            }
            Self::InitMarketReserveWithLiquidity {
                liquidate_fee_rate,
                liquidate_limit_rate,
                min_borrow_utilization_rate,
                max_borrow_utilization_rate,
                interest_fee_rate,
            } => {
                buf.push(2);
                buf.extend_from_slice(&liquidate_fee_rate.to_le_bytes());
                buf.extend_from_slice(&liquidate_limit_rate.to_le_bytes());
                buf.extend_from_slice(&min_borrow_utilization_rate.to_le_bytes());
                buf.extend_from_slice(&max_borrow_utilization_rate.to_le_bytes());
                buf.extend_from_slice(&interest_fee_rate.to_le_bytes());
            }
            Self::InitUserObligation => {
                buf.push(3);
            }
            Self::InitUserAsset => {
                buf.push(4);
            }
            Self::DepositLiquidity { amount } => {
                buf.push(5);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::WithdrawLiquidity { amount } => {
                buf.push(6);
                buf.extend_from_slice(&amount.to_le_bytes()); 
            }
            Self::DepositCollateral { amount } => {
                buf.push(7);
                buf.extend_from_slice(&amount.to_le_bytes()); 
            }
            Self::BorrowLiquidity { amount } => {
                buf.push(8);
                buf.extend_from_slice(&amount.to_le_bytes()); 
            }
        }
        buf
    }
}

///
pub fn init_manager(
    program_id: Pubkey,
    manager_info: Pubkey,
    owner_info: Pubkey,
    quote_decimal: u8,
) -> Instruction {
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new(manager_info, false),
            AccountMeta::new_readonly(owner_info, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::InitManager { quote_decimal }.pack(),
    }
}

///
pub fn init_market_reserve_without_liquidity(
    program_id: Pubkey,
    manager_info: Pubkey,
    market_reserve_info: Pubkey,
    price_oracle_info: Pubkey,
    token_account_info: Pubkey,
    authority_info: Pubkey,
    liquidate_fee_rate: u64,
    liquidate_limit_rate: u64,
) -> Instruction {
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_info, false),
            AccountMeta::new(market_reserve_info, false),
            AccountMeta::new_readonly(price_oracle_info, false),
            AccountMeta::new_readonly(token_account_info, false),
            AccountMeta::new_readonly(authority_info, true),
        ],
        data: LendingInstruction::InitMarketReserveWithoutLiquidity {
            liquidate_fee_rate,
            liquidate_limit_rate,
        }.pack(),
    }
}

