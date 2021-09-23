//! Instruction types
#![allow(missing_docs)]
use crate::{
    id,
    error::LendingError,
    state::{
        IndexedCollateralConfig, IndexedLoanConfig,
        LiquidityConfig, CollateralConfig, RateModel,
    },
    oracle::{OracleConfig, OracleType},
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    msg,
    program_error::ProgramError,
    pubkey::{Pubkey, PUBKEY_BYTES},
    sysvar,
};
use typenum::Bit;
use std::{convert::TryInto, mem::size_of};

/// Instructions supported by the lending program.
#[derive(Clone, Debug, PartialEq)]
pub enum LendingInstruction {
    /// 0
    InitManager,
    /// 1
    InitMarketReserve {
        ///
        oracle_config: OracleConfig,
        ///
        collateral_config: CollateralConfig,
        ///
        liquidity_config: LiquidityConfig,
        ///
        rate_model: RateModel,
    },
    /// 2
    RefreshMarketReserves,
    /// 3
    Deposit {
        ///
        amount: u64,
    },
    /// 4
    Withdraw {
        ///
        amount: u64,
    },
    /// 5
    InitUserObligation,
    /// 6
    RefreshUserObligation,
    /// 7
    #[cfg(feature = "friend")]
    BindFriend,
    /// 8
    #[cfg(feature = "friend")]
    UnbindFriend,
    /// 9
    PledgeCollateral {
        ///
        amount: u64,
    },
    /// 10
    DepositAndPledge {
        ///
        amount: u64,
    },
    /// 11
    RedeemCollateral {
        ///
        amount: u64,
    },
    /// 12
    RedeemAndWithdraw {
        ///
        amount: u64,
    },
    /// 13
    RedeemCollateralWithoutLoan {
        ///
        amount: u64,
    },
    /// 14
    RedeemWithoutLoanAndWithdraw {
        ///
        amount: u64,
    },
    /// 15
    ReplaceCollateral {
        ///
        amount: u64,
    },
    /// 16
    BorrowLiquidity {
        ///
        amount: u64,
    },
    /// 17
    RepayLoan {
        ///
        amount: u64,
    },
    /// 18
    LiquidateByCollateral {
        ///
        amount: u64,
    },
    /// 19
    LiquidateByLoan {
        ///
        amount: u64,
    },
    /// 20
    FlashLiquidationByCollateral {
        ///
        tag: u8,
        ///
        amount: u64,
    },
    /// 21
    FlashLiquidationByLoan {
        ///
        tag: u8,
        ///
        amount: u64,
    },
    /// 22
    FlashLoan {
        ///
        tag: u8,
        ///
        amount: u64,
    },
    /// 23
    #[cfg(feature = "unique-credit")]
    InitUniqueCredit {
        ///
        authority: Pubkey,
        ///
        amount: u64,
    },
    /// 24
    #[cfg(feature = "unique-credit")]
    BorrowLiquidityByUniqueCredit {
        ///
        amount: u64,
    },
    /// 25
    #[cfg(feature = "unique-credit")]
    RepayLoanByUniqueCredit {
        ///
        amount: u64,
    },
    /// 26
    UpdateIndexedCollateralConfig {
        ///
        config: IndexedCollateralConfig,
    },
    /// 27
    UpdateIndexedLoanConfig {
        ///
        config: IndexedLoanConfig,
    },
    /// 28
    ControlMarketReserveLiquidity {
        ///
        enable: bool,
    },
    /// 29
    UpdateMarketReserveRateModel{
        ///
        model: RateModel,
    },
    /// 30
    UpdateMarketReserveCollateralConfig {
        ///
        config: CollateralConfig,
    },
    /// 31
    UpdateMarketReserveLiquidityConfig {
        ///
        config: LiquidityConfig,
    },
    /// 32
    UpdateMarketReserveOracleConfig {
        ///
        config: OracleConfig,
    },
    /// 33
    ReduceInsurance {
        ///
        amount: u64,
    },
    /// 34
    #[cfg(feature = "unique-credit")]
    UpdateUniqueCreditLimit{
        ///
        amount: u64,
    },
    /// 35
    #[cfg(feature = "general-test")]
    InjectNoBorrow,
    /// 36
    #[cfg(feature = "general-test")]
    InjectLiquidation,
    /// 37
    #[cfg(feature = "general-test")]
    CloseLendingAccount,
}

