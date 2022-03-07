#![allow(missing_docs)]
/// Program state processor
use std::any::Any;
use num_traits::FromPrimitive;
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    decode_error::DecodeError,
    entrypoint::ProgramResult,
    msg,
    program_error::{PrintProgramError, ProgramError},
    program_option::COption,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{clock::Clock, rent::Rent, Sysvar},
};
use spl_token::{state::{Mint, Account}, native_mint};

use crate::{
    assert_rent_exempt, assert_uninitialized, handle_amount, Data,
    dex::{DexType, OrcaSwapContext, Swapper, ORCA_DEX},
    error::LendingError,
    instruction::LendingInstruction,
    invoker::{
        process_token_transfer, process_token_init_account,
        process_token_init_mint, process_token_mint_to,
        process_token_burn, process_token_approve, process_token_revoke, process_invoke,
    },
    state::{
        CollateralConfig, LiquidityControl, LiquidityConfig,
        Manager, MarketReserve, Operator, Param, RateModel,
        TokenConfig, UserObligation, calculate_amount,
    },
    oracle::OracleConfig, math::Decimal,
};
#[cfg(feature = "unique-credit")]
use crate::state::UniqueCredit;

/// Processes an instruction
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = LendingInstruction::unpack(input)?;
    match instruction {
        LendingInstruction::InitManager => {
            msg!("Instruction: Init Lending Manager");
            process_init_manager(program_id, accounts)
        }
        LendingInstruction::InitMarketReserve(
            oracle_config,
            collateral_config,
            liquidity_config,
            rate_model,
        ) => {
            msg!("Instruction: Init Market Reserve");
            process_init_market_reserve(
                program_id,
                accounts,
                oracle_config,
                collateral_config,
                liquidity_config,
                rate_model,
            )
        }
        LendingInstruction::RefreshMarketReserves => {
            msg!("Instruction: Refresh Market Reserves");
            process_refresh_market_reserves(program_id, accounts)
        }
        LendingInstruction::Deposit(amount) => {
            msg!("Instruction: Deposit: {}", amount);
            process_deposit_or_withdraw::<true>(program_id, accounts, amount)
        }
        LendingInstruction::Withdraw (amount) => {
            msg!("Instruction: Withdraw: {}", amount);
            process_deposit_or_withdraw::<false>(program_id, accounts, amount)
        }
        LendingInstruction::InitUserObligation => {
            msg!("Instruction: Init User Obligation");
            process_init_user_obligation(program_id, accounts)
        }
        LendingInstruction::RefreshUserObligation => {
            msg!("Instruction: Refresh User Obligation");
            process_refresh_user_obligation(program_id, accounts)
        }
        #[cfg(feature = "friend")]
        LendingInstruction::BindFriend => {
            msg!("Instruction: Bind Friend");
            process_bind_friend(program_id, accounts)
        }
        #[cfg(feature = "friend")]
        LendingInstruction::UnbindFriend => {
            msg!("Instruction: Unbind Friend");
            process_unbind_friend(program_id, accounts)
        }
        LendingInstruction::PledgeCollateral(amount) => {
            msg!("Instruction: Pledge Collateral: {}", amount);
            process_pledge_collateral(program_id, accounts, amount)
        }
        LendingInstruction::DepositAndPledge(amount) => {
            msg!("Instruction: Deposit Liquidity and Pledge: {}", amount);
            process_deposit_and_pledge(program_id, accounts, amount)
        }
        LendingInstruction::RedeemCollateral(amount) => {
            msg!("Instruction: Redeem Collateral: {}", amount);
            process_redeem_collateral(program_id, accounts, amount)
        }
        LendingInstruction::RedeemAndWithdraw(amount) => {
            msg!("Instruction: Redeem Collateral and Withdraw: {}", amount);
            process_redeem_and_withdraw::<true>(program_id, accounts, amount)
        }
        LendingInstruction::RedeemCollateralWithoutLoan(amount) => {
            msg!("Instruction: Redeem Collateral Without Loan: {}", amount);
            process_redeem_collateral_without_loan(program_id, accounts, amount)
        }
        LendingInstruction::RedeemWithoutLoanAndWithdraw(amount) => {
            msg!("Instruction: Redeem Collateral Without Loan And Withdraw: {}", amount);
            process_redeem_and_withdraw::<false>(program_id, accounts, amount)
        }
        LendingInstruction::ReplaceCollateral(amount) => {
            msg!("Instruction: Replace Collateral: amount = {},", amount);
            process_replace_collateral(program_id, accounts, amount)
        }
        LendingInstruction::BorrowLiquidity(amount) => {
            msg!("Instruction: Borrow Liquidity: {}", amount);
            process_borrow_liquidity(program_id, accounts, amount)
        }
        LendingInstruction::RepayLoan(amount) => {
            msg!("Instruction: Repay Loan: {}", amount);
            process_repay_loan(program_id, accounts, amount)
        }
        LendingInstruction::LiquidateByCollateral(amount) => {
            msg!("Instruction: Liquidate by collateral amount = {}", amount);
            process_liquidate::<true>(program_id, accounts, amount)
        }
        LendingInstruction::LiquidateByLoan(amount) => {
            msg!("Instruction: Liquidate by loan amount = {}", amount);
            process_liquidate::<false>(program_id, accounts, amount)
        }
        LendingInstruction::FlashLiquidationByCollateral(tag, amount) => {
            msg!("Instruction: Flash Liquidation by Collateral: amount = {}", amount);
            process_flash_liquidate::<true>(program_id, accounts, tag, amount)
        }
        LendingInstruction::FlashLiquidationByLoan(tag, amount) => {
            msg!("Instruction: Flash Liquidation by Loan: amount = {}", amount);
            process_flash_liquidate::<false>(program_id, accounts, tag, amount)
        }
        LendingInstruction::FlashLoan(tag, amount) => {
            msg!("Instruction: Flash Loan: amount = {}", amount);
            process_flash_loan(program_id, accounts, tag, amount)
        }
        LendingInstruction::EasyRepayWithOrca(sotoken_amount, min_repay_amount) => {
            msg!("Instruction: Easy Repay With Orca: collateral amount = {}, min repay amount {}", sotoken_amount, min_repay_amount);
            process_easy_repay_by_dex::<ORCA_DEX>(program_id, accounts, sotoken_amount, min_repay_amount)
        }
        LendingInstruction::OpenLeveragePositionWithOrca(borrow_amount, min_collateral_amount) => {
            msg!("Instruction: Open Leverage Position With Orca: borrow amount = {}, min collateral amount {}", borrow_amount, min_collateral_amount);
            process_open_leverage_position_by_dex::<ORCA_DEX>(program_id, accounts, borrow_amount, min_collateral_amount)
        }
        #[cfg(feature = "unique-credit")]
        LendingInstruction::InitUniqueCredit(authority, amount) => {
            msg!("Instruction: Init Unique Credit");
            process_init_unique_credit(program_id, accounts, authority, amount)
        }
        #[cfg(feature = "unique-credit")]
        LendingInstruction::BorrowLiquidityByUniqueCredit(amount) => {
            msg!("Instruction: Borrow Liquidity by Unique Credit: amount = {}", amount);
            process_borrow_liquidity_by_unique_credit(program_id, accounts, amount)
        }
        #[cfg(feature = "unique-credit")]
        LendingInstruction::RepayLoanByUniqueCredit(amount) => {
            msg!("Instruction: Repay Loan by Unique Credit: amount = {}", amount);
            process_repay_loan_by_unique_credit(program_id, accounts, amount)
        }
        LendingInstruction::UpdateIndexedCollateralConfig(config) => {
            msg!("Instruction: Update User Obligation Collateral Config");
            process_operate_user_obligation(program_id, accounts, config)
        }
        LendingInstruction::UpdateIndexedLoanConfig(config) => {
            msg!("Instruction: Update User Obligation Loan Config");
            process_operate_user_obligation(program_id, accounts, config)
        }
        LendingInstruction::ControlMarketReserveLiquidity(enable) => {
            msg!("Instruction: Control Market Reserve Liquidity");
            process_operate_market_reserve(program_id, accounts, LiquidityControl(enable))
        }
        LendingInstruction::UpdateMarketReserveRateModel(model) => {
            msg!("Instruction: Update Market Reserve Rate Model");
            process_operate_market_reserve(program_id, accounts, model)
        }
        LendingInstruction::UpdateMarketReserveCollateralConfig(config) => {
            msg!("Instruction: Update Market Reserve Collateral Config");
            process_operate_market_reserve(program_id, accounts, config)
        }
        LendingInstruction::UpdateMarketReserveLiquidityConfig(config) => {
            msg!("Instruction: Update Market Reserve Liquidity Config");
            process_operate_market_reserve(program_id, accounts, config)
        }
        LendingInstruction::UpdateMarketReserveOracleConfig(config) => {
            msg!("Instruction: Update Market Reserve Price Oracle Config");
            process_operate_market_reserve(program_id, accounts, config)
        }
        LendingInstruction::ReduceInsurance(amount) => {
            msg!("Instruction: Reduce Insurance: {}", amount);
            process_reduce_insurance(program_id, accounts, amount)
        }
        LendingInstruction::ChangeManagerOwner => {
            msg!("Instruction: Change Manager Owner");
            process_change_manager_owner(program_id, accounts)
        }
        #[cfg(feature = "unique-credit")]
        LendingInstruction::UpdateUniqueCreditLimit(amount) => {
            msg!("Instruction: Update Unique Credit Limit: amount = {}", amount);
            process_update_unique_credit_limit(program_id, accounts, amount)
        }
    }
}

