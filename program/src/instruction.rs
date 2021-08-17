//! Instruction types
#![allow(missing_docs)]
use crate::{
    id, error::LendingError,
    state::{LiquidityConfig, CollateralConfig},
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar
};
use std::{convert::{TryInto, TryFrom}, mem::size_of};

/// Instructions supported by the lending program.
#[derive(Clone, Debug, PartialEq)]
pub enum LendingInstruction {
    /// 0
    InitManager {
        ///
        quote_currency: [u8; 32],
    },
    /// 1
    InitRateOracle,
    /// 2
    InitMarketReserveWithoutLiquidity {
        ///
        collateral_config: CollateralConfig,
    },
    /// 3
    InitMarketReserveWithLiquidity {
        ///
        collateral_config: CollateralConfig,
        ///
        liquidity_config: LiquidityConfig,
    },
    /// 4
    InitUserObligation,
    /// 5
    InitUserAsset,
    /// 6
    DepositLiquidity {
        ///
        amount: u64,
    },
    /// 7
    WithdrawLiquidity {
        ///
        amount: u64,
    },
    /// 8
    DepositCollateral {
        ///
        amount: u64,
    },
    /// 9
    BorrowLiquidity {
        ///
        amount: u64,
    },
    /// 10
    RepayLoan {
        ///
        amount: u64,
    },
    /// 11
    RedeemCollateral {
        ///
        amount: u64,      
    },
    /// 12
    Liquidate {
        ///
        is_arbitrary: bool,
        ///
        amount: u64,   
    },
    /// 13
    FeedRateOracle {
        ///
        interest_rate: u64,
        ///
        borrow_rate: u64,
    },
    /// 14
    PauseRateOracle,
    /// 15
    AddLiquidityToReserve {
        ///
        liquidity_config: LiquidityConfig,   
    },
    /// 16
    WithdrawFee {
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
                let (quote_currency, _rest) = Self::unpack_bytes32(rest)?;
                Self::InitManager { quote_currency }
            }
            1 => Self::InitRateOracle,
            2 => {
                let (collateral_config, _rest) = Self::unpack_collateral_config(rest)?;
                Self::InitMarketReserveWithoutLiquidity { collateral_config }
            }
            3 => {
                let (collateral_config, rest) = Self::unpack_collateral_config(rest)?;
                let (liquidity_config, _rest) = Self::unpack_liquidity_config(rest)?;
                Self::InitMarketReserveWithLiquidity { collateral_config, liquidity_config }
            }
            4 => Self::InitUserObligation,
            5 => Self::InitUserAsset,
            6 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::DepositLiquidity { amount }
            }
            7 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::WithdrawLiquidity { amount }
            },
            8 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::DepositCollateral { amount }
            },
            9 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::BorrowLiquidity { amount }
            }
            10 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RepayLoan { amount }
            }
            11 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RedeemCollateral { amount }
            }
            12 => {
                let (is_arbitrary, rest) = Self::unpack_bool(rest)?;
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::Liquidate { is_arbitrary, amount }
            }
            13 => {
                let (interest_rate, rest) = Self::unpack_u64(rest)?;
                let (borrow_rate, _rest) = Self::unpack_u64(rest)?;
                Self::FeedRateOracle { interest_rate, borrow_rate }
            }
            14 => Self::PauseRateOracle,
            15 => {
                let (liquidity_config, _rest) = Self::unpack_liquidity_config(rest)?;
                Self::AddLiquidityToReserve { liquidity_config }
            }
            16 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::WithdrawFee { amount }
            }
            _ => {
                msg!("Instruction cannot be unpacked");
                return Err(LendingError::InstructionUnpackError.into());
            }
        })
    }

    fn unpack_collateral_config(input: &[u8]) -> Result<(CollateralConfig, &[u8]), ProgramError> {
        let (liquidate_fee_rate, rest) = Self::unpack_u64(input)?;
        let (arbitrary_liquidate_rate, rest) = Self::unpack_u64(rest)?;
        let (liquidate_limit, rest) = Self::unpack_u8(rest)?;
        let (effective_value_rate, rest) = Self::unpack_u8(rest)?;
        let (close_factor, rest) = Self::unpack_u8(rest)?;

        Ok((
            CollateralConfig {
                liquidate_fee_rate,
                arbitrary_liquidate_rate,
                liquidate_limit,
                effective_value_rate,
                close_factor,
            }, rest
        ))
    }

    fn unpack_liquidity_config(input: &[u8]) -> Result<(LiquidityConfig, &[u8]), ProgramError> {
        let (interest_fee_rate, rest) = Self::unpack_u64(input)?;
        let (max_borrow_utilization_rate, rest) = Self::unpack_u8(rest)?;

        Ok((
            LiquidityConfig {
                interest_fee_rate,
                max_borrow_utilization_rate,
            }, rest
        ))
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

    fn unpack_bool(input: &[u8]) -> Result<(bool, &[u8]), ProgramError> {
        if input.is_empty() {
            msg!("bool cannot be unpacked");
            return Err(LendingError::InstructionUnpackError.into());
        }
        let (amount, rest) = input.split_first().ok_or(LendingError::InstructionUnpackError)?;
        match *amount {
            0 => Ok((false, rest)),
            1 => Ok((true, rest)),
            _ => {
                msg!("Boolean cannot be unpacked");
                Err(LendingError::InstructionUnpackError.into())
            }
        }
    }

    fn unpack_bytes32(input: &[u8]) -> Result<([u8; 32], &[u8]), ProgramError> {
        if input.len() < 32 {
            msg!("32 bytes cannot be unpacked");
            return Err(LendingError::InstructionUnpackError.into());
        }
        let (bytes, rest) = input.split_at(32);

        Ok((
            *<&[u8; 32]>::try_from(bytes)
                .map_err(|_| LendingError::InstructionUnpackError)?,
            rest
        ))
    }

    /// Packs a [LendingInstruction](enum.LendingInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::InitManager { quote_currency } => {
                buf.push(0);
                buf.extend_from_slice(&quote_currency[..]);
            }
            Self::InitRateOracle => buf.push(1),
            Self::InitMarketReserveWithoutLiquidity { collateral_config } => {
                buf.push(2);
                Self::pack_collateral_config(collateral_config, &mut buf);
            }
            Self::InitMarketReserveWithLiquidity {
                collateral_config,
                liquidity_config 
            } => {
                buf.push(3);
                Self::pack_collateral_config(collateral_config, &mut buf);
                Self::pack_liquidity_config(liquidity_config, &mut buf);
            }
            Self::InitUserObligation => buf.push(4),
            Self::InitUserAsset => buf.push(5),
            Self::DepositLiquidity { amount } => {
                buf.push(6);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::WithdrawLiquidity { amount } => {
                buf.push(7);
                buf.extend_from_slice(&amount.to_le_bytes()); 
            }
            Self::DepositCollateral { amount } => {
                buf.push(8);
                buf.extend_from_slice(&amount.to_le_bytes()); 
            }
            Self::BorrowLiquidity { amount } => {
                buf.push(9);
                buf.extend_from_slice(&amount.to_le_bytes()); 
            }
            Self::RepayLoan { amount } => {
                buf.push(10);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::RedeemCollateral { amount } => {
                buf.push(11);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::Liquidate { is_arbitrary, amount } => {
                buf.push(12);
                buf.extend_from_slice(&[is_arbitrary as u8][..]);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::FeedRateOracle { interest_rate, borrow_rate } => {
                buf.push(13);
                buf.extend_from_slice(&interest_rate.to_le_bytes());
                buf.extend_from_slice(&borrow_rate.to_le_bytes());
            }
            Self::PauseRateOracle => buf.push(14),
            Self::AddLiquidityToReserve { liquidity_config } => {
                buf.push(15);
                Self::pack_liquidity_config(liquidity_config, &mut buf);
            }
            Self::WithdrawFee { amount } => {
                buf.push(16);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
        }
        buf
    }

    fn pack_collateral_config(collateral_config: CollateralConfig, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&collateral_config.liquidate_fee_rate.to_le_bytes());
        buf.extend_from_slice(&collateral_config.arbitrary_liquidate_rate.to_le_bytes());
        buf.extend_from_slice(&collateral_config.liquidate_limit.to_le_bytes());
        buf.extend_from_slice(&collateral_config.effective_value_rate.to_le_bytes());
        buf.extend_from_slice(&collateral_config.close_factor.to_le_bytes());
    }

    fn pack_liquidity_config(liquidity_config: LiquidityConfig, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&liquidity_config.interest_fee_rate.to_le_bytes());
        buf.extend_from_slice(&liquidity_config.max_borrow_utilization_rate.to_le_bytes());
    }
}

