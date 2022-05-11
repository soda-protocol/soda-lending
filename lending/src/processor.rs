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
    assert_rent_exempt,
    assert_uninitialized,
    handle_amount,
    Data,
    dex::{OrcaSwapContext, Swapper, RaydiumSwapContext, DexType, ORCA, RAYDIUM, ORCA_TWICE},
    error::LendingError,
    instruction::LendingInstruction,
    invoker::*,
    state::*,
    oracle::OracleConfig,
    get_rent,
    get_clock,
    create_manager,
    create_market_reserve,
    create_user_obligation,
    get_manager,
    get_mut_manager,
    get_manager_authority,
    get_market_reserve,
    get_mut_market_reserve,
    get_mut_user_obligation,
    get_friend_obligation,
    get_signer,
    get_manager_owner,
    get_user_obligation_owner,
    get_sotoken_mint,
    get_supply_account,
    get_receiver_program,
};
#[cfg(feature = "unique-credit")]
use crate::{
    state::UniqueCredit,
    create_unique_credit,
    get_unique_credit,
    get_unique_credit_owner,
};

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
            msg!("Instruction: Deposit {}", amount);
            process_deposit_or_withdraw::<true>(program_id, accounts, amount)
        }
        LendingInstruction::Withdraw (amount) => {
            msg!("Instruction: Withdraw {}", amount);
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
            msg!("Instruction: Pledge Collateral {}", amount);
            process_pledge_collateral(program_id, accounts, amount)
        }
        LendingInstruction::DepositAndPledge(amount) => {
            msg!("Instruction: Deposit Liquidity and Pledge: {}", amount);
            process_deposit_and_pledge(program_id, accounts, amount)
        }
        LendingInstruction::RedeemCollateral(amount) => {
            msg!("Instruction: Redeem Collateral {}", amount);
            process_redeem_collateral(program_id, accounts, amount)
        }
        LendingInstruction::RedeemAndWithdraw(amount) => {
            msg!("Instruction: Redeem Collateral and Withdraw {}", amount);
            process_redeem_and_withdraw::<true>(program_id, accounts, amount)
        }
        LendingInstruction::RedeemCollateralWithoutLoan(amount) => {
            msg!("Instruction: Redeem Collateral Without Loan {}", amount);
            process_redeem_collateral_without_loan(program_id, accounts, amount)
        }
        LendingInstruction::RedeemWithoutLoanAndWithdraw(amount) => {
            msg!("Instruction: Redeem Collateral Without Loan And Withdraw {}", amount);
            process_redeem_and_withdraw::<false>(program_id, accounts, amount)
        }
        LendingInstruction::ReplaceCollateral(amount) => {
            msg!("Instruction: Replace Collateral {},", amount);
            process_replace_collateral(program_id, accounts, amount)
        }
        LendingInstruction::BorrowLiquidity(amount) => {
            msg!("Instruction: Borrow Liquidity {}", amount);
            process_borrow_liquidity(program_id, accounts, amount)
        }
        LendingInstruction::RepayLoan(amount) => {
            msg!("Instruction: Repay Loan {}", amount);
            process_repay_loan(program_id, accounts, amount)
        }
        LendingInstruction::LiquidateByCollateral(amount) => {
            msg!("Instruction: Liquidate by collateral {}", amount);
            process_liquidate::<true>(program_id, accounts, amount)
        }
        LendingInstruction::LiquidateByLoan(amount) => {
            msg!("Instruction: Liquidate by loan {}", amount);
            process_liquidate::<false>(program_id, accounts, amount)
        }
        LendingInstruction::FlashLiquidationByCollateral(tag, amount) => {
            msg!("Instruction: Flash Liquidation by Collateral {}", amount);
            process_flash_liquidate::<true>(program_id, accounts, tag, amount)
        }
        LendingInstruction::FlashLiquidationByLoan(tag, amount) => {
            msg!("Instruction: Flash Liquidation by Loan {}", amount);
            process_flash_liquidate::<false>(program_id, accounts, tag, amount)
        }
        LendingInstruction::FlashLoan(tag, amount) => {
            msg!("Instruction: Flash Loan {}", amount);
            process_flash_loan(program_id, accounts, tag, amount)
        }
        LendingInstruction::EasyRepayByOrcaBaseIn(sotoken_amount, min_repay_amount) => {
            msg!("Instruction: Easy Repay By Orca with Base In: collateral {}, min repay {}", sotoken_amount, min_repay_amount);
            process_easy_repay_base_in::<ORCA>(program_id, accounts, sotoken_amount, min_repay_amount)
        }
        LendingInstruction::OpenLeveragePositionByOrcaBaseIn(borrow_amount, min_collateral_amount) => {
            msg!("Instruction: Open Leverage Position By Orca with Base In: borrow {}, min collateral {}", borrow_amount, min_collateral_amount);
            process_open_leverage_position_base_in::<ORCA>(program_id, accounts, borrow_amount, min_collateral_amount)
        }
        LendingInstruction::EasyRepayByOrcaTwiceBaseIn(sotoken_amount, min_repay_amount) => {
            msg!("Instruction: Easy Repay By Orca Twice with Base In: collateral {}, min repay {}", sotoken_amount, min_repay_amount);
            process_easy_repay_base_in::<ORCA_TWICE>(program_id, accounts, sotoken_amount, min_repay_amount)
        }
        LendingInstruction::OpenLeveragePositionByOrcaTwiceBaseIn(borrow_amount, min_collateral_amount) => {
            msg!("Instruction: Open Leverage Position By Orca Twice with Base In: borrow {}, min collateral {}", borrow_amount, min_collateral_amount);
            process_open_leverage_position_base_in::<ORCA_TWICE>(program_id, accounts, borrow_amount, min_collateral_amount)
        }
        LendingInstruction::EasyRepayByRaydiumBaseIn(sotoken_amount, min_repay_amount) => {
            msg!("Instruction: Easy Repay By Raydium with Base In: collateral {}, min repay {}", sotoken_amount, min_repay_amount);
            process_easy_repay_base_in::<RAYDIUM>(program_id, accounts, sotoken_amount, min_repay_amount)
        }
        LendingInstruction::EasyRepayByRaydiumBaseOut(max_sotoken_amount, repay_amount) => {
            msg!("Instruction: Easy Repay By Raydium with Base Out: max collateral {}, repay {}", max_sotoken_amount, repay_amount);
            process_easy_repay_base_out::<RAYDIUM>(program_id, accounts, max_sotoken_amount, repay_amount)
        }
        LendingInstruction::OpenLeveragePositionByRaydiumBaseIn(borrow_amount, min_collateral_amount) => {
            msg!("Instruction: Open Leverage Position By Raydium with Base In: borrow {}, min collateral {}", borrow_amount, min_collateral_amount);
            process_open_leverage_position_base_in::<RAYDIUM>(program_id, accounts, borrow_amount, min_collateral_amount)
        }
        LendingInstruction::OpenLeveragePositionByRaydiumBaseOut(max_borrow_amount, collateral_amount) => {
            msg!("Instruction: Open Leverage Position By Raydium with Base In: max borrow {}, collateral {}", max_borrow_amount, collateral_amount);
            process_open_leverage_position_base_out::<RAYDIUM>(program_id, accounts, max_borrow_amount, collateral_amount)
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
            msg!("Instruction: Reduce Insurance {}", amount);
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
    get_rent!(rent_info, rent; account_info_iter);
    // 2
    create_manager!(manager_info; account_info_iter, program_id, rent);
    // 3
    get_signer!(authority_info; account_info_iter);
    
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
    get_rent!(rent_info, rent; account_info_iter);
    // 2
    get_clock!(clock_info, clock; account_info_iter);
    // 3
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 4
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 5
    let supply_token_account_info = next_account_info(account_info_iter)?;
    // 6
    create_market_reserve!(market_reserve_info; account_info_iter, program_id, rent);
    // 7
    let token_mint_info = next_account_info(account_info_iter)?;
    let token_decimals = get_token_decimals(token_mint_info)?;
    // 8
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    // 9
    get_manager_owner!(manager_owner_info; account_info_iter, manager);
    // 10
    let token_program_info = next_account_info(account_info_iter)?;

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
        token_program_info,
        supply_token_account_info,
        token_mint_info,
        rent_info,
        manager_authority_info,
    )?;

    // init sotoken mint
    process_token_init_mint(
        token_program_info,
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
            market_reserve.oracle_info.update_price(price_oracle_info, clock)?;
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_mut_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id, manager_info.key);
    // 5
    get_sotoken_mint!(sotoken_mint_info; account_info_iter, market_reserve);
    // 6
    get_supply_account!(supply_token_account_info; account_info_iter, market_reserve);
    // 7
    let user_authority_info = next_account_info(account_info_iter)?;
    // 8
    let user_token_account_info = next_account_info(account_info_iter)?;
    // 9
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    // 10
    let token_program_info = next_account_info(account_info_iter)?;

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
            token_program_info,
            user_token_account_info,
            supply_token_account_info,
            user_authority_info,
            amount,
            &[],
        )?;

        // mint to user
        process_token_mint_to(
            token_program_info,
            sotoken_mint_info,
            user_sotoken_account_info,
            manager_authority_info,
            mint_amount,
            signer_seeds,
        )
    } else {
        let amount = calculate_amount(amount, Account::unpack(&user_sotoken_account_info.try_borrow_data()?)?.amount);
        let withdraw_amount = market_reserve.withdraw(amount)?;
        // pack
        MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    
        // burn sotoken
        process_token_burn(
            token_program_info,
            user_sotoken_account_info,
            sotoken_mint_info,
            user_authority_info,
            amount,
            &[],
        )?;
    
        // transfer from manager to user
        process_token_transfer(
            token_program_info,
            supply_token_account_info,
            user_token_account_info,
            manager_authority_info,
            withdraw_amount,
            signer_seeds,
        )
    }
}

