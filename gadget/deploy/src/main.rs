use std::{collections::HashMap, convert::TryInto, error::Error, str::FromStr, thread, time::Duration};

use deploy::*;
use crate::{
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
    do_inject_liquidation,
    types::{UserObligationInfo, get_pyth_price},
};
use solana_client::{
    blockhash_query::BlockhashQuery, 
    rpc_client::RpcClient, 
    rpc_request::TokenAccountsFilter,
};
use solana_sdk::{clock::Clock, commitment_config::CommitmentConfig, hash::Hash, instruction::Instruction, program_error::ProgramError, program_pack::Pack, pubkey::Pubkey, signer::{Signer, keypair::Keypair}, system_instruction::create_account, sysvar::{Sysvar, SysvarId}, transaction::Transaction};
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
// mutual
const DEV_NET: &str = "http://65.21.40.30";
const GLOBAL_OWNER: &str = "vG2VqMokQyY82xKda116qAmvMQm4ymoKEV92UtxNVmu4tKDt4X33ELY4rdCfiR1NxJnbek39m5X9rLJnxASNbmQ";
const QUOTE_CURRENCY: &[u8; 32] = &[85, 83, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const MANAGER: &str = "F93DUk6QDpLBRd6pVQNtXvgrU4mBNMv5d1JaYkHvhcr5";
const OBLIGATION: &str = "5DJ3bZDmzv1NruAiEeJ3BgWcw568eKBqdHPkP68vGgXH";
const BORROW_RATE: &str = "7nHzMWXrse8Mcp3Qc5KSJwG5J16wA75DMNEz7jV6hFpf";

// BNB
const BNB_MINT: &str = "6mhUyoQR5CcHN4RJ5PSfcvTjRuWF742ypZeMwptPgFnK";
const SOBNB_MINT: &str = "24Y4Wd6tYBiCgVU4naLAVA8wFSDb9v1hVwdK93x5g7Vw";
const BNB_PRICE: &str = "GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b";
const BNB_MANAGER_TOKEN_ACCOUNT: &str = "DgHRo44dn63LJhTC5hheGhM9dERDQrhAn7V7smz5wWrC";
const BNB_RESERVE: &str = "3raP2UGyEqgQRpza4m6zwqpy6wumioq3fzaqdy1YYcvm";
const BNB_LONE_TOKEN_ACCOUNT: &str = "EnpPrZtpsKb2CK6Jyue6tYi4vPmztLXPDKdco3WnRYuS";
const SOBNB_LONE_TOKEN_ACCOUNT: &str = "3ikRkx3WZkY2puEo89tk2QpN6FSbcrENN4y5bQk2GmwL";

// BTC
const BTC_MINT: &str = "9bRWBCW4BHHoLXFLFcLU3FQCDXXLNds1SJBmpeKYFeBZ";
const SOBTC_MINT: &str = "4YPUDRM9LbxemxoAoZDECoigeRuWa1csxAdGHWoztDT9";
const BTC_PRICE: &str = "HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J";
const BTC_MANAGER_TOKEN_ACCOUNT: &str = "3D3KLLYbnSY9ZxxXRRpLpFwzWTaCahj5YNYFso7FRExu";
const BTC_RESERVE: &str = "73wnWaSncUBgmEp5RFS1ZBLG7Y3SFv45Etnv92UN2WeQ";
const BTC_LONE_TOKEN_ACCOUNT: &str = "GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN";
const SOBTC_LONE_TOKEN_ACCOUNT: &str = "4VnYHEeDi4UpHnKZFjLcZJKXTLzevotkciJY7Vb1JuZz";

fn main() {
    let client = RpcClient::new_with_commitment(DEV_NET.into(), CommitmentConfig::default());

    let mut collaterals_price_oracle_map = HashMap::<Pubkey, (Vec<u8>, Vec<u8>, Vec<u8>)>::new();
    collaterals_price_oracle_map.insert(
        // BNB
        Pubkey::from_str(BNB_RESERVE).unwrap(),
        (
            client.get_account_data(&Pubkey::from_str(BNB_RESERVE).unwrap()).unwrap(),
            client.get_account_data(&Pubkey::from_str(BNB_PRICE).unwrap()).unwrap(),
            client.get_account_data(&Pubkey::from_str(BORROW_RATE).unwrap()).unwrap(),
        ),
    );
    let clock_data = client.get_account_data(&Clock::id()).unwrap();
    match UserObligationInfo::from_raw_data(
        &clock_data,
        &client.get_account_data(&Pubkey::from_str(OBLIGATION).unwrap()).unwrap(),
        &collaterals_price_oracle_map,
    ) {
        Ok(obligation) => {
            println!("collaterals borrow value: {}, collaterals liquidation value: {}, collaterals max value: {}, loans value: {}",
                obligation.collaterals_borrow_value,
                obligation.collaterals_liquidation_value,
                obligation.collaterals_max_value,
                obligation.loans_value,
            );
        }
        Err(e) => println!("{:?}", e),
    }

    // //
    // let rate_data = client.get_account_data(&Pubkey::from_str("7nHzMWXrse8Mcp3Qc5KSJwG5J16wA75DMNEz7jV6hFpf").unwrap()).unwrap();
    // let bnb_reserve_key = Pubkey::from_str("3raP2UGyEqgQRpza4m6zwqpy6wumioq3fzaqdy1YYcvm").unwrap();
    // let bnb_price_data = client.get_account_data(&Pubkey::from_str("GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b").unwrap()).unwrap();
    // let btc_reserve = Pubkey::from_str("3raP2UGyEqgQRpza4m6zwqpy6wumioq3fzaqdy1YYcvm").unwrap();

    // collaterals_price_oracle_map.insert(
    //     // BNB
    //     Pubkey::from_str("3raP2UGyEqgQRpza4m6zwqpy6wumioq3fzaqdy1YYcvm").unwrap(),
    //     (
    //         client.get_account_data(Pubkey::from_str("3raP2UGyEqgQRpza4m6zwqpy6wumioq3fzaqdy1YYcvm").unwrap()),
    //     )

    // let clock_data = client.get_account_data(&Clock::id()).unwrap();

    // match UserObligationInfo::from_raw_data(
    //     &clock_data,
    //     &market_reserve_data,
    //     &obligation_data,
    //     &rate_oracle_data,
    //     &liquidity_price_oracle_data,
    //     &collaterals_price_oracle_map
    // ) {
    //     Ok(obligation) => {
    //         println!("borrow equivalent value: {}, liquidation equivalent value: {}, max value: {}, loan value: {},
    //             dept amount: {}, borrowed amount: {}",
    //             obligation.borrow_equivalent_value,
    //             obligation.liquidation_equivalent_value,
    //             obligation.max_value,
    //             obligation.loan_value,
    //             obligation.dept_amount,
    //             obligation.borrowed_amount,
    //         );
    //     }
    //     Err(e) => println!("{:?}", e),
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

    // create market reserve (BTC)
    // let account_lamports = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();
    // let reserve_lamports = client.get_minimum_balance_for_rent_exemption(MarketReserve::LEN).unwrap();
    // let mint_lamports = client.get_minimum_balance_for_rent_exemption(Mint::LEN).unwrap();
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_market_reserve(
    //     authority,
    //     Pubkey::from_str("F93DUk6QDpLBRd6pVQNtXvgrU4mBNMv5d1JaYkHvhcr5").unwrap(),
    //     Pubkey::from_str("3m1y5h2uv7EQL3KaJZehvAJa4yDNvgc5yAdL9KPMKwvk").unwrap(),
    //     Pubkey::from_str("HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J").unwrap(),
    //     Pubkey::from_str("7nHzMWXrse8Mcp3Qc5KSJwG5J16wA75DMNEz7jV6hFpf").unwrap(),
    //     Pubkey::from_str("9bRWBCW4BHHoLXFLFcLU3FQCDXXLNds1SJBmpeKYFeBZ").unwrap(),
    //     CollateralConfig {
    //         borrow_value_ratio: 70, 
    //         liquidation_value_ratio: 85,
    //         close_factor: 60,
    //     },
    //     LiquidityConfig {
    //         borrow_fee_rate: 50_000_000_000_000_000, // 5%
    //         liquidation_fee_rate: 50_000_000_000_000_000, // 5%
    //         flash_loan_fee_rate: 100_000_000_000_000_000, // 10%
    //     },
    //     true,
    //     reserve_lamports,
    //     account_lamports, 
    //     mint_lamports,
    //     block_hash,
    // ).unwrap();
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // create market reserve (BNB)
    // let account_lamports = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();
    // let reserve_lamports = client.get_minimum_balance_for_rent_exemption(MarketReserve::LEN).unwrap();
    // let mint_lamports = client.get_minimum_balance_for_rent_exemption(Mint::LEN).unwrap();
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_market_reserve(
    //     authority,
    //     Pubkey::from_str("F93DUk6QDpLBRd6pVQNtXvgrU4mBNMv5d1JaYkHvhcr5").unwrap(),
    //     Pubkey::from_str("2weC6fjXrfaCLQpqEzdgBHpz6yVNvmSN133m7LDuZaDb").unwrap(),
    //     Pubkey::from_str("GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b").unwrap(),
    //     Pubkey::from_str("7nHzMWXrse8Mcp3Qc5KSJwG5J16wA75DMNEz7jV6hFpf").unwrap(),
    //     Pubkey::from_str("6mhUyoQR5CcHN4RJ5PSfcvTjRuWF742ypZeMwptPgFnK").unwrap(),
    //     CollateralConfig {
    //         borrow_value_ratio: 60, 
    //         liquidation_value_ratio: 80,
    //         close_factor: 60,
    //     },
    //     LiquidityConfig {
    //         borrow_fee_rate: 50_000_000_000_000_000, // 5%
    //         liquidation_fee_rate: 50_000_000_000_000_000, // 5%
    //         flash_loan_fee_rate: 100_000_000_000_000_000, // 10%
    //     },
    //     false,
    //     reserve_lamports,
    //     account_lamports, 
    //     mint_lamports,
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
    // println!("account is {:?}", &account.pubkey());
    // let transaction = do_init_token_account(
    //     authority,
    //     account,
    //     Pubkey::from_str("4YPUDRM9LbxemxoAoZDECoigeRuWa1csxAdGHWoztDT9").unwrap(),
    //     lamports,
    //     block_hash,
    // ).unwrap();
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // exchange
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let transaction = do_exchange(
    //     authority,
    //     Pubkey::from_str("F93DUk6QDpLBRd6pVQNtXvgrU4mBNMv5d1JaYkHvhcr5").unwrap(),
    //     Pubkey::from_str("73wnWaSncUBgmEp5RFS1ZBLG7Y3SFv45Etnv92UN2WeQ").unwrap(),
    //     Pubkey::from_str("4YPUDRM9LbxemxoAoZDECoigeRuWa1csxAdGHWoztDT9").unwrap(),
    //     Pubkey::from_str("3D3KLLYbnSY9ZxxXRRpLpFwzWTaCahj5YNYFso7FRExu").unwrap(),
    //     Pubkey::from_str("7nHzMWXrse8Mcp3Qc5KSJwG5J16wA75DMNEz7jV6hFpf").unwrap(),
    //     Pubkey::from_str("GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN").unwrap(),
    //     Pubkey::from_str("4VnYHEeDi4UpHnKZFjLcZJKXTLzevotkciJY7Vb1JuZz").unwrap(),
    //     false,
    //     1_000_000_000_000,
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
    //     Pubkey::from_str("F93DUk6QDpLBRd6pVQNtXvgrU4mBNMv5d1JaYkHvhcr5").unwrap(),
    //     lamports, 
    //     block_hash
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // deposit collateral
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_deposit_collateral(
    //     authority,
    //     Pubkey::from_str("3raP2UGyEqgQRpza4m6zwqpy6wumioq3fzaqdy1YYcvm").unwrap(),
    //     Pubkey::from_str("24Y4Wd6tYBiCgVU4naLAVA8wFSDb9v1hVwdK93x5g7Vw").unwrap(),
    //     Pubkey::from_str("5DJ3bZDmzv1NruAiEeJ3BgWcw568eKBqdHPkP68vGgXH").unwrap(),
    //     Pubkey::from_str("3ikRkx3WZkY2puEo89tk2QpN6FSbcrENN4y5bQk2GmwL").unwrap(),
    //     800_000_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // deposit liquidity
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_deposit_liquidity(
    //     authority,
    //     Pubkey::from_str("Ev7ugN8CcahvjRXeByFWejhCLhRG9gYZ8s4QReKHRxNP").unwrap(),
    //     Pubkey::from_str("3sAzDiT2dBjrCPsADnRUPEUi8wquWxNynHDCnnU3M8z1").unwrap(),
    //     Pubkey::from_str("6weJxYMjio6qAoXvNafpzgwCF3fi1knQkgm6DHg1WN1J").unwrap(),
    //     Pubkey::from_str("Csvk8Wp3AxVaVQqDQgP6KLVMCLEvuypEKtPD6xhAvV8L").unwrap(),
    //     Pubkey::from_str("GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN").unwrap(),
    //     1_000_000_000_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // withdraw liquidity
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_withdraw_liquidity(
    //     authority,
    //     Pubkey::from_str("5nBpNCqkH8aKpUkJjruykZsuSjmLKSzCYEnAb2p8TB13").unwrap(),
    //     Pubkey::from_str("Ev7ugN8CcahvjRXeByFWejhCLhRG9gYZ8s4QReKHRxNP").unwrap(),
    //     Pubkey::from_str("3sAzDiT2dBjrCPsADnRUPEUi8wquWxNynHDCnnU3M8z1").unwrap(),
    //     Pubkey::from_str("6weJxYMjio6qAoXvNafpzgwCF3fi1knQkgm6DHg1WN1J").unwrap(),
    //     Pubkey::from_str("Csvk8Wp3AxVaVQqDQgP6KLVMCLEvuypEKtPD6xhAvV8L").unwrap(),
    //     Pubkey::from_str("GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN").unwrap(),
    //     1_000_000_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // borrow liquidity
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_borrow_liquidity(
    //     authority,
    //     Pubkey::from_str("5nBpNCqkH8aKpUkJjruykZsuSjmLKSzCYEnAb2p8TB13").unwrap(),
    //     Pubkey::from_str("Ev7ugN8CcahvjRXeByFWejhCLhRG9gYZ8s4QReKHRxNP").unwrap(),
    //     Pubkey::from_str("3sAzDiT2dBjrCPsADnRUPEUi8wquWxNynHDCnnU3M8z1").unwrap(),
    //     Pubkey::from_str("HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J").unwrap(),
    //     Pubkey::from_str("6weJxYMjio6qAoXvNafpzgwCF3fi1knQkgm6DHg1WN1J").unwrap(),
    //     vec![Pubkey::from_str("GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b").unwrap()],
    //     Pubkey::from_str("GZ57zaxfgq1eWvHvGtw1ASsydqGRWLCoqM2TmvYuw1Pw").unwrap(),
    //     Pubkey::from_str("GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN").unwrap(),
    //     1_000_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // repay loan
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_repay_loan(
    //     authority,
    //     Pubkey::from_str("Ev7ugN8CcahvjRXeByFWejhCLhRG9gYZ8s4QReKHRxNP").unwrap(),
    //     Pubkey::from_str("3sAzDiT2dBjrCPsADnRUPEUi8wquWxNynHDCnnU3M8z1").unwrap(),
    //     Pubkey::from_str("6weJxYMjio6qAoXvNafpzgwCF3fi1knQkgm6DHg1WN1J").unwrap(),
    //     Pubkey::from_str("GZ57zaxfgq1eWvHvGtw1ASsydqGRWLCoqM2TmvYuw1Pw").unwrap(),
    //     Pubkey::from_str("GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN").unwrap(),
    //     0,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // redeem collateral
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_redeem_collateral(
    //     authority,
    //     Pubkey::from_str("5nBpNCqkH8aKpUkJjruykZsuSjmLKSzCYEnAb2p8TB13").unwrap(),
    //     Pubkey::from_str("Ev7ugN8CcahvjRXeByFWejhCLhRG9gYZ8s4QReKHRxNP").unwrap(),
    //     Pubkey::from_str("HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J").unwrap(),
    //     Pubkey::from_str("6weJxYMjio6qAoXvNafpzgwCF3fi1knQkgm6DHg1WN1J").unwrap(),
    //     Pubkey::from_str("6MRdknnThzPSz1vkfMAYWnepnAF5wGitRTNrJ6rrQe1s").unwrap(),
    //     Pubkey::from_str("3vtj3VomHHAoqHKtJQL1ymEP6GQmzXHb9TD1LRkBoxFq").unwrap(),
    //     vec![Pubkey::from_str("GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b").unwrap()],
    //     Pubkey::from_str("GZ57zaxfgq1eWvHvGtw1ASsydqGRWLCoqM2TmvYuw1Pw").unwrap(),
    //     Pubkey::from_str("EnpPrZtpsKb2CK6Jyue6tYi4vPmztLXPDKdco3WnRYuS").unwrap(),
    //     0,
    //     100_000_000_000,
    //     block_hash,
    // );
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
    // let authority = Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_liquidate(
    //     authority,
    //     Pubkey::from_str("5nBpNCqkH8aKpUkJjruykZsuSjmLKSzCYEnAb2p8TB13").unwrap(),
    //     Pubkey::from_str("Ev7ugN8CcahvjRXeByFWejhCLhRG9gYZ8s4QReKHRxNP").unwrap(),
    //     Pubkey::from_str("3sAzDiT2dBjrCPsADnRUPEUi8wquWxNynHDCnnU3M8z1").unwrap(),
    //     Pubkey::from_str("HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J").unwrap(),
    //     Pubkey::from_str("6weJxYMjio6qAoXvNafpzgwCF3fi1knQkgm6DHg1WN1J").unwrap(),
    //     Pubkey::from_str("6MRdknnThzPSz1vkfMAYWnepnAF5wGitRTNrJ6rrQe1s").unwrap(),
    //     Pubkey::from_str("3vtj3VomHHAoqHKtJQL1ymEP6GQmzXHb9TD1LRkBoxFq").unwrap(),
    //     vec![Pubkey::from_str("GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b").unwrap()],
    //     Pubkey::from_str("GZ57zaxfgq1eWvHvGtw1ASsydqGRWLCoqM2TmvYuw1Pw").unwrap(),
    //     Pubkey::from_str("GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN").unwrap(),
    //     Pubkey::from_str("EnpPrZtpsKb2CK6Jyue6tYi4vPmztLXPDKdco3WnRYuS").unwrap(),
    //     0,
    //     false,
    //     100_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }






















    // let lamports1 = client.get_minimum_balance_for_rent_exemption(Mint::LEN).unwrap();
    // let lamports2 = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();

    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);

    // let mint = &Keypair::new();
    // let token_account = &Keypair::new();
    // println!("mint key: {:?}, token account pubkey: {:?}", mint.pubkey(), token_account.pubkey());

    // client.request_airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // thread::sleep(Duration::from_secs(30));

    // let (block_hash, _) = client.get_recent_blockhash().unwrap();

    // let transaction = create_token(
    //     mint,
    //     authority, 
    //     token_account, 
    //     lamports1, 
    //     lamports2,
    //     1_000_000_000_000_000_000, 
    //     block_hash
    // ).unwrap();

    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }
}