fn process_init_manager(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, manager_info)?;
    assert_uninitialized::<Manager>(manager_info)?;
    // 3
    let authority_info = next_account_info(account_info_iter)?;
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    
    let manager = Manager::new(
        Pubkey::find_program_address(&[manager_info.key.as_ref()], program_id).1,
        *authority_info.key,
    );
    Manager::pack(manager, &mut manager_info.try_borrow_mut_data()?)
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn process_init_market_reserve(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    oracle_config: OracleConfig,
    collateral_config: CollateralConfig,
    liquidity_config: LiquidityConfig,
    rate_model: RateModel,
) -> ProgramResult {
    // check config
    collateral_config.assert_valid()?;
    liquidity_config.assert_valid()?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let rent_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(rent_info)?;
    // 2
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 3
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let manager_authority = Pubkey::create_program_address(
        &[manager_info.key.as_ref(), &[manager.bump_seed]],
        program_id
    )?;
    // 4
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 5
    let supply_token_account_info = next_account_info(account_info_iter)?;
    // 6
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, market_reserve_info)?;
    assert_uninitialized::<MarketReserve>(market_reserve_info)?;
    // 7
    let token_mint_info = next_account_info(account_info_iter)?;
    let token_decimals = get_token_decimals(token_mint_info)?;
    // 8
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    // 9
    let authority_info = next_account_info(account_info_iter)?;
    if authority_info.key != &manager.owner {
        msg!("Only manager owner can create market reserve");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 10
    let token_program_id = next_account_info(account_info_iter)?;

    let market_reserve = MarketReserve::new(
        clock.slot,
        *manager_info.key,
        TokenConfig {
            mint_pubkey: *token_mint_info.key,
            supply_account: *supply_token_account_info.key,
            decimal: token_decimals,
        },
        oracle_config,
        liquidity_config,
        *sotoken_mint_info.key,
        collateral_config,
        rate_model,
    );
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

    // init manager token account
    process_token_init_account(
        token_program_id,
        supply_token_account_info,
        token_mint_info,
        rent_info,
        manager_authority_info,
    )?;

    // init sotoken mint
    process_token_init_mint(
        token_program_id,
        sotoken_mint_info,
        rent_info,
        manager_authority_info.key,
        token_decimals,
    )
}

fn process_refresh_market_reserves(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let (clock_account, accounts) = accounts
        .split_first()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    // 1
    let clock = &Clock::from_account_info(clock_account)?;
    accounts
        .chunks_exact(2)
        .try_for_each(|accounts_info| {
            // 2 + i * 2
            let market_reserve_info = &accounts_info[0];
            // 3 + i * 2
            let price_oracle_info = &accounts_info[1];
        
            if market_reserve_info.owner != program_id {
                msg!("Market reserve provided is not owned by the lending program");
                return Err(LendingError::InvalidAccountOwner.into());
            }
            let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
        
            if price_oracle_info.key != &market_reserve.oracle_info.config.oracle {
                msg!("Oracle of market reserve is not matched with oracle provided");
                return Err(LendingError::InvalidPriceOracle.into());
            }
        
            // update
            market_reserve.oracle_info.update_price(&price_oracle_info.try_borrow_data()?, clock)?;
            market_reserve.accrue_interest(clock.slot)?;
            market_reserve.last_update.update_slot(clock.slot, false);
            // pack
            MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)
        })
}

#[inline(never)]
fn process_deposit_or_withdraw<const IS_DEPOSIT: bool>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        if IS_DEPOSIT {
            msg!("Liquidity amount provided cannot be zero");
        } else {
            msg!("Collateral amount provided cannot be zero");
        }
    })?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Manager of market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    // 5
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    if sotoken_mint_info.key != &market_reserve.collateral_info.sotoken_mint_pubkey {
        msg!("Sotoken mint of market reserve is not matched with sotoken mint acount provided");
        return Err(LendingError::InvalidSoTokenMint.into())
    }
    // 6
    let supply_token_account_info = next_account_info(account_info_iter)?;
    if supply_token_account_info.key != &market_reserve.token_config.supply_account {
        msg!("Supply token account in market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 7
    let user_authority_info = next_account_info(account_info_iter)?;
    // 8
    let user_token_account_info = next_account_info(account_info_iter)?;
    // 9
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    // 10
    let token_program_id = next_account_info(account_info_iter)?;

    // accrue interest
    market_reserve.accrue_interest(clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    // deposit or withdraw
    if IS_DEPOSIT {
        let user_token_account = Account::unpack(&user_token_account_info.try_borrow_data()?)?;
        let amount = calculate_amount(amount, get_available_balance(user_token_account, user_authority_info.key));
        let mint_amount = market_reserve.deposit(amount)?;
        // pack
        MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

        // transfer from user to manager
        process_token_transfer(
            token_program_id,
            user_token_account_info,
            supply_token_account_info,
            user_authority_info,
            amount,
            &[],
        )?;

        // mint to user
        process_token_mint_to(
            token_program_id,
            sotoken_mint_info,
            user_sotoken_account_info,
            manager_authority_info,
            mint_amount,
            authority_signer_seeds,
        )
    } else {
        let amount = calculate_amount(amount, Account::unpack(&user_sotoken_account_info.try_borrow_data()?)?.amount);
        let withdraw_amount = market_reserve.withdraw(amount)?;
        // pack
        MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    
        // burn sotoken
        process_token_burn(
            token_program_id,
            user_sotoken_account_info,
            sotoken_mint_info,
            user_authority_info,
            amount,
            &[],
        )?;
    
        // transfer from manager to user
        process_token_transfer(
            token_program_id,
            supply_token_account_info,
            user_token_account_info,
            manager_authority_info,
            withdraw_amount,
            authority_signer_seeds,
        )
    }
}

fn process_init_user_obligation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 3
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    Manager::unpack(&manager_info.try_borrow_data()?)?;
    // 4
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, user_obligation_info)?;
    assert_uninitialized::<UserObligation>(user_obligation_info)?;
    // 5
    let authority_info = next_account_info(account_info_iter)?;
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }

    let user_obligation = UserObligation::new(
        clock.slot,
        *manager_info.key,
        *authority_info.key,
    );
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)
}