///
pub fn init_manager(
    manager_info: Pubkey,
    owner_info: Pubkey,
    oracle_program_id: Pubkey,
    quote_currency: [u8; 32],
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new(manager_info, false),
            AccountMeta::new_readonly(owner_info, false),
            AccountMeta::new_readonly(oracle_program_id, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::InitManager { quote_currency }.pack(),
    }
}

pub fn init_rate_oracle(
    rate_oracle_info: Pubkey,
    owner_info: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new(rate_oracle_info, false),
            AccountMeta::new_readonly(owner_info, false),
        ],
        data: LendingInstruction::InitRateOracle.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn init_market_reserve_without_liquidity(
    manager_info: Pubkey,
    market_reserve_info: Pubkey,
    pyth_product_info: Pubkey,
    pyth_price_info: Pubkey,
    token_mint_info: Pubkey,
    token_account_info: Pubkey,
    authority_info: Pubkey,
    collateral_config: CollateralConfig,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_info, false),
            AccountMeta::new(market_reserve_info, false),
            AccountMeta::new_readonly(pyth_product_info, false),
            AccountMeta::new_readonly(pyth_price_info, false),
            AccountMeta::new_readonly(token_mint_info, false),
            AccountMeta::new_readonly(token_account_info, false),
            AccountMeta::new_readonly(authority_info, true),
        ],
        data: LendingInstruction::InitMarketReserveWithoutLiquidity{ collateral_config }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn init_market_reserve_with_liquidity(
    manager_info: Pubkey,
    market_reserve_info: Pubkey,
    pyth_product_info: Pubkey,
    pyth_price_info: Pubkey,
    token_mint_info: Pubkey,
    token_account_info: Pubkey,
    authority_info: Pubkey,
    rate_oracle_info: Pubkey,
    collateral_config: CollateralConfig,
    liquidity_config: LiquidityConfig,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_info, false),
            AccountMeta::new(market_reserve_info, false),
            AccountMeta::new_readonly(pyth_product_info, false),
            AccountMeta::new_readonly(pyth_price_info, false),
            AccountMeta::new_readonly(token_mint_info, false),
            AccountMeta::new_readonly(token_account_info, false),
            AccountMeta::new_readonly(authority_info, true),
            AccountMeta::new_readonly(rate_oracle_info, false),
        ],
        data: LendingInstruction::InitMarketReserveWithLiquidity{
            collateral_config,
            liquidity_config,
        }.pack(),
    }
}

