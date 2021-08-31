use std::{collections::HashMap, convert::TryInto, error::Error, str::FromStr, thread, time::Duration};

use deploy::*;
use crate::{
    get_market_and_price_map,
    create_manager,
    create_rate_oracle,
    create_market_reserve,
    create_user_obligation,
    do_deposit,
    do_withdraw,
    do_bind_friend,
    do_unbind_friend,
    do_pledge_collateral,
    do_redeem_collateral,
    do_redeem_collateral_with_friend,
    do_redeem_collateral_without_loan,
    do_replace_collateral,
    do_replace_collateral_with_friend,
    do_borrow_liquidity,
    do_borrow_liquidity_with_friend,
    do_repay_loan,
    do_liquidate,
    do_liquidate_with_friend,
    do_inject_no_borrow,
    do_liquidate_by_injection,
    types::{UserObligationInfo, get_pyth_price},
};
use solana_client::{
    blockhash_query::BlockhashQuery, 
    rpc_client::RpcClient, 
    rpc_request::TokenAccountsFilter,
};
use solana_sdk::{
    clock::Clock,
    commitment_config::CommitmentConfig, 
    program_pack::Pack,
    pubkey::Pubkey,
    signer::{Signer, keypair::Keypair},
    system_instruction::create_account,
    sysvar::{Sysvar, SysvarId},
    transaction::Transaction
};
use spl_token::{
    instruction::{initialize_mint, initialize_account, mint_to},
    state::{Mint, Account},
};
use soda_lending_contract::{
    math::{WAD, Rate},
    state::{Manager, MarketReserve, RateOracle, UserObligation, 
        CollateralConfig, LiquidityConfig, RateOracleConfig
    },
    pyth::{self, Product},
};