fn process_init_user_obligation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    get_rent!(rent_info, rent; account_info_iter);
    // 2
    get_clock!(clock_info, clock; account_info_iter);
    // 3
    get_manager!(manager_info, _manager; account_info_iter, program_id);
    // 4
    create_user_obligation!(user_obligation_info; account_info_iter, program_id, rent);
    // 5
    get_signer!(user_authority_info; account_info_iter);

    let user_obligation = UserObligation::new(
        clock.slot,
        *manager_info.key,
        *user_authority_info.key,
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id);
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
                msg!("User obligation manager provided is matched with market reserve provided");
                return Err(LendingError::UnmatchedAccounts.into());
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
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id);
    // 2
    get_mut_user_obligation!(friend_obligation_info, friend_obligation; account_info_iter, program_id, &user_obligation.manager);
    // 3
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 4
    get_user_obligation_owner!(friend_authority_info; account_info_iter, friend_obligation);

    if user_obligation_info.key == friend_obligation_info.key {
        msg!("User obligation provided is not matched with friend obligation provided");
        return Err(LendingError::ObligationInvalidFriend.into())
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
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id);
    if user_obligation.last_update.is_lax_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }
    // 3
    get_mut_user_obligation!(friend_obligation_info, friend_obligation; account_info_iter, program_id, &user_obligation.manager, clock);
    // 4
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 5
    get_user_obligation_owner!(friend_authority_info; account_info_iter, friend_obligation);

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
    get_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id);
    // 2
    get_sotoken_mint!(sotoken_mint_info; account_info_iter, market_reserve);
    // 3
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, &market_reserve.manager);
    // 4
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 5
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    let user_sotoken_account = Account::unpack(&user_sotoken_account_info.try_borrow_data()?)?;
    // 6
    let token_program_info = next_account_info(account_info_iter)?;

    // handle obligation
    let balance = get_available_balance(user_sotoken_account, user_authority_info.key);
    let amount = if let Ok(index) = user_obligation.find_collateral(market_reserve_info.key) {
        user_obligation.pledge::<false>(balance, amount, index, &market_reserve)?
    } else {
        user_obligation.new_pledge::<false>(balance, amount, *market_reserve_info.key, &market_reserve)?
    };
    user_obligation.last_update.mark_stale();
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    
    // burn from user
    process_token_burn(
        token_program_info,
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_mut_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id);
    // 3
    get_supply_account!(supply_token_account_info; account_info_iter, market_reserve);
    // 4
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, &market_reserve.manager);
    // 5
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 6
    let user_token_account_info = next_account_info(account_info_iter)?;
    let user_token_account = Account::unpack(&user_token_account_info.try_borrow_data()?)?;
    // 7
    let token_program_info = next_account_info(account_info_iter)?;

    // accrue interest
    market_reserve.accrue_interest(clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    // deposit in reserve
    let amount = calculate_amount(amount, get_available_balance(user_token_account, user_authority_info.key));
    let mint_amount = market_reserve.deposit(amount)?;
    // pledge in obligation
    let _ = if let Ok(index) = user_obligation.find_collateral(market_reserve_info.key) {
        user_obligation.pledge::<false>(mint_amount, None, index, &market_reserve)?
    } else {
        user_obligation.new_pledge::<false>(mint_amount, None, *market_reserve_info.key, &market_reserve)?
    };
    user_obligation.last_update.mark_stale();
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;

    // transfer token to manager
    process_token_transfer(
        token_program_info,
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 5
    get_sotoken_mint!(sotoken_mint_info; account_info_iter, market_reserve);
    // 6
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, manager_info.key, clock);
    // 7?
    get_friend_obligation!(friend_obligation; account_info_iter, user_obligation, clock);
    // 7/8
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 8/9
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    // 9/10
    let token_program_info = next_account_info(account_info_iter)?;

    // redeem in obligation
    let index = user_obligation.find_collateral(market_reserve_info.key)?;
    let amount = user_obligation.redeem::<true, true>(amount, index, &market_reserve, friend_obligation)?;
    user_obligation.last_update.mark_stale();
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    
    // mint to user
    process_token_mint_to(
        token_program_info,
        sotoken_mint_info,
        user_sotoken_account_info,
        manager_authority_info,
        amount,
        signer_seeds,
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_mut_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id, manager_info.key, clock, WITH_LOAN);
    // 5
    get_supply_account!(supply_account_info; account_info_iter, market_reserve);
    // 6
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, manager_info.key, clock, WITH_LOAN);
    // 7?
    get_friend_obligation!(friend_obligation; account_info_iter, user_obligation, clock, WITH_LOAN);
    // 7/8
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 8/9
    let user_token_account_info = next_account_info(account_info_iter)?;
    // 9/10
    let token_program_info = next_account_info(account_info_iter)?;

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
        token_program_info,
        supply_account_info,
        user_token_account_info,
        manager_authority_info,
        withdraw_amount,
        signer_seeds,
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
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 2
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 3
    get_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id, manager_info.key);
    // 4
    get_sotoken_mint!(sotoken_mint_info; account_info_iter, market_reserve);
    // 5
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, manager_info.key);
    // 6?
    get_friend_obligation!(friend_obligation; account_info_iter, user_obligation);
    // 6/7
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 7/8
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    // 8/9
    let token_program_info = next_account_info(account_info_iter)?;

    // redeem in obligation
    let index = user_obligation.find_collateral(market_reserve_info.key)?;
    let amount = user_obligation.redeem_without_loan(amount, index, friend_obligation)?;
    user_obligation.last_update.mark_stale();
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    
    // mint to user
    process_token_mint_to(
        token_program_info,
        sotoken_mint_info,
        user_sotoken_account_info,
        manager_authority_info,
        amount,
        signer_seeds,
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_market_reserve!(out_market_reserve_info, out_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 5
    get_sotoken_mint!(out_sotoken_mint_info; account_info_iter, out_market_reserve);
    // 6
    get_market_reserve!(in_market_reserve_info, in_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 7
    get_sotoken_mint!(in_sotoken_mint_info; account_info_iter, in_market_reserve);
    // 8
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, manager_info.key, clock);
    // 9?
    get_friend_obligation!(friend_obligation; account_info_iter, user_obligation, clock);
    // 9/10
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 10/11
    let user_out_sotoken_account_info = next_account_info(account_info_iter)?;
    // 12/13
    let user_in_sotoken_account_info = next_account_info(account_info_iter)?;
    let user_in_sotoken_account = Account::unpack(&user_in_sotoken_account_info.try_borrow_data()?)?;
    // 13/14
    let token_program_info = next_account_info(account_info_iter)?;

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
        token_program_info,
        out_sotoken_mint_info,
        user_out_sotoken_account_info,
        manager_authority_info,
        out_amount,
        signer_seeds,
    )?;

    // burn from user
    process_token_burn(
        token_program_info,
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_mut_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 5
    get_supply_account!(supply_account_info; account_info_iter, market_reserve);
    // 6
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, manager_info.key, clock);
    // 7
    get_friend_obligation!(friend_obligation; account_info_iter, user_obligation, clock);
    // 7/8
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 8/9
    let user_token_account_info = next_account_info(account_info_iter)?;
    // 9/10
    let token_program_info = next_account_info(account_info_iter)?;

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
        token_program_info,
        supply_account_info,
        user_token_account_info,
        manager_authority_info,
        amount,
        signer_seeds,
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_mut_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id);
    // 3
    get_supply_account!(supply_account_info; account_info_iter, market_reserve);
    // 4
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, &market_reserve.manager);
    // 5
    let user_authority_info = next_account_info(account_info_iter)?;
    // 6
    let user_token_account_info = next_account_info(account_info_iter)?;
    let user_balance = Account::unpack(&user_token_account_info.try_borrow_data()?)?.amount;
    // 7
    let token_program_info = next_account_info(account_info_iter)?;    

    // accrue interest
    market_reserve.accrue_interest(clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    // repay in obligation
    let index = user_obligation.find_loan(market_reserve_info.key)?;
    user_obligation.loans[index].accrue_interest(&market_reserve)?;
    let settle = user_obligation.repay::<false>(amount, user_balance, index, &market_reserve)?;
    user_obligation.last_update.mark_stale();
    // repay in reserve 
    market_reserve.liquidity_info.repay(&settle)?;
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

    // transfer to manager
    process_token_transfer(
        token_program_info,
        user_token_account_info,
        supply_account_info,
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_market_reserve!(collateral_market_reserve_info, collateral_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 5
    get_sotoken_mint!(sotoken_mint_info; account_info_iter, collateral_market_reserve);
    // 6
    get_mut_market_reserve!(loan_market_reserve_info, loan_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 7
    get_supply_account!(supply_token_account_info; account_info_iter, loan_market_reserve);
    // 8
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, &loan_market_reserve.manager, clock);
    // 9
    get_friend_obligation!(friend_obligation; account_info_iter, user_obligation, clock);
    // 9/10
    let liquidator_authority_info = next_account_info(account_info_iter)?;
    // 10/11
    let liquidator_token_account_info = next_account_info(account_info_iter)?;
    // 11/12
    let liquidator_sotoken_account_info = next_account_info(account_info_iter)?;
    // 12/13
    let token_program_info = next_account_info(account_info_iter)?;

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
        token_program_info,
        liquidator_token_account_info,
        supply_token_account_info,
        liquidator_authority_info,
        settle.amount,
        &[],
    )?;

    // mint to user
    process_token_mint_to(
        token_program_info,
        sotoken_mint_info,
        liquidator_sotoken_account_info,
        manager_authority_info,
        so_token_amount,
        signer_seeds,
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_mut_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id, manager_info.key);
    // 5
    get_supply_account!(supply_account_info; account_info_iter, market_reserve);
    // 6
    let receiver_authority_info = next_account_info(account_info_iter)?;
    // 7
    let token_program_info = next_account_info(account_info_iter)?;
    // 8
    get_receiver_program!(receiver_program_id; account_info_iter, program_id);

    // accrue interest
    market_reserve.accrue_interest(clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    // flash loan borrow calculate
    let borrow_amount = calculate_amount(amount, market_reserve.liquidity_info.available);
    let (flash_loan_total_repay, flash_loan_fee) = market_reserve.liquidity_info.flash_loan_borrow_out(borrow_amount)?;
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

    let expect_balance_after_flash_loan = Account::unpack(&supply_account_info.try_borrow_data()?)?.amount
        .checked_add(flash_loan_fee)
        .ok_or(LendingError::MathOverflow)?;

    // approve to receiver
    process_token_approve(
        token_program_info,
        supply_account_info,
        receiver_authority_info,
        manager_authority_info,
        borrow_amount,
        signer_seeds,
    )?;

    // prepare instruction and account infos    
    let mut flash_loan_instruction_account_infos = vec![
        clock_info.clone(),
        supply_account_info.clone(),
        receiver_authority_info.clone(),
        token_program_info.clone(),
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
        token_program_info,
        supply_account_info,
        manager_authority_info,
        signer_seeds,
    )?;

    // check balance
    let balance_after = Account::unpack(&supply_account_info.try_borrow_data()?)?.amount;
    if balance_after < expect_balance_after_flash_loan {
        return Err(LendingError::FlashLoanRepayInsufficient.into());
    }
    // check if reserve changed during flash loan
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    market_reserve.liquidity_info.flash_loan_repay(borrow_amount, flash_loan_fee)?;
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)
}

#[inline(never)]
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_mut_market_reserve!(collateral_market_reserve_info, collateral_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 5
    get_supply_account!(collateral_supply_account_info; account_info_iter, collateral_market_reserve);
    // 6
    get_mut_market_reserve!(loan_market_reserve_info, loan_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 7
    get_supply_account!(loan_supply_account_info; account_info_iter, loan_market_reserve);
    // 8
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, manager_info.key, clock);
    // 9
    get_friend_obligation!(friend_obligation; account_info_iter, user_obligation, clock);
    // 9/10
    let user_authority_info = next_account_info(account_info_iter)?;
    // 10/11
    let token_program_info = next_account_info(account_info_iter)?;
    // 11/12
    get_receiver_program!(flash_program_id; account_info_iter, program_id);

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
        token_program_info,
        collateral_supply_account_info,
        user_authority_info,
        manager_authority_info,
        collateral_amount,
        signer_seeds,
    )?;

    // prepare instruction and account infos    
    let mut flash_instruction_account_infos = vec![
        clock_info.clone(),
        collateral_supply_account_info.clone(),
        loan_supply_account_info.clone(),
        user_authority_info.clone(),
        token_program_info.clone(),
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
        token_program_info,
        collateral_supply_account_info,
        manager_authority_info,
        signer_seeds,
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
#[inline(never)]
fn process_open_leverage_position_base_in<const DEX_TYPE: DexType>(
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_mut_market_reserve!(collateral_market_reserve_info, collateral_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 5
    get_supply_account!(collateral_supply_account_info; account_info_iter, collateral_market_reserve);
    // 6
    get_mut_market_reserve!(loan_market_reserve_info, loan_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 7
    get_supply_account!(loan_supply_account_info; account_info_iter, loan_market_reserve);
    // 8
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, manager_info.key, clock);
    // 9?
    get_friend_obligation!(friend_obligation; account_info_iter, user_obligation, clock);
    // 9/10
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 10/11
    let token_program_info = next_account_info(account_info_iter)?;
    // 11/12
    let swap_program_info = next_account_info(account_info_iter)?;

    // user borrow from reserve first
    let borrow_amount = calculate_amount(borrow_amount, loan_market_reserve.liquidity_info.available);
    let collateral_amount = match DEX_TYPE {
        ORCA => {
            let swap_ctx = OrcaSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                pool_info: next_account_info(account_info_iter)?,
                pool_authority: next_account_info(account_info_iter)?,
                pool_lp_token_mint: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                pool_fee_account: next_account_info(account_info_iter)?,
                user_source_token_account: loan_supply_account_info,
                user_dest_token_account: collateral_supply_account_info,
                user_authority: manager_authority_info,
                signer_seeds,
            };
            // check ctx
            if !swap_ctx.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let collateral_amount_before = swap_ctx.get_user_dest_token_balance()?;
            // do swap
            swap_ctx.swap_base_in(borrow_amount, min_collateral_amount)?;
            // after swap
            swap_ctx.get_user_dest_token_balance()?
                .checked_sub(collateral_amount_before)
                .ok_or(LendingError::MathOverflow)?
        }
        ORCA_TWICE => {
            let temp_token_account = next_account_info(account_info_iter)?;
            let swap_ctx_1 = OrcaSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                pool_info: next_account_info(account_info_iter)?,
                pool_authority: next_account_info(account_info_iter)?,
                pool_lp_token_mint: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                pool_fee_account: next_account_info(account_info_iter)?,
                user_source_token_account: loan_supply_account_info,
                user_dest_token_account: temp_token_account,
                user_authority: manager_authority_info,
                signer_seeds,
            };
            let swap_ctx_2 = OrcaSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                pool_info: next_account_info(account_info_iter)?,
                pool_authority: next_account_info(account_info_iter)?,
                pool_lp_token_mint: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                pool_fee_account: next_account_info(account_info_iter)?,
                user_source_token_account: temp_token_account,
                user_dest_token_account: collateral_supply_account_info,
                user_authority: user_authority_info,
                signer_seeds: &[],
            };
            // check ctx
            if !swap_ctx_1.is_supported() || !swap_ctx_2.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let collateral_amount_before = swap_ctx_2.get_user_dest_token_balance()?;
            // do swap 1
            swap_ctx_1.swap_base_in(borrow_amount, 1)?;
            let temp_amount = swap_ctx_1.get_user_dest_token_balance()?;
            // do swap 2
            swap_ctx_2.swap_base_in(temp_amount, min_collateral_amount)?;
            // after swap
            swap_ctx_2.get_user_dest_token_balance()?
                .checked_sub(collateral_amount_before)
                .ok_or(LendingError::MathOverflow)?
        }
        RAYDIUM => {
            let swap_ctx = RaydiumSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                amm_info: next_account_info(account_info_iter)?,
                amm_authority: next_account_info(account_info_iter)?,
                amm_open_orders: next_account_info(account_info_iter)?,
                amm_target_orders: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                serum_program: next_account_info(account_info_iter)?,
                serum_market: next_account_info(account_info_iter)?,
                serum_bids: next_account_info(account_info_iter)?,
                serum_asks: next_account_info(account_info_iter)?,
                serum_event_queue: next_account_info(account_info_iter)?,
                serum_source_token_account: next_account_info(account_info_iter)?,
                serum_dest_token_account: next_account_info(account_info_iter)?,
                serum_vault_signer: next_account_info(account_info_iter)?,
                user_source_token_account: loan_supply_account_info,
                user_dest_token_account: collateral_supply_account_info,
                user_authority: manager_authority_info,
                signer_seeds,
            };
            // check ctx
            if !swap_ctx.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let collateral_amount_before = swap_ctx.get_user_dest_token_balance()?;
            // do swap
            swap_ctx.swap_base_in(borrow_amount, min_collateral_amount)?;
            // after swap
            swap_ctx.get_user_dest_token_balance()?
                .checked_sub(collateral_amount_before)
                .ok_or(LendingError::MathOverflow)?
        }
        _ => unreachable!("unexpected dex type"),
    };
    
    // deposit
    // accure interest
    collateral_market_reserve.accrue_interest(clock.slot)?;
    collateral_market_reserve.last_update.update_slot(clock.slot, true);
    let mint_amount = collateral_market_reserve.deposit(collateral_amount)?;
    // pledge in obligation
    let _ = if let Ok(index) = user_obligation.find_collateral(collateral_market_reserve_info.key) {
        user_obligation.pledge::<true>(mint_amount, None, index, &collateral_market_reserve)?
    } else {
        user_obligation.new_pledge::<true>(mint_amount, None, *collateral_market_reserve_info.key, &collateral_market_reserve)?
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

// must after update obligation
#[inline(never)]
fn process_open_leverage_position_base_out<const DEX_TYPE: DexType>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    max_borrow_amount: u64,
    collateral_amount: u64,
) -> ProgramResult {
    let max_borrow_amount = handle_amount(max_borrow_amount, || {
        msg!("Open leverage position max borrow amount provided cannot be zero");
    })?;
    if collateral_amount == 0 {
        msg!("Open leverage position collateral amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter().peekable();
    // 1
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_mut_market_reserve!(collateral_market_reserve_info, collateral_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 5
    get_supply_account!(collateral_supply_account_info; account_info_iter, collateral_market_reserve);
    // 6
    get_mut_market_reserve!(loan_market_reserve_info, loan_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 7
    get_supply_account!(loan_supply_account_info; account_info_iter, loan_market_reserve);
    // 8
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, manager_info.key, clock);
    // 9?
    get_friend_obligation!(friend_obligation; account_info_iter, user_obligation, clock);
    // 9/10
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 10/11
    let token_program_info = next_account_info(account_info_iter)?;
    // 11/12
    let swap_program_info = next_account_info(account_info_iter)?;

    // user borrow from reserve first
    let max_borrow_amount = calculate_amount(max_borrow_amount, loan_market_reserve.liquidity_info.available);
    let borrow_amount = match DEX_TYPE {
        ORCA => {
            let swap_ctx = OrcaSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                pool_info: next_account_info(account_info_iter)?,
                pool_authority: next_account_info(account_info_iter)?,
                pool_lp_token_mint: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                pool_fee_account: next_account_info(account_info_iter)?,
                user_source_token_account: loan_supply_account_info,
                user_dest_token_account: collateral_supply_account_info,
                user_authority: manager_authority_info,
                signer_seeds,
            };
            // check ctx
            if !swap_ctx.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let loan_amount_before = swap_ctx.get_user_source_token_balance()?;
            // do swap
            swap_ctx.swap_base_out(max_borrow_amount, collateral_amount)?;
            // after swap
            loan_amount_before
                .checked_sub(swap_ctx.get_user_source_token_balance()?)
                .ok_or(LendingError::MathOverflow)?
        }
        ORCA_TWICE => unimplemented!("Orca twice router does not support base out"),
        RAYDIUM => {
            let swap_ctx = RaydiumSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                amm_info: next_account_info(account_info_iter)?,
                amm_authority: next_account_info(account_info_iter)?,
                amm_open_orders: next_account_info(account_info_iter)?,
                amm_target_orders: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                serum_program: next_account_info(account_info_iter)?,
                serum_market: next_account_info(account_info_iter)?,
                serum_bids: next_account_info(account_info_iter)?,
                serum_asks: next_account_info(account_info_iter)?,
                serum_event_queue: next_account_info(account_info_iter)?,
                serum_source_token_account: next_account_info(account_info_iter)?,
                serum_dest_token_account: next_account_info(account_info_iter)?,
                serum_vault_signer: next_account_info(account_info_iter)?,
                user_source_token_account: loan_supply_account_info,
                user_dest_token_account: collateral_supply_account_info,
                user_authority: manager_authority_info,
                signer_seeds,
            };
            // check ctx
            if !swap_ctx.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let loan_amount_before = swap_ctx.get_user_source_token_balance()?;
            // do swap
            swap_ctx.swap_base_out(max_borrow_amount, collateral_amount)?;
            // after swap
            loan_amount_before
                .checked_sub(swap_ctx.get_user_source_token_balance()?)
                .ok_or(LendingError::MathOverflow)?
        }
        _ => unreachable!("unexpected dex type"),
    };
    
    // accure interest
    collateral_market_reserve.accrue_interest(clock.slot)?;
    collateral_market_reserve.last_update.update_slot(clock.slot, true);
    // deposit
    let mint_amount = collateral_market_reserve.deposit(collateral_amount)?;
    // pledge in obligation
    let _ = if let Ok(index) = user_obligation.find_collateral(collateral_market_reserve_info.key) {
        user_obligation.pledge::<true>(mint_amount, None, index, &collateral_market_reserve)?
    } else {
        user_obligation.new_pledge::<true>(mint_amount, None, *collateral_market_reserve_info.key, &collateral_market_reserve)?
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

#[inline(never)]
fn process_easy_repay_base_in<const DEX_TYPE: DexType>(
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_mut_market_reserve!(collateral_market_reserve_info, collateral_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 5
    get_supply_account!(collateral_supply_account_info; account_info_iter, collateral_market_reserve);
    // 6
    get_mut_market_reserve!(loan_market_reserve_info, loan_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 7
    get_supply_account!(loan_supply_account_info; account_info_iter, loan_market_reserve);
    // 8
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, manager_info.key, clock);
    // 9?
    get_friend_obligation!(friend_obligation; account_info_iter, user_obligation, clock);
    // 9/10
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 10/11
    let token_program_info = next_account_info(account_info_iter)?;
    // 11/12
    let swap_program_info = next_account_info(account_info_iter)?;

    let collateral_index = user_obligation.find_collateral(collateral_market_reserve_info.key)?;
    // redeem without remove
    let sotoken_amount = user_obligation.redeem::<false, false>(sotoken_amount, collateral_index, &collateral_market_reserve, friend_obligation.clone())?;
    // accure interest
    collateral_market_reserve.accrue_interest(clock.slot)?;
    collateral_market_reserve.last_update.update_slot(clock.slot, true);
    let collateral_amount = collateral_market_reserve.withdraw(sotoken_amount)?;

    let actual_repay_amount = match DEX_TYPE {
        ORCA => {
            let swap_ctx = OrcaSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                pool_info: next_account_info(account_info_iter)?,
                pool_authority: next_account_info(account_info_iter)?,
                pool_lp_token_mint: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                pool_fee_account: next_account_info(account_info_iter)?,
                user_source_token_account: collateral_supply_account_info,
                user_dest_token_account: loan_supply_account_info,
                user_authority: manager_authority_info,
                signer_seeds,
            };
            // check ctx
            if !swap_ctx.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let loan_amount_before = swap_ctx.get_user_dest_token_balance()?;
            // do swap
            swap_ctx.swap_base_in(collateral_amount, min_repay_amount)?;
            // after swap
            swap_ctx.get_user_dest_token_balance()?
                .checked_sub(loan_amount_before)
                .ok_or(LendingError::MathOverflow)?
        }
        ORCA_TWICE => {
            let temp_token_account = next_account_info(account_info_iter)?;
            let swap_ctx_1 = OrcaSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                pool_info: next_account_info(account_info_iter)?,
                pool_authority: next_account_info(account_info_iter)?,
                pool_lp_token_mint: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                pool_fee_account: next_account_info(account_info_iter)?,
                user_source_token_account: collateral_supply_account_info,
                user_dest_token_account: temp_token_account,
                user_authority: manager_authority_info,
                signer_seeds,
            };
            let swap_ctx_2 = OrcaSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                pool_info: next_account_info(account_info_iter)?,
                pool_authority: next_account_info(account_info_iter)?,
                pool_lp_token_mint: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                pool_fee_account: next_account_info(account_info_iter)?,
                user_source_token_account: temp_token_account,
                user_dest_token_account: loan_supply_account_info,
                user_authority: user_authority_info,
                signer_seeds: &[],
            };
            // check ctx
            if !swap_ctx_1.is_supported() || !swap_ctx_2.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let loan_amount_before = swap_ctx_2.get_user_dest_token_balance()?;
            // do swap 1
            swap_ctx_1.swap_base_in(collateral_amount, 1)?;
            let temp_amount = swap_ctx_1.get_user_dest_token_balance()?;
            // do swap 2
            swap_ctx_2.swap_base_in(temp_amount, min_repay_amount)?;
            // after swap
            swap_ctx_2.get_user_dest_token_balance()?
                .checked_sub(loan_amount_before)
                .ok_or(LendingError::MathOverflow)?
        }
        RAYDIUM => {
            let swap_ctx = RaydiumSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                amm_info: next_account_info(account_info_iter)?,
                amm_authority: next_account_info(account_info_iter)?,
                amm_open_orders: next_account_info(account_info_iter)?,
                amm_target_orders: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                serum_program: next_account_info(account_info_iter)?,
                serum_market: next_account_info(account_info_iter)?,
                serum_bids: next_account_info(account_info_iter)?,
                serum_asks: next_account_info(account_info_iter)?,
                serum_event_queue: next_account_info(account_info_iter)?,
                serum_source_token_account: next_account_info(account_info_iter)?,
                serum_dest_token_account: next_account_info(account_info_iter)?,
                serum_vault_signer: next_account_info(account_info_iter)?,
                user_source_token_account: collateral_supply_account_info,
                user_dest_token_account: loan_supply_account_info,
                user_authority: manager_authority_info,
                signer_seeds,
            };
            // check ctx
            if !swap_ctx.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let loan_amount_before = swap_ctx.get_user_dest_token_balance()?;
            // do swap
            swap_ctx.swap_base_in(collateral_amount, min_repay_amount)?;
            // after swap
            swap_ctx.get_user_dest_token_balance()?
                .checked_sub(loan_amount_before)
                .ok_or(LendingError::MathOverflow)?
        }
        _ => unreachable!("unexpected dex type"),
    };

    let loan_index = user_obligation.find_loan(loan_market_reserve_info.key)?;
    let settle = user_obligation.repay::<true>(
        Some(actual_repay_amount),
        u64::MAX,
        loan_index,
        &loan_market_reserve,
    )?;
    // accrue interest
    loan_market_reserve.accrue_interest(clock.slot)?;
    loan_market_reserve.last_update.update_slot(clock.slot, true);
    // user repay in loan reserve
    loan_market_reserve.liquidity_info.repay(&settle)?;
    
    // validate health
    user_obligation.validate_health(friend_obligation)?;
    user_obligation.last_update.mark_stale();
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(loan_market_reserve, &mut loan_market_reserve_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(collateral_market_reserve, &mut collateral_market_reserve_info.try_borrow_mut_data()?)
}

#[inline(never)]
fn process_easy_repay_base_out<const DEX_TYPE: DexType>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    max_sotoken_amount: u64,
    repay_amount: u64,
) -> ProgramResult {
    let max_sotoken_amount = handle_amount(max_sotoken_amount, || {
        msg!("Easy repay max sotoken amount provided cannot be zero");
    })?;
    let repay_amount = handle_amount(repay_amount, || {
        msg!("Easy repay repaying amount provided cannot be zero");
    })?;

    let account_info_iter = &mut accounts.iter().peekable();
    // 1
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_mut_market_reserve!(collateral_market_reserve_info, collateral_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 5
    get_supply_account!(collateral_supply_account_info; account_info_iter, collateral_market_reserve);
    // 6
    get_mut_market_reserve!(loan_market_reserve_info, loan_market_reserve; account_info_iter, program_id, manager_info.key, clock);
    // 7
    get_supply_account!(loan_supply_account_info; account_info_iter, loan_market_reserve);
    // 8
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, manager_info.key, clock);
    // 9?
    get_friend_obligation!(friend_obligation; account_info_iter, user_obligation, clock);
    // 9/10
    get_user_obligation_owner!(user_authority_info; account_info_iter, user_obligation);
    // 10/11
    let token_program_info = next_account_info(account_info_iter)?;
    // 11/12
    let swap_program_info = next_account_info(account_info_iter)?;

    let collateral_index = user_obligation.find_collateral(collateral_market_reserve_info.key)?;
    // redeem
    let max_sotoken_amount = user_obligation.redeem::<false, false>(max_sotoken_amount, collateral_index, &collateral_market_reserve, friend_obligation.clone())?;
    // accure interest
    collateral_market_reserve.accrue_interest(clock.slot)?;
    collateral_market_reserve.last_update.update_slot(clock.slot, true);
    // withdraw
    let max_collateral_amount = collateral_market_reserve.withdraw(max_sotoken_amount)?;
    // repay in obligation
    let loan_index = user_obligation.find_loan(loan_market_reserve_info.key)?;
    let settle = user_obligation.repay::<true>(repay_amount, u64::MAX, loan_index, &loan_market_reserve)?;
    // accrue interest
    loan_market_reserve.accrue_interest(clock.slot)?;
    loan_market_reserve.last_update.update_slot(clock.slot, true);
    // user repay in loan reserve
    loan_market_reserve.liquidity_info.repay(&settle)?;
    
    let collateral_amount = match DEX_TYPE {
        ORCA => {
            let swap_ctx = OrcaSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                pool_info: next_account_info(account_info_iter)?,
                pool_authority: next_account_info(account_info_iter)?,
                pool_lp_token_mint: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                pool_fee_account: next_account_info(account_info_iter)?,
                user_source_token_account: collateral_supply_account_info,
                user_dest_token_account: loan_supply_account_info,
                user_authority: manager_authority_info,
                signer_seeds,
            };
            // check ctx
            if !swap_ctx.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let collateral_amount_before = swap_ctx.get_user_source_token_balance()?;
            // dp swap
            swap_ctx.swap_base_out(max_collateral_amount, settle.amount)?;
            // after swap
            collateral_amount_before
                .checked_sub(swap_ctx.get_user_source_token_balance()?)
                .ok_or(LendingError::MathOverflow)?
        }
        ORCA_TWICE => unimplemented!("Orca twice router does not support base out"),
        RAYDIUM => {
            let swap_ctx = RaydiumSwapContext {
                swap_program: swap_program_info,
                token_program: token_program_info,
                amm_info: next_account_info(account_info_iter)?,
                amm_authority: next_account_info(account_info_iter)?,
                amm_open_orders: next_account_info(account_info_iter)?,
                amm_target_orders: next_account_info(account_info_iter)?,
                pool_source_token_account: next_account_info(account_info_iter)?,
                pool_dest_token_account: next_account_info(account_info_iter)?,
                serum_program: next_account_info(account_info_iter)?,
                serum_market: next_account_info(account_info_iter)?,
                serum_bids: next_account_info(account_info_iter)?,
                serum_asks: next_account_info(account_info_iter)?,
                serum_event_queue: next_account_info(account_info_iter)?,
                serum_source_token_account: next_account_info(account_info_iter)?,
                serum_dest_token_account: next_account_info(account_info_iter)?,
                serum_vault_signer: next_account_info(account_info_iter)?,
                user_source_token_account: collateral_supply_account_info,
                user_dest_token_account: loan_supply_account_info,
                user_authority: manager_authority_info,
                signer_seeds,
            };
            // check ctx
            if !swap_ctx.is_supported() {
                return Err(LendingError::InvalidDexAccounts.into());
            }
            // before swap
            let collateral_amount_before = swap_ctx.get_user_source_token_balance()?;
            // dp swap
            swap_ctx.swap_base_out(max_collateral_amount, settle.amount)?;
            // after swap
            collateral_amount_before
                .checked_sub(swap_ctx.get_user_source_token_balance()?)
                .ok_or(LendingError::MathOverflow)?
        }
        _ => unreachable!("unexpected dex type"),
    };

    if max_collateral_amount > collateral_amount {
        // deposit back to collateral reserve
        let mint_amount = collateral_market_reserve.deposit(max_collateral_amount - collateral_amount)?;
        user_obligation.pledge::<true>(mint_amount, None, collateral_index, &collateral_market_reserve)?;
    } else {
        user_obligation.close_empty_collateral(collateral_index);
    }
    
    // validate health
    user_obligation.validate_health(friend_obligation)?;
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
    get_rent!(rent_info, rent; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id, manager_info.key);
    // 5
    create_unique_credit!(unique_credit_info; account_info_iter, program_id, rent);
    // 6
    get_manager_owner!(manager_owner_info; account_info_iter, manager);

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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_mut_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id, manager_info.key);
    // 5
    get_supply_account!(supply_token_account_info; account_info_iter, market_reserve);
    // 6
    get_unique_credit!(unique_credit_info, unique_credit; account_info_iter, program_id, market_reserve_info.key);
    // 7
    get_unique_credit_owner!(authority_info; account_info_iter, unique_credit);
    // 8
    let token_program_info = next_account_info(account_info_iter)?;

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
    process_token_approve(
        token_program_info,
        supply_token_account_info,
        authority_info,
        manager_authority_info,
        amount,
        signer_seeds,
    )
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
    get_clock!(clock_info, clock; account_info_iter);
    // 2
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 3
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 4
    get_mut_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id, manager_info.key);
    // 5
    get_supply_account!(supply_token_account_info; account_info_iter, market_reserve);
    // 6
    get_unique_credit!(unique_credit_info, unique_credit; account_info_iter, program_id, market_reserve_info.key);
    // 7
    let source_token_account_info = next_account_info(account_info_iter)?;
    let source_token_account = Account::unpack(&source_token_account_info.try_borrow_data()?)?;
    if &source_token_account.owner == manager_authority_info.key {
        msg!("Source token account owner should not be manager authority");
        return Err(LendingError::InvalidTokenAccountOwner.into()); 
    }
    let source_token_account = Account::unpack(&source_token_account_info.try_borrow_data()?)?;
    // 8
    let token_program_info = next_account_info(account_info_iter)?;

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
    process_token_transfer(
        token_program_info,
        source_token_account_info,
        supply_token_account_info,
        manager_authority_info,
        settle.amount,
        signer_seeds,
    )
}

