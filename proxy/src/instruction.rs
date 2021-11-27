#![allow(missing_docs)]
use std::{convert::TryInto, mem::size_of};
use solana_program::{
    msg,
    program_error::ProgramError,
    pubkey::{Pubkey, PUBKEY_BYTES},
    instruction::{Instruction, AccountMeta},
    sysvar,
    system_program,
};
use soda_lending::{
    state::{CollateralConfig, LiquidityConfig, RateModel},
    oracle::{OracleConfig, OracleType},
};
use spl_associated_token_account::get_associated_token_address;
use crate::{id, error::ProxyError};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RouterSwapInput {
    ///
    pub router_num: u8,
    ///
    pub amount_in: u64,
    ///
    pub minimum_amount_out: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SwapInput {
    ///
    pub amount_in: u64,
    ///
    pub minimum_amount_out: u64,
}

pub enum ProxyInstruction {
    /// 0
    CreateManager,
    /// 1
    CreateMarketReserve(OracleConfig, CollateralConfig, LiquidityConfig, RateModel),
    /// 2
    DepositAndPledge(u64),
    /// 3
    RedeemAndWithdraw(u64),
    /// 4
    RedeemWithoutLoanAndWithdraw(u64),
    /// 5
    Borrow(u64),
    /// 6
    Repay(u64),
    /// 250
    SolanaRouterSwap(RouterSwapInput),
    /// 251
    RaydiumRouterSwap(RouterSwapInput),
    /// 252
    SaberRouterSwap(RouterSwapInput),
    /// 253
    SolanaSwap(SwapInput),
    /// 254
    RaydiumSwap(SwapInput),
    /// 255
    SaberSwap(SwapInput),
}

impl ProxyInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(ProxyError::InstructionUnpackError)?;
        
        Ok(match tag {
            0 => Self::CreateManager,
            1 => {
                let (oracle_config, rest) = Self::unpack_oracle_config(rest)?;
                let (collateral_config, rest) = Self::unpack_collateral_config(rest)?;
                let (liquidity_config, rest) = Self::unpack_liquidity_config(rest)?;
                let (rate_model, _rest) = Self::unpack_rate_model(rest)?;
                Self::CreateMarketReserve(oracle_config, collateral_config, liquidity_config, rate_model)
            }
            2 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::DepositAndPledge(amount)
            }
            3 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RedeemAndWithdraw(amount)
            }
            4 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::RedeemWithoutLoanAndWithdraw(amount)
            }
            5 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::Borrow(amount)
            }
            6 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::Repay(amount)
            }
            250 => {
                let (router_num, rest) = Self::unpack_u8(rest)?;
                let (amount_in, rest) = Self::unpack_u64(rest)?;
                let (minimum_amount_out, _rest) = Self::unpack_u64(rest)?;
                Self::SolanaRouterSwap(RouterSwapInput {
                    router_num,
                    amount_in,
                    minimum_amount_out,
                })
            }
            251 => {
                let (router_num, rest) = Self::unpack_u8(rest)?;
                let (amount_in, rest) = Self::unpack_u64(rest)?;
                let (minimum_amount_out, _rest) = Self::unpack_u64(rest)?;
                Self::RaydiumRouterSwap(RouterSwapInput {
                    router_num,
                    amount_in,
                    minimum_amount_out,
                })
            }
            252 => {
                let (router_num, rest) = Self::unpack_u8(rest)?;
                let (amount_in, rest) = Self::unpack_u64(rest)?;
                let (minimum_amount_out, _rest) = Self::unpack_u64(rest)?;
                Self::SaberRouterSwap(RouterSwapInput {
                    router_num,
                    amount_in,
                    minimum_amount_out,
                })
            }
            253 => {
                let (amount_in, rest) = Self::unpack_u64(rest)?;
                let (minimum_amount_out, _rest) = Self::unpack_u64(rest)?;
                Self::SolanaSwap(SwapInput {
                    amount_in,
                    minimum_amount_out,
                })
            }
            254 => {
                let (amount_in, rest) = Self::unpack_u64(rest)?;
                let (minimum_amount_out, _rest) = Self::unpack_u64(rest)?;
                Self::RaydiumSwap(SwapInput {
                    amount_in,
                    minimum_amount_out,
                })
            }
            255 => {
                let (amount_in, rest) = Self::unpack_u64(rest)?;
                let (minimum_amount_out, _rest) = Self::unpack_u64(rest)?;
                Self::SaberSwap(SwapInput {
                    amount_in,
                    minimum_amount_out,
                })
            }
            _ => {
                return Err(ProxyError::InstructionUnpackError.into());
            }
        })
    }

    fn unpack_rate_model(input: &[u8]) -> Result<(RateModel, &[u8]), ProgramError> {
        let (offset, rest) = Self::unpack_u64(input)?;
        let (optimal, rest) = Self::unpack_u64(rest)?;
        let (kink, rest) = Self::unpack_u8(rest)?;
        let (max, rest) = Self::unpack_u128(rest)?;

        Ok((RateModel { offset, optimal, kink, max }, rest))
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
            return Err(ProxyError::InstructionUnpackError.into());
        }
        let (key, rest) = input.split_at(PUBKEY_BYTES);
        let pk = Pubkey::new(key);
        Ok((pk, rest))
    }

    fn unpack_u128(input: &[u8]) -> Result<(u128, &[u8]), ProgramError> {
        if input.len() < 16 {
            msg!("u128 cannot be unpacked");
            return Err(ProxyError::InstructionUnpackError.into());
        }
        let (amount, rest) = input.split_at(16);
        let amount = amount
            .get(..16)
            .and_then(|slice| slice.try_into().ok())
            .map(u128::from_le_bytes)
            .ok_or(ProxyError::InstructionUnpackError)?;
        Ok((amount, rest))
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        if input.len() < 8 {
            msg!("u64 cannot be unpacked");
            return Err(ProxyError::InstructionUnpackError.into());
        }
        let (amount, rest) = input.split_at(8);
        let amount = amount
            .get(..8)
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(ProxyError::InstructionUnpackError)?;
        Ok((amount, rest))
    }

    fn unpack_u8(input: &[u8]) -> Result<(u8, &[u8]), ProgramError> {
        if input.is_empty() {
            msg!("u8 cannot be unpacked");
            return Err(ProxyError::InstructionUnpackError.into());
        }
        let (amount, rest) = input.split_at(1);
        let amount = amount
            .get(..1)
            .and_then(|slice| slice.try_into().ok())
            .map(u8::from_le_bytes)
            .ok_or(ProxyError::InstructionUnpackError)?;
        Ok((amount, rest))
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::CreateManager => buf.push(0),
            Self::CreateMarketReserve(
                oracle_config,
                collateral_config,
                liquidity_config,
                rate_model,
            ) => {
                buf.push(1);
                Self::pack_oracle_config(oracle_config, &mut buf);
                Self::pack_collateral_config(collateral_config, &mut buf);
                Self::pack_liquidity_config(liquidity_config, &mut buf);
                Self::pack_rate_model(rate_model, &mut buf);
            }
            Self::DepositAndPledge(amount) => {
                buf.push(2);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::RedeemAndWithdraw(amount) => {
                buf.push(3);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::RedeemWithoutLoanAndWithdraw(amount) => {
                buf.push(4);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::Borrow(amount) => {
                buf.push(5);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::Repay(amount) => {
                buf.push(6);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::SolanaRouterSwap(RouterSwapInput {
                router_num,
                amount_in,
                minimum_amount_out 
            }) => {
                buf.push(250);
                buf.extend_from_slice(&router_num.to_le_bytes());
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
            Self::RaydiumRouterSwap(RouterSwapInput {
                router_num,
                amount_in,
                minimum_amount_out 
            }) => {
                buf.push(251);
                buf.extend_from_slice(&router_num.to_le_bytes());
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
            Self::SaberRouterSwap(RouterSwapInput {
                router_num,
                amount_in,
                minimum_amount_out 
            }) => {
                buf.push(252);
                buf.extend_from_slice(&router_num.to_le_bytes());
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
            Self::SolanaSwap(SwapInput {
                amount_in,
                minimum_amount_out,
            }) => {
                buf.push(253);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
            Self::RaydiumSwap(SwapInput { 
                amount_in,
                minimum_amount_out,
            }) => {
                buf.push(254);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
            Self::SaberSwap(SwapInput { 
                amount_in,
                minimum_amount_out,
            }) => {
                buf.push(255);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
        }
        buf
    }

    fn pack_rate_model(model: RateModel, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&model.offset.to_le_bytes());
        buf.extend_from_slice(&model.optimal.to_le_bytes());
        buf.extend_from_slice(&model.kink.to_le_bytes());
        buf.extend_from_slice(&model.max.to_le_bytes());
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

pub fn create_manager(
    manager_key: Pubkey,
    authority_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new(manager_key, true),
            AccountMeta::new_readonly(authority_key, true),
            AccountMeta::new_readonly(soda_lending::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ProxyInstruction::CreateManager.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create_market_reserve(
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
    let lending_id = soda_lending::id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &lending_id,
    );

    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new_readonly(manager_authority_key, false),
            AccountMeta::new(supply_token_account_key, true),
            AccountMeta::new(market_reserve_key, true),
            AccountMeta::new_readonly(token_mint_key, false),
            AccountMeta::new(sotoken_mint_key, true),
            AccountMeta::new_readonly(authority_key, true),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(lending_id, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ProxyInstruction::CreateMarketReserve(oracle_config, collateral_config, liquidity_config, rate_model).pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn deposit_and_pledge_or_repay<IsRepay: typenum::Bit>(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    supply_mint_key: Pubkey,
    supply_token_account_key: Pubkey,
    authority_key: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let lending_id = soda_lending::id();
    let (user_obligation_key, _bump_seed) = Pubkey::find_program_address(
        &[
            lending_id.as_ref(),
            manager_key.as_ref(),
            authority_key.as_ref(),
        ],
        &program_id,
    );

    let user_token_account_key = get_associated_token_address(
        &authority_key,
        &supply_mint_key,
    );

    let accounts = if IsRepay::BOOL {
        vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new_readonly(supply_mint_key, false),
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new(authority_key, true),
            AccountMeta::new(user_token_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(lending_id, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_associated_token_account::id(), false),
        ]
    } else {
        vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new_readonly(supply_mint_key, false),
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new(authority_key, true),
            AccountMeta::new(user_token_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(lending_id, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_associated_token_account::id(), false),
        ]
    };

    Instruction {
        program_id,
        accounts,
        data: if IsRepay::BOOL {
            ProxyInstruction::Repay(amount)
        } else {
            ProxyInstruction::DepositAndPledge(amount)
        }.pack(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn redeem_and_withdraw_or_borrow<IsBorrow: typenum::Bit, WithLoan: typenum::Bit>(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    supply_mint_key: Pubkey,
    supply_token_account_key: Pubkey,
    authority_key: Pubkey,
    amount: u64,
) -> Instruction {
    let program_id = id();
    let lending_id = soda_lending::id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &lending_id,
    );

    let (user_obligation_key, _bump_seed) = Pubkey::find_program_address(
        &[
            lending_id.as_ref(),
            manager_key.as_ref(),
            authority_key.as_ref(),
        ],
        &program_id,
    );

    let user_token_account_key = get_associated_token_address(
        &authority_key,
        &supply_mint_key,
    );

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new_readonly(manager_authority_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new_readonly(supply_mint_key, false),
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new(user_obligation_key, false),
            AccountMeta::new(authority_key, true),
            AccountMeta::new(user_token_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(lending_id, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_associated_token_account::id(), false),
        ],
        data: if IsBorrow::BOOL {
            ProxyInstruction::Borrow(amount)
        } else {
            if WithLoan::BOOL {
                ProxyInstruction::RedeemAndWithdraw(amount)
            } else {
                ProxyInstruction::RedeemWithoutLoanAndWithdraw(amount)
            }
        }.pack(),
    }
}