fn main() {
    let client = RpcClient::new_with_commitment(DEV_NET.into(), CommitmentConfig::default());

    // let collaterals_price_oracle_map = get_market_and_price_map(&client).unwrap();
    // let clock_data = client.get_account_data(&Clock::id()).unwrap();
    // match UserObligationInfo::from_raw_data(
    //     &clock_data,
    //     &client.get_account_data(&Pubkey::from_str(OBLIGATION).unwrap()).unwrap(),
    //     &collaterals_price_oracle_map,
    // ) {
    //     Ok(obligation) => {
    //         println!("collaterals borrow value: {}, collaterals liquidation value: {}, collaterals max value: {}, loans value: {}",
    //             obligation.collaterals_borrow_value,
    //             obligation.collaterals_liquidation_value,
    //             obligation.collaterals_max_value,
    //             obligation.loans_value,
    //         );
    //     }
    //     Err(e) => println!("{:?}", e),
    // }

    // create test token
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let mint_lamports = client.get_minimum_balance_for_rent_exemption(Mint::LEN).unwrap();
    // let acnt_lamports = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let mint = Keypair::new();
    // let account = Keypair::new();
    // let transaction = create_test_token(
    //     mint,
    //     authority,
    //     account,
    //     mint_lamports,
    //     acnt_lamports,
    //     6,
    //     1_000_000_000_000_000_000,
    //     block_hash,
    // ).unwrap();
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // init sotoken account
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let lamports = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let account = Keypair::new();
    // let transaction = do_init_token_account(
    //     authority,
    //     account,
    //     Pubkey::from_str(SOSOL_MINT).unwrap(),
    //     lamports,
    //     block_hash,
    // ).unwrap();
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // create manager
    // let lamports = client.get_minimum_balance_for_rent_exemption(Manager::LEN).unwrap();
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let manager = Keypair::new();
    // println!("manager key: {:?}", manager.pubkey());
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_manager(manager, authority, lamports, block_hash);
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // create oracle
    // let lamports = client.get_minimum_balance_for_rent_exemption(RateOracle::LEN).unwrap();
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let rate_oracle = Keypair::new();
    // println!("rate oracle key: {:?}", rate_oracle.pubkey());
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_rate_oracle(rate_oracle, authority, RateOracleConfig {
    //     a: WAD,
    //     c: WAD / 10,
    //     l_u: 80,
    //     k_u: WAD as u128 * 3,
    // }, lamports, block_hash);
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // create market reserve
    // let account_lamports = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();
    // let reserve_lamports = client.get_minimum_balance_for_rent_exemption(MarketReserve::LEN).unwrap();
    // let mint_lamports = client.get_minimum_balance_for_rent_exemption(Mint::LEN).unwrap();
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_market_reserve(
    //     authority,
    //     Pubkey::from_str(MANAGER).unwrap(),
    //     Pubkey::from_str(SOL_PRODUCT).unwrap(),
    //     Pubkey::from_str(SOL_PRICE).unwrap(),
    //     Pubkey::from_str(RATE_ORACLE).unwrap(),
    //     Pubkey::from_str(SOL_MINT).unwrap(),
    //     CollateralConfig {
    //         borrow_value_ratio: 80, 
    //         liquidation_value_ratio: 90,
    //         close_factor: 60,
    //     },
    //     LiquidityConfig {
    //         borrow_fee_rate: 10_000_000_000_000_000, // 1%
    //         liquidation_fee_rate: 200_000_000_000_000_000, // 20%
    //         flash_loan_fee_rate: 50_000_000_000_000_000, // 5%
    //         max_deposit: u64::MAX,
    //         max_acc_deposit: u64::MAX,
    //     },
    //     reserve_lamports,
    //     account_lamports, 
    //     mint_lamports,
    //     block_hash,
    // ).unwrap();
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // deposit
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let transaction = do_deposit(
    //     authority,
    //     Pubkey::from_str(MANAGER).unwrap(),
    //     Pubkey::from_str(USDT_RESERVE).unwrap(),
    //     Pubkey::from_str(SOUSDT_MINT).unwrap(),
    //     Pubkey::from_str(USDT_MANAGER_TOKEN_ACCOUNT).unwrap(),
    //     Pubkey::from_str(RATE_ORACLE).unwrap(),
    //     Pubkey::from_str(USDT_LONE_TOKEN_ACCOUNT).unwrap(),
    //     Pubkey::from_str(SOUSDT_LONE_TOKEN_ACCOUNT).unwrap(),
    //     1_000_000_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // withdraw
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let transaction = do_withdraw(
    //     authority,
    //     Pubkey::from_str("F93DUk6QDpLBRd6pVQNtXvgrU4mBNMv5d1JaYkHvhcr5").unwrap(),
    //     Pubkey::from_str("73wnWaSncUBgmEp5RFS1ZBLG7Y3SFv45Etnv92UN2WeQ").unwrap(),
    //     Pubkey::from_str("4YPUDRM9LbxemxoAoZDECoigeRuWa1csxAdGHWoztDT9").unwrap(),
    //     Pubkey::from_str("3D3KLLYbnSY9ZxxXRRpLpFwzWTaCahj5YNYFso7FRExu").unwrap(),
    //     Pubkey::from_str("7nHzMWXrse8Mcp3Qc5KSJwG5J16wA75DMNEz7jV6hFpf").unwrap(),
    //     Pubkey::from_str("GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN").unwrap(),
    //     Pubkey::from_str("4VnYHEeDi4UpHnKZFjLcZJKXTLzevotkciJY7Vb1JuZz").unwrap(),
    //     800_000_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // create user obligation
    // let lamports = client.get_minimum_balance_for_rent_exemption(UserObligation::LEN).unwrap();
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_user_obligation(
    //     authority,
    //     Pubkey::from_str(MANAGER).unwrap(),
    //     lamports, 
    //     block_hash
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // pledge collateral
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_pledge_collateral(
    //     authority,
    //     Pubkey::from_str(USDT_RESERVE).unwrap(),
    //     Pubkey::from_str(SOUSDT_MINT).unwrap(),
    //     Pubkey::from_str(OBLIGATION).unwrap(),
    //     Pubkey::from_str(SOUSDT_LONE_TOKEN_ACCOUNT).unwrap(),
    //     5_000_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // borrow liquidity
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let updating_keys = vec![
    //     (Pubkey::from_str(BNB_RESERVE).unwrap(), Pubkey::from_str(BNB_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(BTC_RESERVE).unwrap(), Pubkey::from_str(BTC_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(SOL_RESERVE).unwrap(), Pubkey::from_str(SOL_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(SRM_RESERVE).unwrap(), Pubkey::from_str(SRM_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(DOGE_RESERVE).unwrap(), Pubkey::from_str(DOGE_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(LUNA_RESERVE).unwrap(), Pubkey::from_str(LUNA_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(USDC_RESERVE).unwrap(), Pubkey::from_str(USDC_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(USDT_RESERVE).unwrap(), Pubkey::from_str(USDT_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    // ];
    // let transaction = do_borrow_liquidity(
    //     authority,
    //     updating_keys,
    //     7,
    //     Pubkey::from_str(MANAGER).unwrap(),
    //     Pubkey::from_str(USDT_MANAGER_TOKEN_ACCOUNT).unwrap(),
    //     Pubkey::from_str(OBLIGATION).unwrap(),
    //     Pubkey::from_str(USDT_LONE_TOKEN_ACCOUNT).unwrap(),
    //     130_000_000_000,
    //     block_hash,
    // ).unwrap();
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // repay loan
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_repay_loan(
    //     authority,
    //     Pubkey::from_str(USDT_RESERVE).unwrap(),
    //     Pubkey::from_str(USDT_MANAGER_TOKEN_ACCOUNT).unwrap(),
    //     Pubkey::from_str(RATE_ORACLE).unwrap(),
    //     Pubkey::from_str(OBLIGATION).unwrap(),
    //     Pubkey::from_str(USDT_LONE_TOKEN_ACCOUNT).unwrap(),
    //     100_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // redeem collateral
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let updating_keys = vec![
    //     (Pubkey::from_str(BNB_RESERVE).unwrap(), Pubkey::from_str(BNB_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(BTC_RESERVE).unwrap(), Pubkey::from_str(BTC_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(SOL_RESERVE).unwrap(), Pubkey::from_str(SOL_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(SRM_RESERVE).unwrap(), Pubkey::from_str(SRM_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(DOGE_RESERVE).unwrap(), Pubkey::from_str(DOGE_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(LUNA_RESERVE).unwrap(), Pubkey::from_str(LUNA_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(USDC_RESERVE).unwrap(), Pubkey::from_str(USDC_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    //     (Pubkey::from_str(USDT_RESERVE).unwrap(), Pubkey::from_str(USDT_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    // ];
    // let transaction = do_redeem_collateral(
    //     authority,
    //     updating_keys,
    //     7,
    //     Pubkey::from_str(MANAGER).unwrap(),
    //     Pubkey::from_str(SOUSDT_MINT).unwrap(),
    //     Pubkey::from_str(OBLIGATION).unwrap(),
    //     Pubkey::from_str(SOUSDT_LONE_TOKEN_ACCOUNT).unwrap(),
    //     u64::MAX,
    //     block_hash,
    // ).unwrap();
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // redeem collateral without loan
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_redeem_collateral_without_loan(
    //     authority,
    //     Pubkey::from_str("5nBpNCqkH8aKpUkJjruykZsuSjmLKSzCYEnAb2p8TB13").unwrap(),
    //     Pubkey::from_str("Ev7ugN8CcahvjRXeByFWejhCLhRG9gYZ8s4QReKHRxNP").unwrap(),
    //     Pubkey::from_str("6MRdknnThzPSz1vkfMAYWnepnAF5wGitRTNrJ6rrQe1s").unwrap(),
    //     Pubkey::from_str("3vtj3VomHHAoqHKtJQL1ymEP6GQmzXHb9TD1LRkBoxFq").unwrap(),
    //     Pubkey::from_str("GZ57zaxfgq1eWvHvGtw1ASsydqGRWLCoqM2TmvYuw1Pw").unwrap(),
    //     Pubkey::from_str("EnpPrZtpsKb2CK6Jyue6tYi4vPmztLXPDKdco3WnRYuS").unwrap(),
    //     100_000_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // liquidate
    let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    let (block_hash, _) = client.get_recent_blockhash().unwrap();
    let updating_keys = vec![
        (Pubkey::from_str(BNB_RESERVE).unwrap(), Pubkey::from_str(BNB_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
        (Pubkey::from_str(BTC_RESERVE).unwrap(), Pubkey::from_str(BTC_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
        (Pubkey::from_str(SOL_RESERVE).unwrap(), Pubkey::from_str(SOL_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
        (Pubkey::from_str(SRM_RESERVE).unwrap(), Pubkey::from_str(SRM_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
        (Pubkey::from_str(DOGE_RESERVE).unwrap(), Pubkey::from_str(DOGE_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
        (Pubkey::from_str(LUNA_RESERVE).unwrap(), Pubkey::from_str(LUNA_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
        (Pubkey::from_str(USDC_RESERVE).unwrap(), Pubkey::from_str(USDC_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
        (Pubkey::from_str(USDT_RESERVE).unwrap(), Pubkey::from_str(USDT_PRICE).unwrap(), Pubkey::from_str(RATE_ORACLE).unwrap()),
    ];
    let transaction = do_liquidate_by_injection(
        authority,
        updating_keys,
        0,
        7,
        Pubkey::from_str(MANAGER).unwrap(),
        Pubkey::from_str(USDT_MANAGER_TOKEN_ACCOUNT).unwrap(),
        Pubkey::from_str(SOBNB_MINT).unwrap(),
        Pubkey::from_str(OBLIGATION).unwrap(),
        Pubkey::from_str(USDT_LONE_TOKEN_ACCOUNT).unwrap(),
        Pubkey::from_str(SOBNB_LONE_TOKEN_ACCOUNT).unwrap(),
        u64::MAX,
        block_hash,
    ).unwrap();
    match client.send_and_confirm_transaction(&transaction) {
        Ok(sig) => println!("sig is {:?}", sig),
        Err(err) => println!("error: {:?}", err),
    }
}