impl LendingInstruction {
    /// Unpacks a byte buffer into a [LendingInstruction](enum.LendingInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(LendingError::InstructionUnpackError)?;
        Ok(match tag {
            0 => Self::InitManager,
            1 => {
                let (oracle_config, rest) = Self::unpack_oracle_config(rest)?;
                let (collateral_config, rest) = Self::unpack_collateral_config(rest)?;
                let (liquidity_config, rest) = Self::unpack_liquidity_config(rest)?;
                let (rate_model, _rest) = Self::unpack_rate_model(rest)?;
                Self::InitMarketReserve { oracle_config, collateral_config, liquidity_config, rate_model }
            }
            2 => Self::RefreshMarketReserves,
            3 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::Deposit { amount }
            }
            4 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::Withdraw { amount }
            }
            5 => Self::InitUserObligation,
            6 => Self::RefreshUserObligation,
            #[cfg(feature = "friend")]
            7 => Self::BindFriend,
            #[cfg(feature = "friend")]
            8 => Self::UnbindFriend,
            9 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::PledgeCollateral { amount }
            }
            10 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::DepositAndPledge { amount }
            }
            11 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RedeemCollateral { amount }
            }
            12 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RedeemAndWithdraw { amount }
            }
            13 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RedeemCollateralWithoutLoan { amount }
            }
            14 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RedeemWithoutLoanAndWithdraw { amount }
            }
            15 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::ReplaceCollateral { amount }
            }
            16 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::BorrowLiquidity { amount }
            }
            17 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RepayLoan { amount }
            }
            18 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::LiquidateByCollateral { amount }
            }
            19 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::LiquidateByLoan { amount }
            }
            20 => {
                let (tag, rest) = Self::unpack_u8(rest)?;
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::FlashLiquidationByCollateral { tag, amount }
            }
            21 => {
                let (tag, rest) = Self::unpack_u8(rest)?;
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::FlashLiquidationByLoan { tag, amount }
            }
            22 => {
                let (tag, rest) = Self::unpack_u8(rest)?;
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::FlashLoan { tag, amount }
            }
            #[cfg(feature = "unique-credit")]
            23 => {
                let (authority, rest) = Self::unpack_pubkey(rest)?;
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::InitUniqueCredit { authority, amount }
            }
            #[cfg(feature = "unique-credit")]
            24 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::BorrowLiquidityByUniqueCredit { amount }
            }
            #[cfg(feature = "unique-credit")]
            25 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RepayLoanByUniqueCredit { amount }
            }
            26 => {
                let (config, _rest) = Self::unpack_indexed_collateral_config(rest)?;
                Self::UpdateIndexedCollateralConfig { config }
            }
            27 => {
                let (config, _rest) = Self::unpack_indexed_loan_config(rest)?;
                Self::UpdateIndexedLoanConfig { config }
            }
            28 => {
                let (enable, _rest) = Self::unpack_bool(rest)?;
                Self::ControlMarketReserveLiquidity { enable }
            }
            29 => {
                let (model, _rest) = Self::unpack_rate_model(rest)?;
                Self::UpdateMarketReserveRateModel { model }
            }
            30 => {
                let (config, _rest) = Self::unpack_collateral_config(rest)?;
                Self::UpdateMarketReserveCollateralConfig { config }
            }
            31 => {
                let (config, _rest) = Self::unpack_liquidity_config(rest)?;
                Self::UpdateMarketReserveLiquidityConfig { config }
            }
            32 => {
                let (config, _rest) = Self::unpack_oracle_config(rest)?;
                Self::UpdateMarketReserveOracleConfig { config }
            }
            33 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::ReduceInsurance { amount }
            }
            #[cfg(feature = "unique-credit")]
            34 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::UpdateUniqueCreditLimit { amount }
            }
            #[cfg(feature = "general-test")]
            35 => Self::InjectNoBorrow,
            #[cfg(feature = "general-test")]
            36 => Self::InjectLiquidation,
            #[cfg(feature = "general-test")]
            37 => Self::CloseLendingAccount,
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

    fn unpack_rate_model(input: &[u8]) -> Result<(RateModel, &[u8]), ProgramError> {
        let (a, rest) = Self::unpack_u64(input)?;
        let (c, rest) = Self::unpack_u64(rest)?;
        let (l_u, rest) = Self::unpack_u8(rest)?;
        let (k_u, rest) = Self::unpack_u128(rest)?;

        Ok((RateModel { a, c, l_u, k_u }, rest))
    }

    fn unpack_oracle_config(input: &[u8]) -> Result<(OracleConfig, &[u8]), ProgramError> {
        let (oracle, rest) = Self::unpack_pubkey(input)?;
        let (oracle_type, rest) = Self::unpack_u8(rest)?;
        
        Ok((OracleConfig { oracle, oracle_type: OracleType::from(oracle_type) }, rest))
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

        Ok((
            LiquidityConfig {
                close_ratio,
                borrow_tax_rate,
                flash_loan_fee_rate,
                max_deposit,
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

    /// Packs a [LendingInstruction](enum.LendingInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::InitManager => buf.push(0),
            Self::InitMarketReserve {
                oracle_config,
                collateral_config,
                liquidity_config,
                rate_model,
            } => {
                buf.push(1);
                Self::pack_oracle_config(oracle_config, &mut buf);
                Self::pack_collateral_config(collateral_config, &mut buf);
                Self::pack_liquidity_config(liquidity_config, &mut buf);
                Self::pack_rate_model(rate_model, &mut buf);
            }
            Self::RefreshMarketReserves => buf.push(2),
            Self::Deposit { amount } => {
                buf.push(3);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::Withdraw { amount } => {
                buf.push(4);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::InitUserObligation => buf.push(5),
            Self::RefreshUserObligation => buf.push(6),
            #[cfg(feature = "friend")]
            Self::BindFriend => buf.push(7),
            #[cfg(feature = "friend")]
            Self::UnbindFriend => buf.push(8),
            Self::PledgeCollateral { amount } => {
                buf.push(9);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::DepositAndPledge { amount } => {
                buf.push(10);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::RedeemCollateral { amount } => {
                buf.push(11);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::RedeemAndWithdraw { amount } => {
                buf.push(12);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::RedeemCollateralWithoutLoan { amount } => {
                buf.push(13);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::RedeemWithoutLoanAndWithdraw { amount } => {
                buf.push(14);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::ReplaceCollateral { amount } => {
                buf.push(15);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::BorrowLiquidity { amount } => {
                buf.push(16);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::RepayLoan { amount } => {
                buf.push(17);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::LiquidateByCollateral { amount } => {
                buf.push(18);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::LiquidateByLoan { amount } => {
                buf.push(19);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::FlashLiquidationByCollateral { tag, amount } => {
                buf.push(20);
                buf.extend_from_slice(&tag.to_le_bytes());
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::FlashLiquidationByLoan { tag, amount } => {
                buf.push(21);
                buf.extend_from_slice(&tag.to_le_bytes());
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::FlashLoan { tag, amount } => {
                buf.push(22);
                buf.extend_from_slice(&tag.to_le_bytes());
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            #[cfg(feature = "unique-credit")]
            Self::InitUniqueCredit { authority, amount } => {
                buf.push(23);
                buf.extend_from_slice(authority.as_ref());
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            #[cfg(feature = "unique-credit")]
            Self::BorrowLiquidityByUniqueCredit { amount } => {
                buf.push(24);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            #[cfg(feature = "unique-credit")]
            Self::RepayLoanByUniqueCredit { amount } => {
                buf.push(25);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::UpdateIndexedCollateralConfig { config } => {
                buf.push(26);
                buf.extend_from_slice(&config.index.to_le_bytes());
                buf.extend_from_slice(&config.borrow_value_ratio.to_le_bytes());
                buf.extend_from_slice(&config.liquidation_value_ratio.to_le_bytes());
            }
            Self::UpdateIndexedLoanConfig { config } => {
                buf.push(27);
                buf.extend_from_slice(&config.index.to_le_bytes());
                buf.extend_from_slice(&config.close_ratio.to_le_bytes());
            }
            Self::ControlMarketReserveLiquidity { enable } => {
                buf.push(28);
                buf.extend_from_slice(&(enable as u8).to_le_bytes());
            }
            Self::UpdateMarketReserveRateModel { model } => {
                buf.push(29);
                Self::pack_rate_model(model, &mut buf);
            }
            Self::UpdateMarketReserveCollateralConfig { config } => {
                buf.push(30);
                Self::pack_collateral_config(config, &mut buf);
            }
            Self::UpdateMarketReserveLiquidityConfig { config } => {
                buf.push(31);
                Self::pack_liquidity_config(config, &mut buf);
            }
            Self::UpdateMarketReserveOracleConfig { config } => {
                buf.push(32);
                Self::pack_oracle_config(config, &mut buf);
            }
            Self::ReduceInsurance { amount } => {
                buf.push(33);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            #[cfg(feature = "unique-credit")]
            Self::UpdateUniqueCreditLimit { amount } => {
                buf.push(34);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            #[cfg(feature = "general-test")]
            Self::InjectNoBorrow => buf.push(35),
            #[cfg(feature = "general-test")]
            Self::InjectLiquidation => buf.push(36),
            #[cfg(feature = "general-test")]
            Self::CloseLendingAccount => buf.push(37),
        }
        buf
    }

    fn pack_rate_model(model: RateModel, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&model.a.to_le_bytes());
        buf.extend_from_slice(&model.c.to_le_bytes());
        buf.extend_from_slice(&model.l_u.to_le_bytes());
        buf.extend_from_slice(&model.k_u.to_le_bytes());
    }

    fn pack_oracle_config(config: OracleConfig, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&config.oracle.as_ref());
        let oracle_type_u8: u8 = config.oracle_type.into();
        buf.extend_from_slice(&oracle_type_u8.to_le_bytes());
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
    }
}

pub fn init_manager(
    manager_key: Pubkey,
    authority_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new(manager_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::InitManager.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn init_market_reserve(
    manager_key: Pubkey,
    supply_token_account_key: Pubkey,
    market_reserve_key: Pubkey,
    token_mint_key: Pubkey,
    sotoken_mint_key: Pubkey,
    authority_key: Pubkey,
    oracle_config: OracleConfig,
    collateral_config: CollateralConfig,
    liquidity_config: LiquidityConfig,
    rate_model: RateModel,
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
            AccountMeta::new_readonly(token_mint_key, false),
            AccountMeta::new(sotoken_mint_key, false),
            AccountMeta::new_readonly(authority_key, true),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::InitMarketReserve{
            oracle_config,
            collateral_config,
            liquidity_config,
            rate_model,
        }.pack(),
    }
}

pub fn refresh_market_reserves(
    updating_keys: Vec<(Pubkey, Pubkey)>
) -> Instruction {
    let mut accounts = vec![AccountMeta::new_readonly(sysvar::clock::id(), false)];

    accounts.extend(
        updating_keys
            .into_iter()
            .map(|(market_reserve_key, price_oracle_key)|
                vec![
                    AccountMeta::new(market_reserve_key, false),
                    AccountMeta::new_readonly(price_oracle_key, false),
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
pub fn deposit_or_withdraw<IsDeposit: Bit>(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    supply_token_account_key: Pubkey,
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
            AccountMeta::new_readonly(user_authority_key, true),
            AccountMeta::new(user_token_account_key, false),
            AccountMeta::new(user_sotoken_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: if IsDeposit::BOOL {
            LendingInstruction::Deposit { amount }
        } else {
            LendingInstruction::Withdraw { amount }
        }.pack()
    }
}

pub fn init_user_obligation(
    manager_key: Pubkey,
    user_obligation_key: Pubkey,
    authority_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new_readonly(authority_key, true),
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

#[cfg(feature = "friend")]
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

#[cfg(feature = "friend")]
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
pub fn deposit_and_pledge(
    market_reserve_key: Pubkey,
    supply_token_account_key: Pubkey,
    user_obligation_key: Pubkey,
    user_authority_key: Pubkey,
    user_token_account_info: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new_readonly(user_authority_key, true),
            AccountMeta::new(user_token_account_info, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::DepositAndPledge{ amount }.pack(),
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
pub fn redeem_and_withdraw<WithLoan: Bit>(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    supply_token_account_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    user_authority_key: Pubkey,
    user_token_account_info: Pubkey,
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
        AccountMeta::new(user_token_account_info, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(friend_obligation_key) = friend_obligation_key {
        accounts.insert(6, AccountMeta::new_readonly(friend_obligation_key, false))
    }

    Instruction {
        program_id,
        accounts,
        data: if WithLoan::BOOL {
            LendingInstruction::RedeemAndWithdraw{ amount }
        } else {
            LendingInstruction::RedeemWithoutLoanAndWithdraw{ amount }
        }.pack(),
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
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new_readonly(user_authority_key, true),
            AccountMeta::new(user_token_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::RepayLoan{ amount }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn liquidate<IsCollateral: Bit>(
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
        data: if IsCollateral::BOOL {
            LendingInstruction::LiquidateByCollateral{ amount }
        } else {
            LendingInstruction::LiquidateByLoan{ amount }
        }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn flash_liquidation<IsCollateral: Bit>(
    manager_key: Pubkey,
    collateral_market_reserve_key: Pubkey,
    collateral_supply_account_key: Pubkey,
    loan_market_reserve_key: Pubkey,
    loan_supply_account_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    liquidator_authority_key: Pubkey,
    liquidator_program_id: Pubkey,
    liquidator_program_accounts: Vec<AccountMeta>,
    amount: u64,
    tag: u8,
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
        AccountMeta::new_readonly(liquidator_authority_key, true),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(liquidator_program_id, false),
    ];

    if let Some(friend_obligation_key) = friend_obligation_key {
        accounts.insert(8, AccountMeta::new_readonly(friend_obligation_key, false))
    }
    accounts.extend(liquidator_program_accounts);

    Instruction {
        program_id,
        accounts,
        data: if IsCollateral::BOOL {
            LendingInstruction::FlashLiquidationByCollateral{ tag, amount }
        } else {
            LendingInstruction::FlashLiquidationByLoan{ tag, amount }
        }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn flash_loan(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    supply_token_account_key: Pubkey,
    receiver_authority_key: Pubkey,
    receiver_program_id: Pubkey,
    receiver_program_accounts: Vec<AccountMeta>,
    tag: u8,
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
        AccountMeta::new_readonly(receiver_authority_key, true),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(receiver_program_id, false),
    ];

    accounts.extend(receiver_program_accounts);

    Instruction {
        program_id,
        accounts,
        data: LendingInstruction::FlashLoan{ tag, amount }.pack(),
    }
}

#[cfg(feature = "unique-credit")]
pub fn init_unique_credit(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    unique_credit_key: Pubkey,
    authority_key: Pubkey,
    credit_authority_key: Pubkey,
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
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new_readonly(manager_authority_key, false),
            AccountMeta::new_readonly(market_reserve_key, false),
            AccountMeta::new(unique_credit_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::InitUniqueCredit{
            authority: credit_authority_key,
            amount,
        }.pack(),
    }
}

#[cfg(feature = "unique-credit")]
pub fn borrow_liquidity_by_unique_credit(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    supply_token_account_key: Pubkey,
    unique_credit_key: Pubkey,
    authority_key: Pubkey,
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
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new(unique_credit_key, false),
            AccountMeta::new_readonly(authority_key, true),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::BorrowLiquidityByUniqueCredit{ amount }.pack(),
    }
}

#[cfg(feature = "unique-credit")]
pub fn repay_loan_by_unique_credit(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    supply_token_account_key: Pubkey,
    unique_credit_key: Pubkey,
    source_token_account_key: Pubkey,
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
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new(unique_credit_key, false),
            AccountMeta::new(source_token_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::RepayLoanByUniqueCredit{ amount }.pack(),
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

pub fn update_market_reserve_rate_model(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    authority_key: Pubkey,
    model: RateModel,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::UpdateMarketReserveRateModel{ model }.pack(),
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

pub fn update_market_reserve_oracle_config(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    authority_key: Pubkey,
    config: OracleConfig,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::UpdateMarketReserveOracleConfig{ config }.pack(),
    }
}

#[cfg(feature = "unique-credit")]
pub fn update_unique_credit_limit(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    unique_credit_key: Pubkey,
    authority_key: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new_readonly(market_reserve_key, false),
            AccountMeta::new(unique_credit_key, false),
            AccountMeta::new_readonly(authority_key, true),
        ],
        data: LendingInstruction::UpdateUniqueCreditLimit{ amount }.pack(),
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

#[cfg(feature = "general-test")]
pub fn inject_no_borrow(
    user_obligation_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![AccountMeta::new(user_obligation_key, false)],
        data: LendingInstruction::InjectNoBorrow.pack(),
    }
}

#[cfg(feature = "general-test")]
pub fn inject_liquidation(
    user_obligation_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![AccountMeta::new(user_obligation_key, false)],
        data: LendingInstruction::InjectLiquidation.pack(),
    }
}

#[cfg(feature = "general-test")]
pub fn close_lending_account(
    source_account_key: Pubkey,
    dest_account_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new(source_account_key, false),
            AccountMeta::new(dest_account_key, false),
        ],
        data: LendingInstruction::CloseLendingAccount.pack(),
    }
}