pub fn init_user_obligation(
    market_reserve_info: Pubkey,
    user_obligation_info: Pubkey,
    owner_info: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(market_reserve_info, false),
            AccountMeta::new(user_obligation_info, false),
            AccountMeta::new_readonly(owner_info, false),
        ],
        data: LendingInstruction::InitUserObligation.pack(),
    }
}

pub fn init_user_asset(
    market_reserve_info: Pubkey,
    user_asset_info: Pubkey,
    owner_info: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(market_reserve_info, false),
            AccountMeta::new(user_asset_info, false),
            AccountMeta::new_readonly(owner_info, false),
        ],
        data: LendingInstruction::InitUserAsset.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn deposit_liquidity(
    market_reserve_info: Pubkey,
    manager_token_account_info: Pubkey,
    rate_oracle_info: Pubkey,
    user_asset_info: Pubkey,
    user_authority_info: Pubkey,
    user_token_account_info: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new(market_reserve_info, false),
            AccountMeta::new(manager_token_account_info, false),
            AccountMeta::new_readonly(rate_oracle_info, false),
            AccountMeta::new(user_asset_info, false),
            AccountMeta::new_readonly(user_authority_info, true),
            AccountMeta::new(user_token_account_info, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::DepositLiquidity{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn withdraw_liquidity(
    manager_info: Pubkey,
    market_reserve_info: Pubkey,
    manager_token_account_info: Pubkey,
    rate_oracle_info: Pubkey,
    user_asset_info: Pubkey,
    user_authority_info: Pubkey,
    user_token_account_info: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_info, _bump_seed) = Pubkey::find_program_address(
        &[manager_info.as_ref()],
        &program_id,
    );

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_info, false),
            AccountMeta::new_readonly(manager_authority_info, true),
            AccountMeta::new(market_reserve_info, false),
            AccountMeta::new(manager_token_account_info, false),
            AccountMeta::new_readonly(rate_oracle_info, false),
            AccountMeta::new(user_asset_info, false),
            AccountMeta::new_readonly(user_authority_info, true),
            AccountMeta::new(user_token_account_info, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::WithdrawLiquidity{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn deposit_collateral(
    market_reserve_info: Pubkey,
    manager_token_account_info: Pubkey,
    user_obligatiton_info: Pubkey,
    user_authority_info: Pubkey,
    user_token_account_info: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new(market_reserve_info, false),
            AccountMeta::new(manager_token_account_info, false),
            AccountMeta::new(user_obligatiton_info, false),
            AccountMeta::new_readonly(user_authority_info, true),
            AccountMeta::new(user_token_account_info, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::DepositCollateral{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn borrow_liquidity(
    manager_info: Pubkey,
    market_reserve_info: Pubkey,
    manager_token_account_info: Pubkey,
    liquidity_price_oracle_info: Pubkey,
    rate_oracle_info: Pubkey,
    user_obligatiton_info: Pubkey,
    user_authority_info: Pubkey,
    user_token_account_info: Pubkey,
    price_oracle_infos: Vec<Pubkey>,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_info, _bump_seed) = Pubkey::find_program_address(
        &[manager_info.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(manager_info, false),
        AccountMeta::new_readonly(manager_authority_info, true),
        AccountMeta::new(market_reserve_info, false),
        AccountMeta::new(manager_token_account_info, false),
        AccountMeta::new_readonly(liquidity_price_oracle_info, false),
        AccountMeta::new_readonly(rate_oracle_info, false),
        AccountMeta::new(user_obligatiton_info, false),
        AccountMeta::new_readonly(user_authority_info, true),
        AccountMeta::new(user_token_account_info, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    accounts.extend(
        price_oracle_infos
            .into_iter()
            .map(|price_oracle| AccountMeta::new_readonly(price_oracle, false))
    );

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::BorrowLiquidity{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn repay_loan(
    market_reserve_info: Pubkey,
    manager_token_account_info: Pubkey,
    rate_oracle_info: Pubkey,
    user_obligatiton_info: Pubkey,
    user_authority_info: Pubkey,
    user_token_account_info: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new(market_reserve_info, false),
            AccountMeta::new(manager_token_account_info, false),
            AccountMeta::new_readonly(rate_oracle_info, false),
            AccountMeta::new(user_obligatiton_info, false),
            AccountMeta::new_readonly(user_authority_info, true),
            AccountMeta::new(user_token_account_info, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::RepayLoan{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn redeem_collateral(
    manager_info: Pubkey,
    liquidity_market_reserve_info: Pubkey,
    price_oracle_info: Pubkey,
    rate_oracle_info: Pubkey,
    colleteral_market_reserve_info: Pubkey,
    manager_token_account_info: Pubkey,
    user_obligatiton_info: Pubkey,
    user_authority_info: Pubkey,
    user_token_account_info: Pubkey,
    price_oracle_infos: Vec<Pubkey>,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_info, _bump_seed) = Pubkey::find_program_address(
        &[manager_info.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(manager_info, false),
        AccountMeta::new_readonly(manager_authority_info, true),
        AccountMeta::new_readonly(liquidity_market_reserve_info, false),
        AccountMeta::new_readonly(price_oracle_info, false),
        AccountMeta::new_readonly(rate_oracle_info, false),
        AccountMeta::new(colleteral_market_reserve_info, false),
        AccountMeta::new(manager_token_account_info, false),
        AccountMeta::new(user_obligatiton_info, false),
        AccountMeta::new_readonly(user_authority_info, true),
        AccountMeta::new(user_token_account_info, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    accounts.extend(
        price_oracle_infos
            .into_iter()
            .map(|price_oracle| AccountMeta::new_readonly(price_oracle, false))
    );

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::RedeemCollateral{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn liquidate(
    manager_info: Pubkey,
    liquidity_market_reserve_info: Pubkey,
    manager_liquidity_token_account_info: Pubkey,
    price_oracle_info: Pubkey,
    rate_oracle_info: Pubkey,
    colleteral_market_reserve_info: Pubkey,
    manager_collateral_token_account_info: Pubkey,
    user_obligatiton_info: Pubkey,
    liquidator_authority_info: Pubkey,
    liquidator_liquidity_account_info: Pubkey,
    liquidator_collateral_account_info: Pubkey,
    price_oracle_infos: Vec<Pubkey>,
    is_arbitrary: bool,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_info, _bump_seed) = Pubkey::find_program_address(
        &[manager_info.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(manager_info, false),
        AccountMeta::new_readonly(manager_authority_info, true),
        AccountMeta::new(liquidity_market_reserve_info, false),
        AccountMeta::new(manager_liquidity_token_account_info, false),
        AccountMeta::new_readonly(price_oracle_info, false),
        AccountMeta::new_readonly(rate_oracle_info, false),
        AccountMeta::new(colleteral_market_reserve_info, false),
        AccountMeta::new(manager_collateral_token_account_info, false),
        AccountMeta::new(user_obligatiton_info, false),
        AccountMeta::new_readonly(liquidator_authority_info, true),
        AccountMeta::new(liquidator_liquidity_account_info, false),
        AccountMeta::new(liquidator_collateral_account_info, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    accounts.extend(
        price_oracle_infos
            .into_iter()
            .map(|price_oracle| AccountMeta::new_readonly(price_oracle, false))
    );

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::Liquidate{ is_arbitrary, amount }.pack(),
    }
}

pub fn feed_rate_oracle(
    rate_oracle_info: Pubkey,
    authority_info: Pubkey,
    interest_rate: u64,
    borrow_rate: u64,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new(rate_oracle_info, false),
            AccountMeta::new_readonly(authority_info, true),
        ],
        data: LendingInstruction::FeedRateOracle{ interest_rate, borrow_rate }.pack(),
    }
}

pub fn pause_rate_oracle(
    rate_oracle_info: Pubkey,
    authority_info: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new(rate_oracle_info, false),
            AccountMeta::new_readonly(authority_info, true),
        ],
        data: LendingInstruction::PauseRateOracle.pack(),
    }
}

pub fn add_liquidity_to_market_reserve(
    manager_info: Pubkey,
    market_reserve_info: Pubkey,
    rate_oracle_info: Pubkey,
    authority_info: Pubkey,
    liquidity_config: LiquidityConfig,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_info, false),
            AccountMeta::new(market_reserve_info, false),
            AccountMeta::new_readonly(rate_oracle_info, false),
            AccountMeta::new_readonly(authority_info, true),
        ],
        data: LendingInstruction::AddLiquidityToReserve{ liquidity_config }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn withdraw_fee(
    manager_info: Pubkey,
    market_reserve_info: Pubkey,
    manager_token_account_info: Pubkey,
    authority_info: Pubkey,
    receiver_token_account_info: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_info, _bump_seed) = Pubkey::find_program_address(
        &[manager_info.as_ref()],
        &program_id,
    );

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_info, false),
            AccountMeta::new_readonly(manager_authority_info, true),
            AccountMeta::new(market_reserve_info, false),
            AccountMeta::new(manager_token_account_info, false),
            AccountMeta::new_readonly(authority_info, true),
            AccountMeta::new(receiver_token_account_info, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::WithdrawFee{ amount }.pack(),
    }
}