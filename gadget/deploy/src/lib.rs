#![allow(missing_docs)]
pub mod types;
pub mod error;

use std::str::FromStr;

use solana_sdk::{
    hash::Hash,
    program_error::ProgramError, 
    program_pack::Pack, 
    pubkey::Pubkey, 
    signer::{Signer, keypair::Keypair}, 
    system_instruction::create_account, 
    transaction::Transaction
};
use spl_token::{
    instruction::{initialize_mint, initialize_account, mint_to},
    state::{Mint, Account},
};
use soda_lending_contract::{
    instruction::{
        bind_friend, borrow_liquidity, deposit_collateral, exchange,
        init_manager, init_market_reserve, init_rate_oracle, init_user_obligation,
        liquidate, pause_rate_oracle, redeem_collateral, redeem_collateral_without_loan,
        repay_loan, replace_collateral, unbind_friend, update_market_reserves,
        update_user_obligation, withdraw_fee, inject_case,
    },
    math::WAD, pyth::{self, Product},
    state::{CollateralConfig, LiquidityConfig, Manager,
        MarketReserve, RateOracle, RateOracleConfig, UserObligation
    }
};

const PYTH_ID: &str = "gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s";
const QUOTE_CURRENCY: &[u8; 32] = &[85, 83, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

#[allow(clippy::too_many_arguments)]
pub fn create_fake_token(
    mint: &Keypair,
    authority: &Keypair,
    account: &Keypair,
    mint_lamports: u64,
    acnt_lamports: u64,
    decimals: u8,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let program_id = spl_token::id();
    let mint_pubkey = &mint.pubkey();
    let account_pubkey = &account.pubkey();
    let authority_pubkey = &authority.pubkey();

    Ok(Transaction::new_signed_with_payer(&[
            create_account(
                authority_pubkey,
                mint_pubkey,
                mint_lamports,
                Mint::LEN as u64,
                &program_id,
            ),
            create_account(
                authority_pubkey,
                account_pubkey,
                acnt_lamports,
                Account::LEN as u64,
                &program_id,
            ),
            initialize_mint(
                &program_id,
                mint_pubkey,
                authority_pubkey,
                None,
                decimals,
            )?,
            initialize_account(
                &program_id,
                account_pubkey,
                mint_pubkey,
                authority_pubkey,
            )?,
            mint_to(
                &program_id,
                mint_pubkey,
                account_pubkey,
                authority_pubkey,
                &[authority_pubkey],
                amount
            )?,
        ],
        Some(authority_pubkey),
        &[mint, account, authority],
        recent_blockhash,
    ))
}

pub fn do_init_token_account(
    authority: Keypair,
    account: Keypair,
    mint_key: Pubkey,
    lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let authority_key = &authority.pubkey();
    let account_key = &account.pubkey();
    let program_id = spl_token::id();

    Ok(Transaction::new_signed_with_payer(&[
            create_account(
                authority_key,
                &account_key,
                lamports,
                Account::LEN as u64,
                &program_id,
            ),
            initialize_account(
                &program_id,
                &account_key,
                &mint_key,
                authority_key,
            )?,
        ],
        Some(authority_key),
        &[&account, &authority],
        recent_blockhash,
    ))
}

pub fn create_manager(
    manager: Keypair,
    authority: Keypair,
    lamports: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let manager_key = &manager.pubkey();
    let authority_key = &authority.pubkey();
    let pyth_id = Pubkey::from_str(PYTH_ID).unwrap();

    Transaction::new_signed_with_payer(&[
        create_account(
            authority_key,
            manager_key,
            lamports,
            Manager::LEN as u64,
            &soda_lending_contract::id(),
        ),
        init_manager(
            *manager_key,
            *authority_key,
            pyth_id,
            *QUOTE_CURRENCY,
        )
    ],
    Some(authority_key),
        &[&manager, &authority],
        recent_blockhash,
    )
}

pub fn create_rate_oracle(
    rate_oracle: Keypair,
    authority: Keypair,
    config: RateOracleConfig,
    lamports: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_key = &authority.pubkey();
    let rate_oracle_key = &rate_oracle.pubkey();

    Transaction::new_signed_with_payer(&[
        create_account(
            authority_key,
            rate_oracle_key,
            lamports,
            RateOracle::LEN as u64,
            &soda_lending_contract::id(),
        ),
        init_rate_oracle(
            *rate_oracle_key,
            *authority_key,
            config,
        )
    ],
    Some(authority_key),
        &[&rate_oracle, &authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn create_market_reserve(
    authority: Keypair,
    manager_key: Pubkey,
    pyth_product_key: Pubkey,
    pyth_price_key: Pubkey,
    rate_oracle_key: Pubkey,
    token_mint_key: Pubkey,
    collateral_config: CollateralConfig,
    liquidity_config: LiquidityConfig,
    enable_borrow: bool,
    reserve_lamports: u64,
    account_lamports: u64,
    mint_lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let market_reserve = Keypair::new();
    let manager_token_account = Keypair::new();
    let sotoken_mint = Keypair::new();
    let market_reserve_key = market_reserve.pubkey();
    let manager_token_account_key = manager_token_account.pubkey();
    let sotoken_mint_key = sotoken_mint.pubkey();

    println!("market reserve key: {:?}", market_reserve_key);
    println!("manager token account key: {:?}", manager_token_account_key);
    println!("sotoken mint key: {:?}", sotoken_mint_key);

    let token_program_id = &spl_token::id();
    let authority_key = &authority.pubkey();

    Ok(Transaction::new_signed_with_payer(&[
        create_account(
            authority_key,
            &market_reserve_key,
            reserve_lamports,
            MarketReserve::LEN as u64,
            &soda_lending_contract::id(),
        ),
        create_account(
            authority_key,
            &manager_token_account_key,
            account_lamports,
            Account::LEN as u64,
            token_program_id,
        ),
        create_account(
            authority_key,
            &sotoken_mint_key,
            mint_lamports,
            Mint::LEN as u64,
            token_program_id,
        ),
        init_market_reserve(
            manager_key,
            manager_token_account_key,
            market_reserve_key,
            pyth_product_key,
            pyth_price_key,
            rate_oracle_key,
            token_mint_key,
            sotoken_mint_key,
            *authority_key,
            collateral_config,
            liquidity_config,
            enable_borrow,
        )
    ],
    Some(authority_key),
        &[
            &market_reserve,
            &manager_token_account,
            &sotoken_mint,
            &authority,
        ],
        recent_blockhash,
    ))
}

pub fn do_exchange(
    authority: Keypair,
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    manager_token_account_key: Pubkey,
    rate_oracle_key: Pubkey,
    user_token_account_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    from_collateral: bool,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_key = &authority.pubkey();

    Transaction::new_signed_with_payer(&[
        exchange(
            manager_key,
            market_reserve_key,
            sotoken_mint_key,
            manager_token_account_key,
            rate_oracle_key,
            *authority_key,
            user_token_account_key,
            user_sotoken_account_key,
            from_collateral,
            amount,
        ),
    ],
    Some(authority_key),
        &[&authority],
        recent_blockhash,
    )
}

pub fn create_user_obligation(
    authority: Keypair,
    manager_key: Pubkey,
    lamports: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let obligation = Keypair::new();
    let obligation_key = &obligation.pubkey();
    let authority_pubkey = &authority.pubkey();

    println!("obligation key: {:?}", obligation_key);

    Transaction::new_signed_with_payer(&[
        create_account(
            authority_pubkey,
            obligation_key,
            lamports,
            UserObligation::LEN as u64,
            &soda_lending_contract::id(),
        ),
        init_user_obligation(
            manager_key,
            *obligation_key,
            *authority_pubkey,
        ),
    ],
    Some(authority_pubkey),
        &[&obligation, &authority],
        recent_blockhash,
    )
}

pub fn do_bind_friend(
    user_authority: Keypair,
    friend_authority: Keypair,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();
    let friend_authority_key = &friend_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        bind_friend(
            user_obligation_key,
            friend_obligation_key,
            *user_authority_key,
            *friend_authority_key,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority, &friend_authority],
        recent_blockhash,
    )
}

pub fn do_unbind_friend(
    user_authority: Keypair,
    friend_authority: Keypair,
    user_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    friend_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();
    let friend_authority_key = &friend_authority.pubkey();

    let user_market_reserves = user_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let friend_market_reserves = friend_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    Transaction::new_signed_with_payer(&[
        update_market_reserves(user_updating_keys),
        update_user_obligation(user_obligation_key, user_market_reserves),
        update_market_reserves(friend_updating_keys),
        update_user_obligation(friend_obligation_key, friend_market_reserves),
        unbind_friend(
            user_obligation_key,
            friend_obligation_key,
            *user_authority_key,
            *friend_authority_key,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority, &friend_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_deposit_collateral(
    user_authority: Keypair,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligatiton_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        deposit_collateral(
            market_reserve_key,
            sotoken_mint_key,
            user_obligatiton_key,
            *user_authority_key,
            user_sotoken_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_redeem_collateral(
    user_authority: Keypair,
    updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    redeem_index: usize,
    manager_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let market_reserves = updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let market_reserve_key = market_reserves
        .get(redeem_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(updating_keys),
        update_user_obligation(user_obligation_key, market_reserves),
        redeem_collateral(
            manager_key,
            market_reserve_key,
            sotoken_mint_key,
            user_obligation_key,
            None,
            *user_authority_key,
            user_sotoken_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_redeem_collateral_with_friend(
    user_authority: Keypair,
    user_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    friend_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    redeem_index: usize,
    manager_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let user_market_reserves = user_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let friend_market_reserves = friend_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let market_reserve_key = user_market_reserves
        .get(redeem_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(user_updating_keys),
        update_user_obligation(user_obligation_key, user_market_reserves),
        update_market_reserves(friend_updating_keys),
        update_user_obligation(friend_obligation_key, friend_market_reserves),
        redeem_collateral(
            manager_key,
            market_reserve_key,
            sotoken_mint_key,
            user_obligation_key,
            Some(friend_obligation_key),
            *user_authority_key,
            user_sotoken_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_redeem_collateral_without_loan(
    user_authority: Keypair,
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    user_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        redeem_collateral_without_loan(
            manager_key,
            market_reserve_key,
            sotoken_mint_key,
            user_obligation_key,
            friend_obligation_key,
            *user_authority_key,
            user_sotoken_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_replace_collateral(
    user_authority: Keypair,
    updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    replace_out_index: usize,
    replace_in_index: usize,
    manager_key: Pubkey,
    out_sotoken_mint_key: Pubkey,
    in_sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    user_out_sotoken_account_key: Pubkey,
    user_in_sotoken_account_key: Pubkey,
    out_amount: u64,
    in_amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let market_reserves = updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let out_market_reserve_key = market_reserves
        .get(replace_out_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let in_market_reserve_key = market_reserves
        .get(replace_in_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(updating_keys),
        update_user_obligation(user_obligation_key, market_reserves),
        replace_collateral(
            manager_key,
            out_market_reserve_key,
            out_sotoken_mint_key,
            in_market_reserve_key,
            in_sotoken_mint_key,
            user_obligation_key,
            None,
            *user_authority_key,
            user_out_sotoken_account_key,
            user_in_sotoken_account_key,
            out_amount,
            in_amount
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_replace_collateral_with_friend(
    user_authority: Keypair,
    user_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    friend_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    replace_out_index: usize,
    replace_in_index: usize,
    manager_key: Pubkey,
    out_sotoken_mint_key: Pubkey,
    in_sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    user_out_sotoken_account_key: Pubkey,
    user_in_sotoken_account_key: Pubkey,
    out_amount: u64,
    in_amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let user_market_reserves = user_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let friend_market_reserves = friend_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let out_market_reserve_key = user_market_reserves
        .get(replace_out_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let in_market_reserve_key = user_market_reserves
        .get(replace_in_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(user_updating_keys),
        update_user_obligation(user_obligation_key, user_market_reserves),
        update_market_reserves(friend_updating_keys),
        update_user_obligation(friend_obligation_key, friend_market_reserves),
        replace_collateral(
            manager_key,
            out_market_reserve_key,
            out_sotoken_mint_key,
            in_market_reserve_key,
            in_sotoken_mint_key,
            user_obligation_key,
            Some(friend_obligation_key),
            *user_authority_key,
            user_out_sotoken_account_key,
            user_in_sotoken_account_key,
            out_amount,
            in_amount
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_borrow_liquidity(
    user_authority: Keypair,
    updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    borrow_index: usize,
    manager_key: Pubkey,
    manager_token_account_key: Pubkey,
    user_obligation_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let market_reserves = updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let market_reserve_key = market_reserves
        .get(borrow_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(updating_keys),
        update_user_obligation(user_obligation_key, market_reserves),
        borrow_liquidity(
            manager_key,
            market_reserve_key,
            manager_token_account_key,
            user_obligation_key,
            None,
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_borrow_liquidity_with_friend(
    user_authority: Keypair,
    user_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    friend_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    borrow_index: usize,
    manager_key: Pubkey,
    manager_token_account_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let user_market_reserves = user_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let friend_market_reserves = friend_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let market_reserve_key = user_market_reserves
        .get(borrow_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(user_updating_keys),
        update_user_obligation(user_obligation_key, user_market_reserves),
        update_market_reserves(friend_updating_keys),
        update_user_obligation(friend_obligation_key, friend_market_reserves),
        borrow_liquidity(
            manager_key,
            market_reserve_key,
            manager_token_account_key,
            user_obligation_key,
            Some(friend_obligation_key),
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_repay_loan(
    user_authority: Keypair,
    market_reserve_key: Pubkey,
    manager_token_account_key: Pubkey,
    rate_oracle_key: Pubkey,
    user_obligatiton_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        repay_loan(
            market_reserve_key,
            manager_token_account_key,
            rate_oracle_key,
            user_obligatiton_key,
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_liquidate(
    liquidator_authority: Keypair,
    updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    collateral_index: usize,
    loan_index: usize,
    manager_key: Pubkey,
    manager_token_account_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    liquidator_token_account_key: Pubkey,
    liquidator_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let liquidator_authority_key = &liquidator_authority.pubkey();    
    let market_reserves = updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let collateral_market_reserve_key = market_reserves
        .get(collateral_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let loan_market_reserve_key = market_reserves
        .get(loan_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(updating_keys),
        update_user_obligation(user_obligation_key, market_reserves),
        liquidate(
            manager_key,
            collateral_market_reserve_key,
            sotoken_mint_key,
            loan_market_reserve_key,
            manager_token_account_key,
            user_obligation_key,
            None,
            *liquidator_authority_key,
            liquidator_token_account_key,
            liquidator_sotoken_account_key,
            amount,
        ),
    ],
    Some(liquidator_authority_key),
        &[&liquidator_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_liquidate_with_friend(
    liquidator_authority: Keypair,
    user_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    friend_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    collateral_index: usize,
    loan_index: usize,
    manager_key: Pubkey,
    manager_token_account_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    liquidator_token_account_key: Pubkey,
    liquidator_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let liquidator_authority_key = &liquidator_authority.pubkey();    
    let user_market_reserves = user_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let friend_market_reserves = friend_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let collateral_market_reserve_key = user_market_reserves
        .get(collateral_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let loan_market_reserve_key = user_market_reserves
        .get(loan_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(user_updating_keys),
        update_user_obligation(user_obligation_key, user_market_reserves),
        update_market_reserves(friend_updating_keys),
        update_user_obligation(friend_obligation_key, friend_market_reserves),
        liquidate(
            manager_key,
            collateral_market_reserve_key,
            sotoken_mint_key,
            loan_market_reserve_key,
            manager_token_account_key,
            user_obligation_key,
            Some(friend_obligation_key),
            *liquidator_authority_key,
            liquidator_token_account_key,
            liquidator_sotoken_account_key,
            amount,
        ),
    ],
    Some(liquidator_authority_key),
        &[&liquidator_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

pub fn do_inject_case(
    authority: Keypair,
    user_obligation_key: Pubkey,
    is_liquidation: bool,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_key = &authority.pubkey();

    Transaction::new_signed_with_payer(&[
        inject_case(
            user_obligation_key,
            is_liquidation
        ),
    ],
    Some(authority_key),
        &[&authority],
        recent_blockhash,
    )
}