// must after refresh reserves
fn process_refresh_user_obligation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    let manager = user_obligation.manager;
    // 3 + i
    let reserves_vec = account_info_iter
        .map(|market_reserve_info| {
            if market_reserve_info.owner != program_id {
                msg!("Market reserve provided is not owned by the lending program");
                return Err(LendingError::InvalidAccountOwner.into());
            }

            let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
            if market_reserve.manager != manager {
                msg!("User Obligation manager provided is matched with market reserve provided");
                return Err(LendingError::InvalidManager.into());
            }
            if market_reserve.last_update.is_strict_stale(clock.slot)? {
                Err(LendingError::MarketReserveStale.into())
            } else {
                Ok((market_reserve_info.key, market_reserve))
            }
        })
        .collect::<Result<Vec<_>, ProgramError>>()?;

    // update
    user_obligation.update_user_obligation(reserves_vec)?;
    user_obligation.last_update.update_slot(clock.slot, false);
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)
}

#[cfg(feature = "friend")]
fn process_bind_friend(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    // 2
    let friend_obligation_info = next_account_info(account_info_iter)?;
    if friend_obligation_info.owner != program_id {
        msg!("Friend obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;

    if user_obligation_info.key == friend_obligation_info.key {
        msg!("User obligation provided is not matched with friend obligation provided");
        return Err(LendingError::ObligationInvalidFriend.into())
    }
    if user_obligation.manager != friend_obligation.manager {
        msg!("Manager of user obligation is not matched with manager of friend obligation");
        return Err(LendingError::ObligationInvalidFriend.into());
    }
    // 3
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        msg!("User authority provided is not matched with user obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 4
    let friend_authority_info = next_account_info(account_info_iter)?;
    if friend_authority_info.key != &friend_obligation.owner {
        msg!("Friend authority provided is not matched with friend obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !friend_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }

    user_obligation.bind_friend(*friend_obligation_info.key)?;
    friend_obligation.bind_friend(*user_obligation_info.key)?;
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    UserObligation::pack(friend_obligation, &mut friend_obligation_info.try_borrow_mut_data()?)
}

// must after update obligation
#[cfg(feature = "friend")]
fn process_unbind_friend(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if user_obligation.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }
    // 3
    let friend_obligation_info = next_account_info(account_info_iter)?;
    if friend_obligation_info.owner != program_id {
        msg!("Friend obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;
    if friend_obligation.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }
    if user_obligation.manager != friend_obligation.manager {
        msg!("Manager of user obligation is not matched with manager of friend obligation");
        return Err(LendingError::ObligationInvalidFriend.into());
    }
    // 4
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        msg!("User authority provided is not matched with user obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 5
    let friend_authority_info = next_account_info(account_info_iter)?;
    if friend_authority_info.key != &friend_obligation.owner {
        msg!("Friend authority provided is not matched with friend obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !friend_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }

    // unbind
    user_obligation.unbind_friend()?;
    friend_obligation.unbind_friend()?;

    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    UserObligation::pack(friend_obligation, &mut friend_obligation_info.try_borrow_mut_data()?)
}

#[inline(never)]
fn process_pledge_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        msg!("Collateral amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    // 2
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    if sotoken_mint_info.key != &market_reserve.collateral_info.sotoken_mint_pubkey {
        msg!("Sotoken mint of market reserve is not matched with sotoken mint acount provided");
        return Err(LendingError::InvalidSoTokenMint.into()); 
    }
    // 3
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if user_obligation.manager != market_reserve.manager {
        msg!("User Obligation manager provided is matched with market reserve provided");
        return Err(LendingError::InvalidManager.into());
    }
    // 4
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        msg!("User authority provided is not matched with user obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 5
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    let user_sotoken_account = Account::unpack(&user_sotoken_account_info.try_borrow_data()?)?;
    // 6
    let token_program_id = next_account_info(account_info_iter)?;

    // handle obligation
    let balance = get_available_balance(user_sotoken_account, user_authority_info.key);
    let amount = if let Ok(index) = user_obligation.find_collateral(market_reserve_info.key) {
        user_obligation.pledge(balance, amount, index)?
    } else {
        user_obligation.new_pledge(balance, amount, *market_reserve_info.key, &market_reserve)?
    };
    user_obligation.last_update.mark_stale();
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    
    // burn from user
    process_token_burn(
        token_program_id,
        user_sotoken_account_info,
        sotoken_mint_info,
        user_authority_info,
        amount,
        &[],
    )
}

#[inline(never)]
fn process_deposit_and_pledge(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        msg!("Liquidity amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    // 3
    let supply_token_account_info = next_account_info(account_info_iter)?;
    if supply_token_account_info.key != &market_reserve.token_config.supply_account {
        msg!("Supply token account in market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 4
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if user_obligation.manager != market_reserve.manager {
        msg!("User Obligation manager provided is matched with market reserve provided");
        return Err(LendingError::InvalidManager.into());
    }
    // 5
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        msg!("User authority provided is not matched with user obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 6
    let user_token_account_info = next_account_info(account_info_iter)?;
    let user_token_account = Account::unpack(&user_token_account_info.try_borrow_data()?)?;
    // 7
    let token_program_id = next_account_info(account_info_iter)?;

    // accrue interest
    market_reserve.accrue_interest(clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    // deposit in reserve
    let amount = calculate_amount(amount, get_available_balance(user_token_account, user_authority_info.key));
    let mint_amount = market_reserve.deposit(amount)?;
    // pledge in obligation
    let _ = if let Ok(index) = user_obligation.find_collateral(market_reserve_info.key) {
        user_obligation.pledge(mint_amount, None, index)?
    } else {
        user_obligation.new_pledge(mint_amount, None, *market_reserve_info.key, &market_reserve)?
    };
    user_obligation.last_update.mark_stale();
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;

    // transfer token to manager
    process_token_transfer(
        token_program_id,
        user_token_account_info,
        supply_token_account_info,
        user_authority_info,
        amount,
        &[],
    )
}

// must after update obligation
#[inline(never)]
fn process_redeem_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        msg!("Collateral amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Manager of market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    if sotoken_mint_info.key != &market_reserve.collateral_info.sotoken_mint_pubkey {
        msg!("Sotoken mint of market reserve is not matched with sotoken mint acount provided");
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    // 6
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if user_obligation.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    let friend_obligation = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 7
        let friend_obligation_info = next_account_info(account_info_iter)?;
        if friend_obligation_info.key != friend {
            msg!("Friend obligation provided is not matched with friend in user obligation");
            return Err(LendingError::ObligationInvalidFriend.into());
        }
        let friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;
        if friend_obligation.last_update.is_lax_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }

        Some(friend_obligation)
    } else {
        None
    };
    // 7/8
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        msg!("User authority provided is not matched with user obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 8/9
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    // 9/10
    let token_program_id = next_account_info(account_info_iter)?;

    // redeem in obligation
    let index = user_obligation.find_collateral(market_reserve_info.key)?;
    let amount = user_obligation.redeem::<true, true>(amount, index, &market_reserve, friend_obligation)?;
    user_obligation.last_update.mark_stale();
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    
    // mint to user
    process_token_mint_to(
        token_program_id,
        sotoken_mint_info,
        user_sotoken_account_info,
        manager_authority_info,
        amount,
        authority_signer_seeds,
    )
}

// must after update obligation if with loan
#[inline(never)]
fn process_redeem_and_withdraw<const WITH_LOAN: bool>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        msg!("Collateral amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Manager of market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if WITH_LOAN && market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let supply_token_account_info = next_account_info(account_info_iter)?;
    if supply_token_account_info.key != &market_reserve.token_config.supply_account {
        msg!("Supply token account in market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 6
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if WITH_LOAN && user_obligation.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    let friend_obligation = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 7
        let friend_obligation_info = next_account_info(account_info_iter)?;
        if friend_obligation_info.key != friend {
            msg!("Friend obligation provided is not matched with friend in user obligation");
            return Err(LendingError::ObligationInvalidFriend.into());
        }
        let friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;
        if WITH_LOAN && friend_obligation.last_update.is_lax_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into())
        }

        Some(friend_obligation)
    } else {
        None
    };
    // 7/8
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        msg!("User authority provided is not matched with user obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 8/9
    let user_token_account_info = next_account_info(account_info_iter)?;
    // 9/10
    let token_program_id = next_account_info(account_info_iter)?;

    // redeem in obligation
    let index = user_obligation.find_collateral(market_reserve_info.key)?;
    let amount = if WITH_LOAN {
        user_obligation.redeem::<true, true>(amount, index, &market_reserve, friend_obligation)?
    } else {
        user_obligation.redeem_without_loan(amount, index, friend_obligation)?
    };
    // withdraw
    market_reserve.accrue_interest(clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    let withdraw_amount = market_reserve.withdraw(amount)?;
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;

    // transfer from manager to user
    process_token_transfer(
        token_program_id,
        supply_token_account_info,
        user_token_account_info,
        manager_authority_info,
        withdraw_amount,
        authority_signer_seeds,
    )
}

#[inline(never)]
fn process_redeem_collateral_without_loan(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        msg!("Collateral amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 2
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 3
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Manager of market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    // 4
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    if sotoken_mint_info.key != &market_reserve.collateral_info.sotoken_mint_pubkey {
        msg!("Sotoken mint of market reserve is not matched with sotoken mint acount provided");
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    // 5
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }

    let friend_obligation = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 6
        let friend_obligation_info = next_account_info(account_info_iter)?;
        if friend_obligation_info.key != friend {
            msg!("Friend obligation provided is not matched with friend in user obligation");
            return Err(LendingError::ObligationInvalidFriend.into());
        }
        let friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;

        Some(friend_obligation)
    } else {
        None
    };
    // 6/7
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        msg!("User authority provided is not matched with user obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 7/8
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    // 8/9
    let token_program_id = next_account_info(account_info_iter)?;

    // redeem in obligation
    let index = user_obligation.find_collateral(market_reserve_info.key)?;
    let amount = user_obligation.redeem_without_loan(amount, index, friend_obligation)?;
    user_obligation.last_update.mark_stale();
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    
    // mint to user
    process_token_mint_to(
        token_program_id,
        sotoken_mint_info,
        user_sotoken_account_info,
        manager_authority_info,
        amount,
        authority_signer_seeds,
    )
}

// must after update obligation
#[inline(never)]
fn process_replace_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        msg!("Collateral amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let out_market_reserve_info = next_account_info(account_info_iter)?;
    if out_market_reserve_info.owner != program_id {
        msg!("Out market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let out_market_reserve = MarketReserve::unpack(&out_market_reserve_info.try_borrow_data()?)?;
    if &out_market_reserve.manager != manager_info.key {
        msg!("Manager of out market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if out_market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let out_sotoken_mint_info = next_account_info(account_info_iter)?;
    if out_sotoken_mint_info.key != &out_market_reserve.collateral_info.sotoken_mint_pubkey {
        msg!("Sotoken mint of out market reserve is not matched with out sotoken mint acount provided");
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    // 6
    let in_market_reserve_info = next_account_info(account_info_iter)?;
    if in_market_reserve_info.owner != program_id {
        msg!("In market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let in_market_reserve = MarketReserve::unpack(&in_market_reserve_info.try_borrow_data()?)?;
    if &in_market_reserve.manager != manager_info.key {
        msg!("Manager of in market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if in_market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 7
    let in_sotoken_mint_info = next_account_info(account_info_iter)?;
    if in_sotoken_mint_info.key != &in_market_reserve.collateral_info.sotoken_mint_pubkey {
        msg!("Sotoken mint of in market reserve is not matched with in sotoken mint acount provided");
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    // 8
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if user_obligation.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    let friend_obligation = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 9
        let friend_obligation_info = next_account_info(account_info_iter)?;
        if friend_obligation_info.key != friend {
            msg!("Friend obligation provided is not matched with friend in user obligation");
            return Err(LendingError::ObligationInvalidFriend.into());
        }
        let friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;
        if friend_obligation.last_update.is_lax_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }

        Some(friend_obligation)
    } else {
        None
    };
    // 9/10
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        msg!("User authority provided is not matched with user obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 10/11
    let user_out_sotoken_account_info = next_account_info(account_info_iter)?;
    // 12/13
    let user_in_sotoken_account_info = next_account_info(account_info_iter)?;
    let user_in_sotoken_account = Account::unpack(&user_in_sotoken_account_info.try_borrow_data()?)?;
    // 13/14
    let token_program_id = next_account_info(account_info_iter)?;

    // replace
    let out_index = user_obligation.find_collateral(out_market_reserve_info.key)?;
    if user_obligation.find_collateral(in_market_reserve_info.key).is_ok() {
        return Err(LendingError::ObligationReplaceCollateralExists.into());
    }
    let (in_amount, out_amount) = user_obligation.replace_collateral(
        get_available_balance(user_in_sotoken_account, user_authority_info.key),
        amount,
        out_index,
        *in_market_reserve_info.key,
        &out_market_reserve,
        &in_market_reserve,
        friend_obligation,
    )?;
    user_obligation.last_update.mark_stale();
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;

    // mint to user
    process_token_mint_to(
        token_program_id,
        out_sotoken_mint_info,
        user_out_sotoken_account_info,
        manager_authority_info,
        out_amount,
        authority_signer_seeds,
    )?;

    // burn from user
    process_token_burn(
        token_program_id,
        user_in_sotoken_account_info,
        in_sotoken_mint_info,
        user_authority_info,
        in_amount,
        &[],
    )
}

// must after update obligation
#[inline(never)]
fn process_borrow_liquidity(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        msg!("Liquidity amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Manager of market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let supply_token_account_info = next_account_info(account_info_iter)?;
    if supply_token_account_info.key != &market_reserve.token_config.supply_account {
        msg!("Supply token account in market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 6
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if user_obligation.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }
    
    let friend_obligation = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 7
        let friend_obligation_info = next_account_info(account_info_iter)?;
        if friend_obligation_info.key != friend {
            msg!("Friend obligation provided is not matched with friend in user obligation");
            return Err(LendingError::ObligationInvalidFriend.into());
        }
        let friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;
        if friend_obligation.last_update.is_lax_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }

        Some(friend_obligation)
    } else {
        None
    };
    // 7/8
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        msg!("User authority provided is not matched with user obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 8/9
    let user_token_account_info = next_account_info(account_info_iter)?;
    // 9/10
    let token_program_id = next_account_info(account_info_iter)?;

    // borrow
    let amount = if let Ok(index) = user_obligation.find_loan(market_reserve_info.key) {
        user_obligation.borrow_in(
            amount,
            index,
            &market_reserve,
            friend_obligation,
        )?
    } else {
        user_obligation.new_borrow_in(
            amount,
            *market_reserve_info.key,
            &market_reserve,
            friend_obligation,
        )?
    };
    user_obligation.last_update.mark_stale();
    // accrue interest
    market_reserve.accrue_interest(clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    market_reserve.liquidity_info.borrow_out(amount)?;
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

    // transfer token to user
    process_token_transfer(
        token_program_id,
        supply_token_account_info,
        user_token_account_info,
        manager_authority_info,
        amount,
        authority_signer_seeds,
    )
}

#[inline(never)]
fn process_repay_loan(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        msg!("Liquidity amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    // 3
    let supply_token_account_info = next_account_info(account_info_iter)?;
    if supply_token_account_info.key != &market_reserve.token_config.supply_account {
        msg!("Supply token account in market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 4
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if user_obligation.manager != market_reserve.manager {
        msg!("User Obligation manager provided is matched with market reserve provided");
        return Err(LendingError::InvalidManager.into());
    }
    // 5
    let user_authority_info = next_account_info(account_info_iter)?;
    // 6
    let user_token_account_info = next_account_info(account_info_iter)?;
    let user_balance = Account::unpack(&user_token_account_info.try_borrow_data()?)?.amount;
    // 7
    let token_program_id = next_account_info(account_info_iter)?;    

    // accrue interest
    market_reserve.accrue_interest(clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    // repay in obligation
    let index = user_obligation.find_loan(market_reserve_info.key)?;
    user_obligation.loans[index].accrue_interest(&market_reserve)?;
    let settle = user_obligation.repay(amount, user_balance, index)?;
    user_obligation.last_update.mark_stale();
    // repay in reserve 
    market_reserve.liquidity_info.repay(&settle)?;
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

    // transfer to manager
    process_token_transfer(
        token_program_id,
        user_token_account_info,
        supply_token_account_info,
        user_authority_info,
        settle.amount,
        &[],
    )
}

// must after update obligation
#[inline(never)]
fn process_liquidate<const IS_COLLATERAL: bool>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        msg!("Liquidity amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let collateral_market_reserve_info = next_account_info(account_info_iter)?;
    if collateral_market_reserve_info.owner != program_id {
        msg!("Collateral market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let collateral_market_reserve = MarketReserve::unpack(&collateral_market_reserve_info.try_borrow_data()?)?;
    if &collateral_market_reserve.manager != manager_info.key {
        msg!("Manager of collateral market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if collateral_market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    if sotoken_mint_info.key != &collateral_market_reserve.collateral_info.sotoken_mint_pubkey {
        msg!("Sotoken mint of collateral market reserve is not matched with sotoken mint acount provided");
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    // 6
    let loan_market_reserve_info = next_account_info(account_info_iter)?;
    if loan_market_reserve_info.owner != program_id {
        msg!("Loan market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut loan_market_reserve = MarketReserve::unpack(&loan_market_reserve_info.try_borrow_data()?)?;
    if &loan_market_reserve.manager != manager_info.key {
        msg!("Manager of loan market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if loan_market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 7
    let supply_token_account_info = next_account_info(account_info_iter)?;
    if supply_token_account_info.key != &loan_market_reserve.token_config.supply_account {
        msg!("Supply token account in loan market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 8
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if user_obligation.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    let friend_obligation = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 9
        let friend_obligation_info = next_account_info(account_info_iter)?;
        if friend_obligation_info.key != friend {
            msg!("Friend obligation provided is not matched with friend in user obligation");
            return Err(LendingError::ObligationInvalidFriend.into());
        }
        let friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;
        if friend_obligation.last_update.is_lax_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }

        Some(friend_obligation)
    } else {
        None
    };
    // 9/10
    let liquidator_authority_info = next_account_info(account_info_iter)?;
    // 10/11
    let liquidator_token_account_info = next_account_info(account_info_iter)?;
    // 11/12
    let liquidator_sotoken_account_info = next_account_info(account_info_iter)?;
    // 12/13
    let token_program_id = next_account_info(account_info_iter)?;

    // liquidate
    let collateral_index = user_obligation.find_collateral(collateral_market_reserve_info.key)?;
    let loan_index = user_obligation.find_loan(loan_market_reserve_info.key)?;
    let (so_token_amount, settle) = user_obligation.liquidate::<IS_COLLATERAL>(
        amount,
        collateral_index,
        loan_index,
        &collateral_market_reserve,
        &loan_market_reserve,
        friend_obligation,
    )?;
    user_obligation.last_update.mark_stale();
    // repay in market reserve
    loan_market_reserve.accrue_interest(clock.slot)?;
    loan_market_reserve.last_update.update_slot(clock.slot, true);
    loan_market_reserve.liquidity_info.repay(&settle)?;
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(loan_market_reserve, &mut loan_market_reserve_info.try_borrow_mut_data()?)?;

    // transfer token to manager
    process_token_transfer(
        token_program_id,
        liquidator_token_account_info,
        supply_token_account_info,
        liquidator_authority_info,
        settle.amount,
        &[],
    )?;

    // mint to user
    process_token_mint_to(
        token_program_id,
        sotoken_mint_info,
        liquidator_sotoken_account_info,
        manager_authority_info,
        so_token_amount,
        authority_signer_seeds,
    )
}

// must after update market reserve
#[inline(never)]
fn process_flash_loan(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    tag: u8,
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        msg!("Flash loan amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter().peekable();
    // 1
    let clock_info = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(clock_info)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Manager of market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    // 5
    let supply_token_account_info = next_account_info(account_info_iter)?;
    if supply_token_account_info.key != &market_reserve.token_config.supply_account {
        msg!("Supply token account in market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 6
    let receiver_authority_info = next_account_info(account_info_iter)?;
    // 7
    let token_program_id = next_account_info(account_info_iter)?;
    // 8
    let receiver_program_id = next_account_info(account_info_iter)?;
    if receiver_program_id.key == program_id {
        msg!("Flash Loan receiver program can not be lending program");
        return Err(ProgramError::IncorrectProgramId);
    }

    // accrue interest
    market_reserve.accrue_interest(clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    // flash loan borrow calculate
    let borrow_amount = calculate_amount(amount, market_reserve.liquidity_info.available);
    let (flash_loan_total_repay, flash_loan_fee) = market_reserve.liquidity_info.flash_loan_borrow_out(borrow_amount)?;
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

    let expect_balance_after_flash_loan = Account::unpack(&supply_token_account_info.try_borrow_data()?)?.amount
        .checked_add(flash_loan_fee)
        .ok_or(LendingError::MathOverflow)?;

    // approve to receiver
    process_token_approve(
        token_program_id,
        supply_token_account_info,
        receiver_authority_info,
        manager_authority_info,
        borrow_amount,
        authority_signer_seeds,
    )?;

    // prepare instruction and account infos    
    let mut flash_loan_instruction_account_infos = vec![
        clock_info.clone(),
        supply_token_account_info.clone(),
        receiver_authority_info.clone(),
        token_program_id.clone(),
    ];
    // 9 ~
    flash_loan_instruction_account_infos.extend(account_info_iter.map(|account_info| account_info.clone()));

    process_invoke(
        FlashLoanData { tag, flash_loan_total_repay },
        receiver_program_id,
        flash_loan_instruction_account_infos,
        &[],
    )?;

    process_token_revoke(
        token_program_id,
        supply_token_account_info,
        manager_authority_info,
        authority_signer_seeds,
    )?;

    // check balance
    let balance_after = Account::unpack(&supply_token_account_info.try_borrow_data()?)?.amount;
    if balance_after < expect_balance_after_flash_loan {
        return Err(LendingError::FlashLoanRepayInsufficient.into());
    }
    // check if reserve changed during flash loan
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    market_reserve.liquidity_info.flash_loan_repay(borrow_amount, flash_loan_fee)?;
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)
}

fn process_flash_liquidate<const IS_COLLATERAL: bool>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    tag: u8,
    amount: u64,
) -> ProgramResult {
    let amount = handle_amount(amount, || {
        msg!("Flash liquidation amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter().peekable();
    // 1
    let clock_info = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(clock_info)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let collateral_market_reserve_info = next_account_info(account_info_iter)?;
    if collateral_market_reserve_info.owner != program_id {
        msg!("Collateral market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut collateral_market_reserve = MarketReserve::unpack(&collateral_market_reserve_info.try_borrow_data()?)?;
    if &collateral_market_reserve.manager != manager_info.key {
        msg!("Manager of collateral market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if collateral_market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let collateral_supply_account_info = next_account_info(account_info_iter)?;
    if collateral_supply_account_info.key != &collateral_market_reserve.token_config.supply_account {
        msg!("Supply token account in collateral market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 6
    let loan_market_reserve_info = next_account_info(account_info_iter)?;
    if loan_market_reserve_info.owner != program_id {
        msg!("Loan market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut loan_market_reserve = MarketReserve::unpack(&loan_market_reserve_info.try_borrow_data()?)?;
    if &loan_market_reserve.manager != manager_info.key {
        msg!("Manager of loan market reserve is not matched with manager account provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if loan_market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 7
    let loan_supply_account_info = next_account_info(account_info_iter)?;
    if loan_supply_account_info.key != &loan_market_reserve.token_config.supply_account {
        msg!("Supply token account in loan market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 8
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if user_obligation.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    let friend_obligation = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 9
        let friend_obligation_info = next_account_info(account_info_iter)?;
        if friend_obligation_info.key != friend {
            msg!("Friend obligation provided is not matched with friend in user obligation");
            return Err(LendingError::ObligationInvalidFriend.into());
        }
        let friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;
        if friend_obligation.last_update.is_lax_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }

        Some(friend_obligation)
    } else {
        None
    };
    // 9/10
    let user_authority_info = next_account_info(account_info_iter)?;
    // 10/11
    let token_program_id = next_account_info(account_info_iter)?;
    // 11/12
    let flash_program_id = next_account_info(account_info_iter)?;
    if flash_program_id.key == program_id {
        msg!("Cross program can not be lending program");
        return Err(ProgramError::IncorrectProgramId);
    }
    //
    let collateral_index = user_obligation.find_collateral(collateral_market_reserve_info.key)?;
    let loan_index = user_obligation.find_loan(loan_market_reserve_info.key)?;
    let (sotoken_amount, settle) = user_obligation.liquidate::<IS_COLLATERAL>(
        amount,
        collateral_index,
        loan_index,
        &collateral_market_reserve,
        &loan_market_reserve,
        friend_obligation,
    )?;
    user_obligation.last_update.mark_stale();
    // accure interest
    loan_market_reserve.accrue_interest(clock.slot)?;
    loan_market_reserve.last_update.update_slot(clock.slot, true);
    // user flash borrow repaying-loan from reserve
    let (flash_loan_total_repay, flash_loan_fee) = loan_market_reserve.liquidity_info.flash_loan_borrow_out(settle.amount)?;
    // user repay in loan reserve
    loan_market_reserve.liquidity_info.repay(&settle)?;
    // user got sotoken and withdraw immediately
    // remark: token mint + token burn are all omitted here!
    collateral_market_reserve.accrue_interest(clock.slot)?;
    collateral_market_reserve.last_update.update_slot(clock.slot, true);
    let collateral_amount = collateral_market_reserve.withdraw(sotoken_amount)?;
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(loan_market_reserve, &mut loan_market_reserve_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(collateral_market_reserve, &mut collateral_market_reserve_info.try_borrow_mut_data()?)?;

    // record loan balance before
    let expect_loan_balance_after = Account::unpack(&loan_supply_account_info.try_borrow_data()?)?.amount
        .checked_add(flash_loan_total_repay)
        .ok_or(LendingError::MathOverflow)?;

    // transfer collateral from manager to user
    process_token_approve(
        token_program_id,
        collateral_supply_account_info,
        user_authority_info,
        manager_authority_info,
        collateral_amount,
        authority_signer_seeds,
    )?;

    // prepare instruction and account infos    
    let mut flash_instruction_account_infos = vec![
        clock_info.clone(),
        collateral_supply_account_info.clone(),
        loan_supply_account_info.clone(),
        user_authority_info.clone(),
        token_program_id.clone(),
    ];
    // 12/13 ~
    flash_instruction_account_infos.extend(account_info_iter.map(|account_info| account_info.clone()));

    process_invoke(
        FlashLiquidationData { tag, collateral_amount, flash_loan_total_repay },
        flash_program_id,
        flash_instruction_account_infos,
        &[],
    )?;

    process_token_revoke(
        token_program_id,
        collateral_supply_account_info,
        manager_authority_info,
        authority_signer_seeds,
    )?;

    // check loan balance after balance
    let loan_balance_after = Account::unpack(&loan_supply_account_info.try_borrow_data()?)?.amount;
    if loan_balance_after < expect_loan_balance_after {
        return Err(LendingError::FlashLoanRepayInsufficient.into());
    }
    // repay in loan reserve
    let mut loan_market_reserve = MarketReserve::unpack(&loan_market_reserve_info.try_borrow_data()?)?;
    loan_market_reserve.liquidity_info.flash_loan_repay(settle.amount, flash_loan_fee)?;
    MarketReserve::pack(loan_market_reserve, &mut loan_market_reserve_info.try_borrow_mut_data()?)
}

// must after update obligation
fn process_open_leverage_position_by_dex<const DEX_TYPE: DexType>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    borrow_amount: u64,
    min_collateral_amount: u64,
) -> ProgramResult {
    let borrow_amount = handle_amount(borrow_amount, || {
        msg!("Open leverage position borrow amount provided cannot be zero");
    })?;
    if min_collateral_amount == 0 {
        msg!("Open leverage position min collateral amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter().peekable();
    // 1
    let clock_info = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(clock_info)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let collateral_market_reserve_info = next_account_info(account_info_iter)?;
    if collateral_market_reserve_info.owner != program_id {
        msg!("Collateral market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut collateral_market_reserve = MarketReserve::unpack(&collateral_market_reserve_info.try_borrow_data()?)?;
    if &collateral_market_reserve.manager != manager_info.key {
        msg!("Manager of collateral market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if collateral_market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let collateral_supply_account_info = next_account_info(account_info_iter)?;
    if collateral_supply_account_info.key != &collateral_market_reserve.token_config.supply_account {
        msg!("Supply token account in collateral market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 6
    let loan_market_reserve_info = next_account_info(account_info_iter)?;
    if loan_market_reserve_info.owner != program_id {
        msg!("Loan market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut loan_market_reserve = MarketReserve::unpack(&loan_market_reserve_info.try_borrow_data()?)?;
    if &loan_market_reserve.manager != manager_info.key {
        msg!("Manager of loan market reserve is not matched with manager account provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if loan_market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 7
    let loan_supply_account_info = next_account_info(account_info_iter)?;
    if loan_supply_account_info.key != &loan_market_reserve.token_config.supply_account {
        msg!("Supply token account in loan market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 8
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if user_obligation.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    let friend_obligation = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 9
        let friend_obligation_info = next_account_info(account_info_iter)?;
        if friend_obligation_info.key != friend {
            msg!("Friend obligation provided is not matched with friend in user obligation");
            return Err(LendingError::ObligationInvalidFriend.into());
        }
        let friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;
        if friend_obligation.last_update.is_lax_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }

        Some(friend_obligation)
    } else {
        None
    };
    // 9/10
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        msg!("User authority provided is not matched with user obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 10/11
    let token_program_id = next_account_info(account_info_iter)?;

    // user borrow from reserve first
    let borrow_amount = calculate_amount(borrow_amount, loan_market_reserve.liquidity_info.available);
    let collateral_amount = match DEX_TYPE {
        ORCA_DEX => {
            let swap_ctx = OrcaSwapContext {
                swap_program: next_account_info(account_info_iter)?,
                token_program: token_program_id,
                pool_info: next_account_info(account_info_iter)?,
                pool_authority: next_account_info(account_info_iter)?,
                pool_lp_token_mint: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                pool_fee_account: next_account_info(account_info_iter)?,
                user_authority: manager_authority_info,
                user_source_token_account: loan_supply_account_info,
                user_dest_token_account: collateral_supply_account_info,
                signer_seeds: authority_signer_seeds,
            };
            // check ctx
            if !swap_ctx.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let collateral_amount_before = swap_ctx.get_user_dest_token_balance()?;
            // do swap
            swap_ctx.swap(borrow_amount, min_collateral_amount)?;
            // after swap
            swap_ctx.get_user_dest_token_balance()?
                .checked_sub(collateral_amount_before)
                .ok_or(LendingError::MathOverflow)?
        }
        _ => unreachable!("invalid dex type {}", DEX_TYPE)
    };
    
    // deposit
    // accure interest
    collateral_market_reserve.accrue_interest(clock.slot)?;
    collateral_market_reserve.last_update.update_slot(clock.slot, true);
    let mint_amount = collateral_market_reserve.deposit(collateral_amount)?;
    // pledge in obligation
    let _ = if let Ok(index) = user_obligation.find_collateral(collateral_market_reserve_info.key) {
        user_obligation.pledge(mint_amount, None, index)?
    } else {
        user_obligation.new_pledge(mint_amount, None, *collateral_market_reserve_info.key, &collateral_market_reserve)?
    };
    // borrow
    let borrow_amount = if let Ok(index) = user_obligation.find_loan(loan_market_reserve_info.key) {
        user_obligation.borrow_in(
            Some(borrow_amount),
            index,
            &loan_market_reserve,
            friend_obligation,
        )?
    } else {
        user_obligation.new_borrow_in(
            Some(borrow_amount),
            *loan_market_reserve_info.key,
            &loan_market_reserve,
            friend_obligation,
        )?
    };
    user_obligation.last_update.mark_stale();
    // accure interest
    loan_market_reserve.accrue_interest(clock.slot)?;
    loan_market_reserve.last_update.update_slot(clock.slot, true);
    // borrow in reserve
    loan_market_reserve.liquidity_info.borrow_out(borrow_amount)?;
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(loan_market_reserve, &mut loan_market_reserve_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(collateral_market_reserve, &mut collateral_market_reserve_info.try_borrow_mut_data()?)
}

fn process_easy_repay_by_dex<const DEX_TYPE: DexType>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sotoken_amount: u64,
    min_repay_amount: u64,
) -> ProgramResult {
    let sotoken_amount = handle_amount(sotoken_amount, || {
        msg!("Easy repay sotoken amount provided cannot be zero");
    })?;
    if min_repay_amount == 0 {
        msg!("Easy repay min repaying amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter().peekable();
    // 1
    let clock_info = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(clock_info)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let collateral_market_reserve_info = next_account_info(account_info_iter)?;
    if collateral_market_reserve_info.owner != program_id {
        msg!("Collateral market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut collateral_market_reserve = MarketReserve::unpack(&collateral_market_reserve_info.try_borrow_data()?)?;
    if &collateral_market_reserve.manager != manager_info.key {
        msg!("Manager of collateral market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if collateral_market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let collateral_supply_account_info = next_account_info(account_info_iter)?;
    if collateral_supply_account_info.key != &collateral_market_reserve.token_config.supply_account {
        msg!("Supply token account in collateral market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 6
    let loan_market_reserve_info = next_account_info(account_info_iter)?;
    if loan_market_reserve_info.owner != program_id {
        msg!("Loan market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut loan_market_reserve = MarketReserve::unpack(&loan_market_reserve_info.try_borrow_data()?)?;
    if &loan_market_reserve.manager != manager_info.key {
        msg!("Manager of loan market reserve is not matched with manager account provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if loan_market_reserve.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 7
    let loan_supply_account_info = next_account_info(account_info_iter)?;
    if loan_supply_account_info.key != &loan_market_reserve.token_config.supply_account {
        msg!("Supply token account in loan market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 8
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if user_obligation.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    let friend_obligation = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 9
        let friend_obligation_info = next_account_info(account_info_iter)?;
        if friend_obligation_info.key != friend {
            msg!("Friend obligation provided is not matched with friend in user obligation");
            return Err(LendingError::ObligationInvalidFriend.into());
        }
        let friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;
        if friend_obligation.last_update.is_lax_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }

        Some(friend_obligation)
    } else {
        None
    };
    // 9/10
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        msg!("User authority provided is not matched with user obligation owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 10/11
    let token_program_id = next_account_info(account_info_iter)?;

    let collateral_index = user_obligation.find_collateral(collateral_market_reserve_info.key)?;
    // redeem without remove
    let sotoken_amount = user_obligation.redeem::<false, false>(sotoken_amount, collateral_index, &collateral_market_reserve, friend_obligation.clone())?;
    let collateral_amount = collateral_market_reserve.withdraw(sotoken_amount)?;
    match DEX_TYPE {
        ORCA_DEX => {
            let mut swap_ctx = OrcaSwapContext {
                swap_program: next_account_info(account_info_iter)?,
                token_program: token_program_id,
                pool_info: next_account_info(account_info_iter)?,
                pool_authority: next_account_info(account_info_iter)?,
                pool_lp_token_mint: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                pool_fee_account: next_account_info(account_info_iter)?,
                user_authority: manager_authority_info,
                user_source_token_account: collateral_supply_account_info,
                user_dest_token_account: loan_supply_account_info,
                signer_seeds: authority_signer_seeds,
            };
            // check ctx
            if !swap_ctx.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let loan_amount_before = swap_ctx.get_user_dest_token_balance()?;
            // do swap
            swap_ctx.swap(collateral_amount, min_repay_amount)?;
            // after swap
            let actual_repay_amount = swap_ctx.get_user_dest_token_balance()?
                .checked_sub(loan_amount_before)
                .ok_or(LendingError::MathOverflow)?;

            let loan_index = user_obligation.find_loan(loan_market_reserve_info.key)?;
            let settle = user_obligation.repay(
                if Decimal::from(actual_repay_amount) < user_obligation.loans[loan_index].borrowed_amount_wads {
                    Some(actual_repay_amount)
                } else {
                    None
                },
                u64::MAX,
                loan_index,
            )?;
            // validate health
            user_obligation.validate_health(friend_obligation)?;
            // accrue interest
            loan_market_reserve.accrue_interest(clock.slot)?;
            loan_market_reserve.last_update.update_slot(clock.slot, true);
            // user repay in loan reserve
            loan_market_reserve.liquidity_info.repay(&settle)?;

            if actual_repay_amount > settle.amount {
                let pool_dest_token_account = swap_ctx.pool_source_token_account;
                swap_ctx.pool_source_token_account = swap_ctx.pool_dest_token_account;
                swap_ctx.pool_dest_token_account = pool_dest_token_account;

                let user_dest_token_account = swap_ctx.user_source_token_account;
                swap_ctx.user_source_token_account = swap_ctx.user_dest_token_account;
                swap_ctx.user_dest_token_account = user_dest_token_account;

                // before swap
                let collateral_amount_before = swap_ctx.get_user_dest_token_balance()?;
                // do swap back
                swap_ctx.swap(actual_repay_amount - settle.amount, 1)?;
                // after swap
                let collateral_amount_after = swap_ctx.get_user_dest_token_balance()?
                    .checked_sub(collateral_amount_before)
                    .ok_or(LendingError::MathOverflow)?;
                // accure interest
                collateral_market_reserve.accrue_interest(clock.slot)?;
                collateral_market_reserve.last_update.update_slot(clock.slot, true);
                // deposit back to collateral reserve
                let mint_amount = collateral_market_reserve.deposit(collateral_amount_after)?;
                user_obligation.pledge(mint_amount, None, collateral_index)?;
            }
        }
        _ => unreachable!("invalid dex type {}", DEX_TYPE)
    };
    
    user_obligation.last_update.mark_stale();
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(loan_market_reserve, &mut loan_market_reserve_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(collateral_market_reserve, &mut collateral_market_reserve_info.try_borrow_mut_data()?)
}

// by manager
#[cfg(feature = "unique-credit")]
fn process_init_unique_credit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    credit_authority: Pubkey,
    borrow_limit: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let rent = &Rent::from_account_info( next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Manager of market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into())
    }
    // 5
    let unique_credit_info = next_account_info(account_info_iter)?;
    if unique_credit_info.owner != program_id {
        msg!("Unique credit provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, unique_credit_info)?;
    assert_uninitialized::<UniqueCredit>(unique_credit_info)?;
    // 6
    let authority_info = next_account_info(account_info_iter)?;
    if authority_info.key != &manager.owner {
        msg!("Only manager owner can create unique credit");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }

    let unique_credit = UniqueCredit::new(
        credit_authority,
        *manager_info.key,
        *market_reserve_info.key,
        borrow_limit,
    );
    UniqueCredit::pack(unique_credit, &mut unique_credit_info.try_borrow_mut_data()?)
}

#[inline(never)]
#[cfg(feature = "unique-credit")]
fn process_borrow_liquidity_by_unique_credit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed],
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Manager of market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    // 5
    let supply_token_account_info = next_account_info(account_info_iter)?;
    if supply_token_account_info.key != &market_reserve.token_config.supply_account {
        msg!("Supply token account in market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 6
    let unique_credit_info = next_account_info(account_info_iter)?;
    if unique_credit_info.owner != program_id {
        msg!("Unique credit provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut unique_credit = UniqueCredit::unpack(&unique_credit_info.try_borrow_data()?)?;
    if &unique_credit.reserve != market_reserve_info.key {
        msg!("Reserve of unique credit is not matched with market reserve provided");
        return Err(LendingError::InvalidUniqueCredit.into());
    }
    // 7
    let authority_info = next_account_info(account_info_iter)?;
    if authority_info.key != &unique_credit.owner {
        msg!("Only manager owner can create unique credit");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 8
    let token_program_id = next_account_info(account_info_iter)?;

    // accrue interest
    market_reserve.accrue_interest(clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    // borrow in credit
    unique_credit.accrue_interest(&market_reserve)?;
    let amount = unique_credit.borrow_in(amount, &market_reserve)?;
    // borrow in reserve
    market_reserve.liquidity_info.borrow_out(amount)?;
    // pack
    UniqueCredit::pack(unique_credit, &mut unique_credit_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

    // approve to credit owner
    spl_token_approve(TokenApproveParams {
        source: supply_token_account_info.clone(),
        delegate: authority_info.clone(),
        amount,
        authority: manager_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_id.clone(),
    })
}

#[inline(never)]
#[cfg(feature = "unique-credit")]
fn process_repay_loan_by_unique_credit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed],
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Manager of market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    // 5
    let supply_token_account_info = next_account_info(account_info_iter)?;
    if supply_token_account_info.key != &market_reserve.token_config.supply_account {
        msg!("Supply account in market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 6
    let unique_credit_info = next_account_info(account_info_iter)?;
    if unique_credit_info.owner != program_id {
        msg!("Unique credit provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut unique_credit = UniqueCredit::unpack(&unique_credit_info.try_borrow_data()?)?;
    if &unique_credit.reserve != market_reserve_info.key {
        msg!("Reserve of unique credit is not matched with market reserve provided");
        return Err(LendingError::InvalidUniqueCredit.into());
    }
    // 7
    let source_token_account_info = next_account_info(account_info_iter)?;
    let source_token_account = Account::unpack(&source_token_account_info.try_borrow_data()?)?;
    if &source_token_account.owner == manager_authority_info.key {
        msg!("Source token account owner should not be manager authority");
        return Err(LendingError::InvalidTokenAccountOwner.into()); 
    }
    let source_token_account = Account::unpack(&source_token_account_info.try_borrow_data()?)?;
    // 8
    let token_program_id = next_account_info(account_info_iter)?;

    // accrue interest
    market_reserve.accrue_interest(clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    // repay in obligation
    unique_credit.accrue_interest(&market_reserve)?;
    let settle = unique_credit.repay(source_token_account.amount.min(source_token_account.delegated_amount), amount)?;
    // repay in reserve
    market_reserve.liquidity_info.repay(&settle)?;
    // pack
    UniqueCredit::pack(unique_credit, &mut unique_credit_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

    // transfer token to market reserve
    spl_token_transfer(TokenTransferParams {
        source: source_token_account_info.clone(),
        destination: supply_token_account_info.clone(),
        amount: settle.amount,
        authority: manager_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_id.clone(),
    })
}

// by manager
fn process_operate_user_obligation<P: Any + Param>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    param: P,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    // 2
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User obligation provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        msg!("User obligation manager provided is not matched with manager info");
        return Err(LendingError::InvalidManager.into());
    }
    // 3
    let authority_info = next_account_info(account_info_iter)?;
    if authority_info.key != &manager.owner {
        msg!("Only manager owner can operate user obligation");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }

    user_obligation.operate(param)?;
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)
}

// by manager
fn process_operate_market_reserve<P: Any + Param>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    param: P,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    // 2
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Manager of market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    // 3
    let authority_info = next_account_info(account_info_iter)?;
    if authority_info.key != &manager.owner {
        msg!("Only manager owner can operate market reserve");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }

    market_reserve.operate(param)?;
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)
}

// by manager
#[inline(never)]
fn process_reduce_insurance(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Reduce insurance amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    // 1
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed],
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;
    // 2
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 3
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Manager of market reserve is not matched with manager provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    // 4
    let supply_token_account_info = next_account_info(account_info_iter)?;
    if supply_token_account_info.key != &market_reserve.token_config.supply_account {
        msg!("Supply account in market reserve is not matched with supply token account provided");
        return Err(LendingError::InvalidSupplyTokenAccount.into()); 
    }
    // 5
    let authority_info = next_account_info(account_info_iter)?;
    if authority_info.key != &manager.owner {
        msg!("Only manager owner can reduce insurance");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 6
    let receiver_token_account_info = next_account_info(account_info_iter)?;
    // 7
    let token_program_id = next_account_info(account_info_iter)?;

    // reduce insurance
    market_reserve.liquidity_info.reduce_insurance(amount)?;
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    // transfer
    process_token_transfer(
        token_program_id,
        supply_token_account_info,
        receiver_token_account_info,
        manager_authority_info,
        amount,
        authority_signer_seeds,
    )
}

fn process_change_manager_owner(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    // 2
    let authority_info = next_account_info(account_info_iter)?;
    if authority_info.key != &manager.owner {
        msg!("Only manager owner can change owner");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }
    // 3
    let new_authority_info = next_account_info(account_info_iter)?;
    if !new_authority_info.is_signer {
        msg!("New authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }

    manager.owner = *new_authority_info.key;
    Manager::pack(manager, &mut manager_info.try_borrow_mut_data()?)
}

// by manager
#[cfg(feature = "unique-credit")]
fn process_update_unique_credit_limit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    // 2
    let unique_credit_info = next_account_info(account_info_iter)?;
    if unique_credit_info.owner != program_id {
        msg!("Unique credit provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut unique_credit = UniqueCredit::unpack(&unique_credit_info.try_borrow_data()?)?;
    if &unique_credit.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    // 3
    let authority_info = next_account_info(account_info_iter)?;
    if authority_info.key != &manager.owner {
        msg!("Only manager owner can update unique credit limit");
        return Err(LendingError::InvalidAuthority.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidAuthority.into());
    }

    // update borrow limit
    unique_credit.borrow_limit = amount;
    UniqueCredit::pack(unique_credit, &mut unique_credit_info.try_borrow_mut_data()?)
}

#[inline(always)]
fn get_available_balance(account: Account, authority_key: &Pubkey) -> u64 {
    if let COption::Some(ref delegate) = account.delegate {
        if delegate == authority_key {
            return account.amount.min(account.delegated_amount);
        }
    }

    if &account.owner == authority_key {
        account.amount
    } else {
        0
    }
}

#[inline(always)]
fn get_token_decimals(account_info: &AccountInfo) -> Result<u8, ProgramError> {
    if account_info.key == &native_mint::id() {
        Ok(native_mint::DECIMALS)
    } else {
        Ok(Mint::unpack(&account_info.try_borrow_data()?)?.decimals)
    }
}

struct FlashLoanData {
    tag: u8,
    flash_loan_total_repay: u64,
}

impl Data for FlashLoanData {
    fn to_vec(self) -> Vec<u8> {
        let mut flash_loan_data = Vec::with_capacity(1 + 8);
        flash_loan_data.push(self.tag);
        flash_loan_data.extend_from_slice(&self.flash_loan_total_repay.to_le_bytes());

        flash_loan_data
    }
}

struct FlashLiquidationData {
    tag: u8,
    collateral_amount: u64,
    flash_loan_total_repay: u64,
}

impl Data for FlashLiquidationData {
    fn to_vec(self) -> Vec<u8> {
        let mut data = Vec::with_capacity(1 + 8 + 8);
        data.push(self.tag);
        data.extend_from_slice(&self.collateral_amount.to_le_bytes());
        data.extend_from_slice(&self.flash_loan_total_repay.to_le_bytes());

        data
    }
}

impl PrintProgramError for LendingError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        msg!(&self.to_string());
    }
}