// by manager
fn process_operate_user_obligation<P: Any + Param>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    param: P,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 2
    get_mut_user_obligation!(user_obligation_info, user_obligation; account_info_iter, program_id, manager_info.key);
    // 3
    get_manager_owner!(manager_owner_info; account_info_iter, manager);

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
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 2
    get_mut_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id, manager_info.key);
    // 3
    get_manager_owner!(manager_owner_info; account_info_iter, manager);

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
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 2
    get_manager_authority!(manager_authority_info, signer_seeds; account_info_iter, program_id, manager, manager_info);
    // 3
    get_mut_market_reserve!(market_reserve_info, market_reserve; account_info_iter, program_id, manager_info.key);
    // 4
    get_supply_account!(supply_token_account_info; account_info_iter, market_reserve);
    // 5
    get_manager_owner!(manager_owner_info; account_info_iter, manager);
    // 6
    let receiver_token_account_info = next_account_info(account_info_iter)?;
    // 7
    let token_program_info = next_account_info(account_info_iter)?;

    // reduce insurance
    market_reserve.liquidity_info.reduce_insurance(amount)?;
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    // transfer
    process_token_transfer(
        token_program_info,
        supply_token_account_info,
        receiver_token_account_info,
        manager_authority_info,
        amount,
        signer_seeds,
    )
}

fn process_change_manager_owner(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    get_mut_manager!(manager_info, manager; account_info_iter, program_id);
    // 2
    get_manager_owner!(manager_owner_info; account_info_iter, manager);
    // 3
    get_signer!(new_owner_info; account_info_iter);

    manager.owner = *new_owner_info.key;
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
    get_manager!(manager_info, manager; account_info_iter, program_id);
    // 2
    get_unique_credit!(unique_credit_info, unique_credit; account_info_iter, program_id, manager_info.key);
    // 3
    get_manager_owner!(manager_owner_info; account_info_iter, manager);

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