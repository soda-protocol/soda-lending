//! Instruction types
#![allow(missing_docs)]
use crate::{
    id, error::LendingError,
    state::{
        IndexedCollateralConfig, IndexedLoanConfig,
        RateOracleConfig, LiquidityConfig, CollateralConfig,
    },
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    msg,
    program_error::ProgramError,
    pubkey::{Pubkey, PUBKEY_BYTES},
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
    InitRateOracle {
        ///
        config: RateOracleConfig,
    },
    /// 2
    InitMarketReserve {
        ///
        collateral_config: CollateralConfig,
        ///
        liquidity_config: LiquidityConfig,
    },
    /// 3
    RefreshMarketReserves,
    /// 4
    Deposit {
        ///
        amount: u64,
    },
    /// 5
    Withdraw {
        ///
        amount: u64,
    },
    /// 6
    InitUserObligation,
    /// 7
    RefreshUserObligation,
    /// 8
    BindFriend,
    /// 9
    UnbindFriend,
    /// 10
    PledgeCollateral {
        ///
        amount: u64,
    },
    /// 11
    RedeemCollateral {
        ///
        amount: u64,
    },
    /// 12
    RedeemCollateralWithoutLoan {
        ///
        amount: u64,
    },
    /// 13
    ReplaceCollateral {
        ///
        amount: u64,
    },
    /// 14
    BorrowLiquidity {
        ///
        amount: u64,
    },
    /// 15
    RepayLoan {
        ///
        amount: u64,
    },
    /// 16
    LiquidateByCollateral {
        ///
        amount: u64,
    },
    /// 17
    LiquidateByLoan {
        ///
        amount: u64,
    },
    /// 18
    FlashLiquidationByCollateral {
        ///
        amount: u64,
    },
    /// 19
    FlashLiquidationByLoan {
        ///
        amount: u64,
    },
    /// 20
    FlashLoan {
        ///
        amount: u64,
    },
    /// 21
    UpdateIndexedCollateralConfig {
        ///
        config: IndexedCollateralConfig,
    },
    /// 22
    UpdateIndexedLoanConfig {
        ///
        config: IndexedLoanConfig,
    },
    /// 23
    PauseRateOracle,
    /// 24
    UpdateRateOracleConfig{
        ///
        config: RateOracleConfig,
    },
    /// 25
    ControlMarketReserveLiquidity {
        ///
        enable: bool,
    },
    /// 26
    UpdateMarketReserveCollateralConfig {
        ///
        config: CollateralConfig,
    },
    /// 27
    UpdateMarketReserveLiquidityConfig {
        ///
        config: LiquidityConfig,
    },
    /// 28
    UpdateMarketReservePriceOracle {
        ///
        oracle: Pubkey,
    },
    /// 29
    UpdateMarketReserveRateOracle {
        ///
        oracle: Pubkey,
    },
    /// 30
    ReduceInsurance {
        ///
        amount: u64,
    },
    /// 31
    #[cfg(feature = "case-injection")]
    InjectNoBorrow,
    /// 32
    #[cfg(feature = "case-injection")]
    InjectLiquidation,
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
            1 => {
                let (config, _rest) = Self::unpack_rate_oracle_config(rest)?;
                Self::InitRateOracle { config }
            }
            2 => {
                let (collateral_config, rest) = Self::unpack_collateral_config(rest)?;
                let (liquidity_config, _rest) = Self::unpack_liquidity_config(rest)?;
                Self::InitMarketReserve { collateral_config, liquidity_config }
            }
            3 => Self::RefreshMarketReserves,
            4 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::Deposit { amount }
            }
            5 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::Withdraw { amount }
            }
            6 => Self::InitUserObligation,
            7 => Self::RefreshUserObligation,
            8 => Self::BindFriend,
            9 => Self::UnbindFriend,
            10 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::PledgeCollateral { amount }
            }
            11 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RedeemCollateral { amount }
            }
            12 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RedeemCollateralWithoutLoan { amount }
            }
            13 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::ReplaceCollateral { amount }
            }
            14 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::BorrowLiquidity { amount }
            }
            15 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RepayLoan { amount }
            }
            16 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::LiquidateByCollateral { amount }
            }
            17 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::LiquidateByLoan { amount }
            }
            18 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::FlashLiquidationByCollateral { amount }
            }
            19 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::FlashLiquidationByLoan { amount }
            }
            20 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::FlashLoan { amount }
            }
            21 => {
                let (config, _rest) = Self::unpack_indexed_collateral_config(rest)?;
                Self::UpdateIndexedCollateralConfig { config }
            }
            22 => {
                let (config, _rest) = Self::unpack_indexed_loan_config(rest)?;
                Self::UpdateIndexedLoanConfig { config }
            }
            23 => Self::PauseRateOracle,
            24 => {
                let (config, _rest) = Self::unpack_rate_oracle_config(rest)?;
                Self::UpdateRateOracleConfig { config } 
            }
            25 => {
                let (enable, _rest) = Self::unpack_bool(rest)?;
                Self::ControlMarketReserveLiquidity { enable }
            }
            26 => {
                let (config, _rest) = Self::unpack_collateral_config(rest)?;
                Self::UpdateMarketReserveCollateralConfig { config }
            }
            27 => {
                let (config, _rest) = Self::unpack_liquidity_config(rest)?;
                Self::UpdateMarketReserveLiquidityConfig { config }
            }
            28 => {
                let (oracle, _rest) = Self::unpack_pubkey(rest)?;
                Self::UpdateMarketReservePriceOracle { oracle }
            }
            29 => {
                let (oracle, _rest) = Self::unpack_pubkey(rest)?;
                Self::UpdateMarketReserveRateOracle { oracle }
            }
            30 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::ReduceInsurance { amount }
            }
            #[cfg(feature = "case-injection")]
            31 => Self::InjectNoBorrow,
            #[cfg(feature = "case-injection")]
            32 => Self::InjectLiquidation,
            _ => {
                msg!("Instruction cannot be unpacked");
                return Err(LendingError::InstructionUnpackError.into());
            }
        })
    }

    fn unpack_indexed_collateral_config(input: &[u8]) -> Result<(IndexedCollateralConfig, &[u8]), ProgramError> {
        let (index, rest) = Self::unpack_u8(input)?;
        let (borrow_value_ratio, rest) = Self::unpack_u8(rest)?;
        let (liquidation_value_ratio, rest) = Self::unpack_u8(rest)?;

        Ok((IndexedCollateralConfig { index, borrow_value_ratio, liquidation_value_ratio }, rest))
    }

    fn unpack_indexed_loan_config(input: &[u8]) -> Result<(IndexedLoanConfig, &[u8]), ProgramError> {
        let (index, rest) = Self::unpack_u8(input)?;
        let (close_ratio, rest) = Self::unpack_u8(rest)?;

        Ok((IndexedLoanConfig { index, close_ratio }, rest))
    }

    fn unpack_rate_oracle_config(input: &[u8]) -> Result<(RateOracleConfig, &[u8]), ProgramError> {
        let (a, rest) = Self::unpack_u64(input)?;
        let (c, rest) = Self::unpack_u64(rest)?;
        let (l_u, rest) = Self::unpack_u8(rest)?;
        let (k_u, rest) = Self::unpack_u128(rest)?;

        Ok((RateOracleConfig { a, c, l_u, k_u }, rest))
    }

    fn unpack_collateral_config(input: &[u8]) -> Result<(CollateralConfig, &[u8]), ProgramError> {
        let (borrow_value_ratio, rest) = Self::unpack_u8(input)?;
        let (liquidation_value_ratio, rest) = Self::unpack_u8(rest)?;
        let (liquidation_penalty_ratio, rest) = Self::unpack_u8(rest)?;

        Ok((CollateralConfig { borrow_value_ratio, liquidation_value_ratio, liquidation_penalty_ratio }, rest))
    }

    fn unpack_liquidity_config(input: &[u8]) -> Result<(LiquidityConfig, &[u8]), ProgramError> {
        let (close_ratio, rest) = Self::unpack_u8(input)?;
        let (borrow_tax_rate, rest) = Self::unpack_u8(rest)?;
        let (flash_loan_fee_rate, rest) = Self::unpack_u64(rest)?;
        let (max_deposit, rest) = Self::unpack_u64(rest)?;
        let (max_acc_deposit, rest) = Self::unpack_u64(rest)?;

        Ok((
            LiquidityConfig {
                close_ratio,
                borrow_tax_rate,
                flash_loan_fee_rate,
                max_deposit,
                max_acc_deposit,
            }, rest
        ))
    }

    fn unpack_pubkey(input: &[u8]) -> Result<(Pubkey, &[u8]), ProgramError> {
        if input.len() < PUBKEY_BYTES {
            msg!("Pubkey cannot be unpacked");
            return Err(LendingError::InstructionUnpackError.into());
        }
        let (key, rest) = input.split_at(PUBKEY_BYTES);
        let pk = Pubkey::new(key);
        Ok((pk, rest))
    }

    fn unpack_u128(input: &[u8]) -> Result<(u128, &[u8]), ProgramError> {
        if input.len() < 16 {
            msg!("u128 cannot be unpacked");
            return Err(LendingError::InstructionUnpackError.into());
        }
        let (amount, rest) = input.split_at(16);
        let amount = amount
            .get(..16)
            .and_then(|slice| slice.try_into().ok())
            .map(u128::from_le_bytes)
            .ok_or(LendingError::InstructionUnpackError)?;
        Ok((amount, rest))
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
            Self::InitRateOracle { config } => {
                buf.push(1);
                Self::pack_rate_oracle_config(config, &mut buf);
            }
            Self::InitMarketReserve {
                collateral_config,
                liquidity_config,
            } => {
                buf.push(2);
                Self::pack_collateral_config(collateral_config, &mut buf);
                Self::pack_liquidity_config(liquidity_config, &mut buf);
            }
            Self::RefreshMarketReserves => buf.push(3),
            Self::Deposit { amount } => {
                buf.push(4);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::Withdraw { amount } => {
                buf.push(5);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::InitUserObligation => buf.push(6),
            Self::RefreshUserObligation => buf.push(7),
            Self::BindFriend => buf.push(8),
            Self::UnbindFriend => buf.push(9),
            Self::PledgeCollateral { amount } => {
                buf.push(10);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::RedeemCollateral { amount } => {
                buf.push(11);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::RedeemCollateralWithoutLoan { amount } => {
                buf.push(12);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::ReplaceCollateral { amount } => {
                buf.push(13);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::BorrowLiquidity { amount } => {
                buf.push(14);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::RepayLoan { amount } => {
                buf.push(15);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::LiquidateByCollateral { amount } => {
                buf.push(16);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::LiquidateByLoan { amount } => {
                buf.push(17);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::FlashLiquidationByCollateral { amount } => {
                buf.push(18);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::FlashLiquidationByLoan { amount } => {
                buf.push(19);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::FlashLoan { amount } => {
                buf.push(20);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::UpdateIndexedCollateralConfig { config } => {
                buf.push(21);
                buf.extend_from_slice(&config.index.to_le_bytes());
                buf.extend_from_slice(&config.borrow_value_ratio.to_le_bytes());
                buf.extend_from_slice(&config.liquidation_value_ratio.to_le_bytes());
            }
            Self::UpdateIndexedLoanConfig { config } => {
                buf.push(22);
                buf.extend_from_slice(&config.index.to_le_bytes());
                buf.extend_from_slice(&config.close_ratio.to_le_bytes());
            }
            Self::PauseRateOracle => buf.push(23),
            Self::UpdateRateOracleConfig { config } => {
                buf.push(24);
                Self::pack_rate_oracle_config(config, &mut buf);
            }
            Self::ControlMarketReserveLiquidity { enable } => {
                buf.push(25);
                buf.extend_from_slice(&(enable as u8).to_le_bytes());
            }
            Self::UpdateMarketReserveCollateralConfig { config } => {
                buf.push(26);
                Self::pack_collateral_config(config, &mut buf);
            }
            Self::UpdateMarketReserveLiquidityConfig { config } => {
                buf.push(27);
                Self::pack_liquidity_config(config, &mut buf);
            }
            Self::UpdateMarketReservePriceOracle { oracle } => {
                buf.push(28);
                buf.extend_from_slice(oracle.as_ref());
            }
            Self::UpdateMarketReserveRateOracle { oracle } => {
                buf.push(29);
                buf.extend_from_slice(oracle.as_ref());
            }
            Self::ReduceInsurance { amount } => {
                buf.push(30);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            #[cfg(feature = "case-injection")]
            Self::InjectNoBorrow => buf.push(31),
            #[cfg(feature = "case-injection")]
            Self::InjectLiquidation => buf.push(32),
        }
        buf
    }

    fn pack_rate_oracle_config(config: RateOracleConfig, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&config.a.to_le_bytes());
        buf.extend_from_slice(&config.c.to_le_bytes());
        buf.extend_from_slice(&config.l_u.to_le_bytes());
        buf.extend_from_slice(&config.k_u.to_le_bytes());
    }

    fn pack_collateral_config(config: CollateralConfig, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&config.borrow_value_ratio.to_le_bytes());
        buf.extend_from_slice(&config.liquidation_value_ratio.to_le_bytes());
        buf.extend_from_slice(&config.liquidation_penalty_ratio.to_le_bytes());
    }

    fn pack_liquidity_config(config: LiquidityConfig, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&config.close_ratio.to_le_bytes());
        buf.extend_from_slice(&config.borrow_tax_rate.to_le_bytes());
        buf.extend_from_slice(&config.flash_loan_fee_rate.to_le_bytes());
        buf.extend_from_slice(&config.max_deposit.to_le_bytes());
        buf.extend_from_slice(&config.max_acc_deposit.to_le_bytes());
    }
}

pub fn init_manager(
    manager_key: Pubkey,
    owner_key: Pubkey,
    oracle_program_id: Pubkey,
    quote_currency: [u8; 32],
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new(manager_key, false),
            AccountMeta::new_readonly(owner_key, false),
            AccountMeta::new_readonly(oracle_program_id, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::InitManager { quote_currency }.pack(),
    }
}

pub fn init_rate_oracle(
    rate_oracle_key: Pubkey,
    owner_key: Pubkey,
    config: RateOracleConfig,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new(rate_oracle_key, false),
            AccountMeta::new_readonly(owner_key, false),
        ],
        data: LendingInstruction::InitRateOracle { config }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn init_market_reserve(
    manager_key: Pubkey,
    supply_token_account_key: Pubkey,
    market_reserve_key: Pubkey,
    pyth_product_key: Pubkey,
    pyth_price_key: Pubkey,
    rate_oracle_key: Pubkey,
    token_mint_key: Pubkey,
    sotoken_mint_key: Pubkey,
    authority_key: Pubkey,
    collateral_config: CollateralConfig,
    liquidity_config: LiquidityConfig,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new_readonly(manager_authority_key, false),
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new_readonly(pyth_product_key, false),
            AccountMeta::new_readonly(pyth_price_key, false),
            AccountMeta::new_readonly(rate_oracle_key, false),
            AccountMeta::new_readonly(token_mint_key, false),
            AccountMeta::new(sotoken_mint_key, false),
            AccountMeta::new_readonly(authority_key, true),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::InitMarketReserve{
            collateral_config,
            liquidity_config,
        }.pack(),
    }
}

pub fn refresh_market_reserves(
    updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>
) -> Instruction {
    let mut accounts = vec![AccountMeta::new_readonly(sysvar::clock::id(), false)];

    accounts.extend(
        updating_keys
            .into_iter()
            .map(|(market_reserve_key, pyth_price_key, rate_oracle_key)|
                vec![
                    AccountMeta::new(market_reserve_key, false),
                    AccountMeta::new_readonly(pyth_price_key, false),
                    AccountMeta::new_readonly(rate_oracle_key, false),
                ]
            )
            .flatten()
    );

    Instruction {
        program_id: id(),
        accounts,
        data: LendingInstruction::RefreshMarketReserves.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn deposit(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    supply_token_account_key: Pubkey,
    rate_oracle_key: Pubkey,
    user_authority_key: Pubkey,
    user_token_account_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new_readonly(manager_authority_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new(sotoken_mint_key, false),
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new_readonly(rate_oracle_key, false),
            AccountMeta::new_readonly(user_authority_key, true),
            AccountMeta::new(user_token_account_key, false),
            AccountMeta::new(user_sotoken_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::Deposit { amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn withdraw(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    supply_token_account_key: Pubkey,
    rate_oracle_key: Pubkey,
    user_authority_key: Pubkey,
    user_token_account_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new_readonly(manager_authority_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new(sotoken_mint_key, false),
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new_readonly(rate_oracle_key, false),
            AccountMeta::new_readonly(user_authority_key, true),
            AccountMeta::new(user_token_account_key, false),
            AccountMeta::new(user_sotoken_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::Withdraw { amount }.pack(),
    }
}

pub fn init_user_obligation(
    manager_key: Pubkey,
    user_obligation_key: Pubkey,
    owner_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new_readonly(owner_key, false),
        ],
        data: LendingInstruction::InitUserObligation.pack(),
    }
}

pub fn refresh_user_obligation(
    user_obligation_key: Pubkey,
    market_reserve_keys: Vec<Pubkey>,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new(user_obligation_key, false),
    ];

    accounts.extend(
        market_reserve_keys
            .into_iter()
            .map(|market_reserve_key| AccountMeta::new_readonly(market_reserve_key, false))
    );

    Instruction {
        program_id: id(),
        accounts,
        data: LendingInstruction::RefreshUserObligation.pack(),
    }
}

pub fn bind_friend(
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    user_authority_key: Pubkey,
    friend_authority_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new(friend_obligation_key, false),
            AccountMeta::new_readonly(user_authority_key, true),
            AccountMeta::new_readonly(friend_authority_key, true),
        ],
        data: LendingInstruction::BindFriend.pack(),
    }
}

pub fn unbind_friend(
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    user_authority_key: Pubkey,
    friend_authority_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new(friend_obligation_key, false),
            AccountMeta::new_readonly(user_authority_key, true),
            AccountMeta::new_readonly(friend_authority_key, true),
        ],
        data: LendingInstruction::UnbindFriend.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn pledge_collateral(
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    user_authority_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(market_reserve_key, false),
            AccountMeta::new(sotoken_mint_key, false),
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new_readonly(user_authority_key, true),
            AccountMeta::new(user_sotoken_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::PledgeCollateral{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn redeem_collateral(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    user_authority_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(manager_key, false),
        AccountMeta::new_readonly(manager_authority_key, false),
        AccountMeta::new_readonly(market_reserve_key, false),
        AccountMeta::new(sotoken_mint_key, false),
        AccountMeta::new(user_obligation_key, false),
        AccountMeta::new_readonly(user_authority_key, true),
        AccountMeta::new(user_sotoken_account_key, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(friend_obligation_key) = friend_obligation_key {
        accounts.insert(6, AccountMeta::new_readonly(friend_obligation_key, false))
    }

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::RedeemCollateral{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn redeem_collateral_without_loan(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    user_authority_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(manager_key, false),
        AccountMeta::new_readonly(manager_authority_key, false),
        AccountMeta::new_readonly(market_reserve_key, false),
        AccountMeta::new(sotoken_mint_key, false),
        AccountMeta::new(user_obligation_key, false),
        AccountMeta::new_readonly(user_authority_key, true),
        AccountMeta::new(user_sotoken_account_key, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(friend_obligation_key) = friend_obligation_key {
        accounts.insert(5, AccountMeta::new_readonly(friend_obligation_key, false))
    }

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::RedeemCollateralWithoutLoan{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn replace_collateral(
    manager_key: Pubkey,
    out_market_reserve_key: Pubkey,
    out_sotoken_mint_key: Pubkey,
    in_market_reserve_key: Pubkey,
    in_sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    user_authority_key: Pubkey,
    user_out_sotoken_account_key: Pubkey,
    user_in_sotoken_account_key: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(manager_key, false),
        AccountMeta::new_readonly(manager_authority_key, false),
        AccountMeta::new_readonly(out_market_reserve_key, false),
        AccountMeta::new(out_sotoken_mint_key, false),
        AccountMeta::new_readonly(in_market_reserve_key, false),
        AccountMeta::new(in_sotoken_mint_key, false),
        AccountMeta::new(user_obligation_key, false),
        AccountMeta::new_readonly(user_authority_key, true),
        AccountMeta::new(user_out_sotoken_account_key, false),
        AccountMeta::new(user_in_sotoken_account_key, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(friend_obligation_key) = friend_obligation_key {
        accounts.insert(8, AccountMeta::new_readonly(friend_obligation_key, false))
    }

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::ReplaceCollateral{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn borrow_liquidity(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    supply_token_account_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    user_authority_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(manager_key, false),
        AccountMeta::new_readonly(manager_authority_key, false),
        AccountMeta::new(market_reserve_key, false),
        AccountMeta::new(supply_token_account_key, false),
        AccountMeta::new(user_obligation_key, false),
        AccountMeta::new_readonly(user_authority_key, true),
        AccountMeta::new(user_token_account_key, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(friend_obligation_key) = friend_obligation_key {
        accounts.insert(6, AccountMeta::new_readonly(friend_obligation_key, false))
    }

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::BorrowLiquidity{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn repay_loan(
    market_reserve_key: Pubkey,
    supply_token_account_key: Pubkey,
    rate_oracle_key: Pubkey,
    user_obligation_key: Pubkey,
    user_authority_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new_readonly(rate_oracle_key, false),
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new_readonly(user_authority_key, true),
            AccountMeta::new(user_token_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::RepayLoan{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn liquidate_by_collateral(
    manager_key: Pubkey,
    collateral_market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    loan_market_reserve_key: Pubkey,
    supply_token_account_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    liquidator_authority_key: Pubkey,
    liquidator_token_account_key: Pubkey,
    liquidator_sotoken_account_key: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(manager_key, false),
        AccountMeta::new_readonly(manager_authority_key, false),
        AccountMeta::new_readonly(collateral_market_reserve_key, false),
        AccountMeta::new(sotoken_mint_key, false),
        AccountMeta::new(loan_market_reserve_key, false),
        AccountMeta::new(supply_token_account_key, false),
        AccountMeta::new(user_obligation_key, false),
        AccountMeta::new_readonly(liquidator_authority_key, true),
        AccountMeta::new(liquidator_token_account_key, false),
        AccountMeta::new(liquidator_sotoken_account_key, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(friend_obligation_key) = friend_obligation_key {
        accounts.insert(8, AccountMeta::new_readonly(friend_obligation_key, false))
    }

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::LiquidateByCollateral{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn liquidate_by_loan(
    manager_key: Pubkey,
    collateral_market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    loan_market_reserve_key: Pubkey,
    supply_token_account_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    liquidator_authority_key: Pubkey,
    liquidator_token_account_key: Pubkey,
    liquidator_sotoken_account_key: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(manager_key, false),
        AccountMeta::new_readonly(manager_authority_key, false),
        AccountMeta::new_readonly(collateral_market_reserve_key, false),
        AccountMeta::new(sotoken_mint_key, false),
        AccountMeta::new(loan_market_reserve_key, false),
        AccountMeta::new(supply_token_account_key, false),
        AccountMeta::new(user_obligation_key, false),
        AccountMeta::new_readonly(liquidator_authority_key, true),
        AccountMeta::new(liquidator_token_account_key, false),
        AccountMeta::new(liquidator_sotoken_account_key, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(friend_obligation_key) = friend_obligation_key {
        accounts.insert(8, AccountMeta::new_readonly(friend_obligation_key, false))
    }

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::LiquidateByLoan{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn flash_liquidation_by_collateral(
    manager_key: Pubkey,
    collateral_market_reserve_key: Pubkey,
    collateral_supply_account_key: Pubkey,
    loan_market_reserve_key: Pubkey,
    loan_supply_account_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    liquidator_token_account_key: Pubkey,
    flash_liquidation_program_accounts: Vec<AccountMeta>,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(manager_key, false),
        AccountMeta::new_readonly(manager_authority_key, false),
        AccountMeta::new(collateral_market_reserve_key, false),
        AccountMeta::new(collateral_supply_account_key, false),
        AccountMeta::new(loan_market_reserve_key, false),
        AccountMeta::new(loan_supply_account_key, false),
        AccountMeta::new(user_obligation_key, false),
        AccountMeta::new(liquidator_token_account_key, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(friend_obligation_key) = friend_obligation_key {
        accounts.insert(8, AccountMeta::new_readonly(friend_obligation_key, false))
    }
    accounts.extend(flash_liquidation_program_accounts);

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::FlashLiquidationByCollateral{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn flash_liquidation_by_loan(
    manager_key: Pubkey,
    collateral_market_reserve_key: Pubkey,
    collateral_supply_account_key: Pubkey,
    loan_market_reserve_key: Pubkey,
    loan_supply_account_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    liquidator_token_account_key: Pubkey,
    flash_liquidation_program_accounts: Vec<AccountMeta>,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(manager_key, false),
        AccountMeta::new_readonly(manager_authority_key, false),
        AccountMeta::new(collateral_market_reserve_key, false),
        AccountMeta::new(collateral_supply_account_key, false),
        AccountMeta::new(loan_market_reserve_key, false),
        AccountMeta::new(loan_supply_account_key, false),
        AccountMeta::new(user_obligation_key, false),
        AccountMeta::new(liquidator_token_account_key, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(friend_obligation_key) = friend_obligation_key {
        accounts.insert(8, AccountMeta::new_readonly(friend_obligation_key, false))
    }
    accounts.extend(flash_liquidation_program_accounts);

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::FlashLiquidationByLoan{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn flash_loan(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    supply_token_account_info: Pubkey,
    receiver_token_account_key: Pubkey,
    flash_loan_program_accounts: Vec<AccountMeta>,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    let mut accounts = vec![
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(manager_key, false),
        AccountMeta::new_readonly(manager_authority_key, false),
        AccountMeta::new(market_reserve_key, false),
        AccountMeta::new(supply_token_account_info, false),
        AccountMeta::new(receiver_token_account_key, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    accounts.extend(flash_loan_program_accounts);

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::FlashLiquidationByLoan{ amount }.pack(),
    }
}

pub fn update_user_obligation_collateral_config(
    manager_key: Pubkey,
    user_obligation_key: Pubkey,
    authority_key: Pubkey,
    config: IndexedCollateralConfig,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::UpdateIndexedCollateralConfig{ config }.pack(),
    }
}

pub fn update_user_obligation_loan_config(
    manager_key: Pubkey,
    user_obligation_key: Pubkey,
    authority_key: Pubkey,
    config: IndexedLoanConfig,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::UpdateIndexedLoanConfig{ config }.pack(),
    }
}

pub fn pause_rate_oracle(
    rate_oracle_key: Pubkey,
    authority_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new(rate_oracle_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::PauseRateOracle.pack(),
    }
}

pub fn update_rate_oracle_config(
    rate_oracle_key: Pubkey,
    authority_key: Pubkey,
    config: RateOracleConfig,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new(rate_oracle_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::UpdateRateOracleConfig{ config }.pack(),
    }
}

pub fn control_market_reserve_liquidity(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    authority_key: Pubkey,
    enable: bool,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::ControlMarketReserveLiquidity{ enable }.pack(),
    }
}

pub fn update_market_reserve_collateral_config(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    authority_key: Pubkey,
    config: CollateralConfig,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::UpdateMarketReserveCollateralConfig{ config }.pack(),
    }
}

pub fn update_market_reserve_liquidity_config(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    authority_key: Pubkey,
    config: LiquidityConfig,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::UpdateMarketReserveLiquidityConfig{ config }.pack(),
    }
}

pub fn update_market_reserve_price_oracle(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    authority_key: Pubkey,
    oracle: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::UpdateMarketReservePriceOracle{ oracle }.pack(),
    }
}

pub fn update_market_reserve_rate_oracle(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    authority_key: Pubkey,
    oracle: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::UpdateMarketReserveRateOracle{ oracle }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn reduce_insurance(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    supply_token_account_key: Pubkey,
    authority_key: Pubkey,
    receiver_token_account_key: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new_readonly(manager_authority_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new_readonly(authority_key, true),
            AccountMeta::new(receiver_token_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::ReduceInsurance{ amount }.pack(),
    }
}

#[cfg(feature = "case-injection")]
pub fn inject_no_borrow(
    user_obligation_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![AccountMeta::new(user_obligation_key, false)],
        data: LendingInstruction::InjectNoBorrow.pack(),
    }
}

#[cfg(feature = "case-injection")]
pub fn inject_liquidation(
    user_obligation_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![AccountMeta::new(user_obligation_key, false)],
        data: LendingInstruction::InjectLiquidation.pack(),
    }
}