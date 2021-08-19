use std::{convert::TryInto, error::Error, str::FromStr, thread, time::Duration};

use deploy::*;
use solana_client::{
    blockhash_query::BlockhashQuery, 
    rpc_client::RpcClient, 
    rpc_request::TokenAccountsFilter,
};
use solana_sdk::{
    commitment_config::CommitmentConfig, 
    hash::Hash, instruction::Instruction, 
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
    math::WAD,
    state::{Manager, MarketReserve, RateOracle, UserAsset, UserObligation, 
        CollateralConfig, LiquidityConfig
    },
    instruction::{
        init_manager, init_rate_oracle, init_market_reserve_without_liquidity,
        init_market_reserve_with_liquidity, init_user_obligation,
        init_user_asset, deposit_liquidity, withdraw_liquidity,
        deposit_collateral, borrow_liquidity, repay_loan,
        redeem_collateral, liquidate, feed_rate_oracle, pause_rate_oracle,
        add_liquidity_to_market_reserve, withdraw_fee,
    },
    pyth::{self, Product},
};

const DEV_NET: &str = "http://65.21.40.30";
const GLOBAL_OWNER: &str = "vG2VqMokQyY82xKda116qAmvMQm4ymoKEV92UtxNVmu4tKDt4X33ELY4rdCfiR1NxJnbek39m5X9rLJnxASNbmQ";
const QUOTE_CURRENCY: &[u8; 32] = &[85, 83, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

fn main() {
    let client = RpcClient::new_with_commitment(DEV_NET.into(), CommitmentConfig::default());

    //// create manager
    // let lamports = client.get_minimum_balance_for_rent_exemption(Manager::LEN).unwrap();
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let manager = &Keypair::new();
    // println!("manager key: {:?}", manager.pubkey());
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_lending_manager(manager, authority, lamports, block_hash);
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    //// create oracle
    // let lamports = client.get_minimum_balance_for_rent_exemption(RateOracle::LEN).unwrap();
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let rate_oracle = &Keypair::new();
    // println!("rate oracle key: {:?}", rate_oracle.pubkey());
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_rate_oracle(rate_oracle, authority, lamports, block_hash);
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // create market reserve (no liquidity)
    let account_lamports = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();
    let reserve_lamports = client.get_minimum_balance_for_rent_exemption(MarketReserve::LEN).unwrap();
    let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    let (block_hash, _) = client.get_recent_blockhash().unwrap();
    let transaction = create_market_reserve_without_liquidity(
        authority,
        Pubkey::from_str("5nBpNCqkH8aKpUkJjruykZsuSjmLKSzCYEnAb2p8TB13").unwrap(),
        Pubkey::from_str("2weC6fjXrfaCLQpqEzdgBHpz6yVNvmSN133m7LDuZaDb").unwrap(),
        Pubkey::from_str("GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b").unwrap(),
        Pubkey::from_str("6mhUyoQR5CcHN4RJ5PSfcvTjRuWF742ypZeMwptPgFnK").unwrap(),
        CollateralConfig {
            liquidation_1_fee_rate: 25_000_000_000_000_000, // 2.5%
            liquidation_2_repay_rate: 900_000_000_000_000_000,  // 95%
            borrow_value_ratio: 60, 
            liquidation_value_ratio: 70,
            close_factor: 60,
        },
        reserve_lamports,
        account_lamports, 
        block_hash
    ).unwrap();
    match client.send_and_confirm_transaction(&transaction) {
        Ok(sig) => println!("sig is {:?}", sig),
        Err(err) => println!("error: {:?}", err),
    }

    // create market reserve (with liquidity)
    // let account_lamports = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();
    // let reserve_lamports = client.get_minimum_balance_for_rent_exemption(MarketReserve::LEN).unwrap();
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_market_reserve_with_liquidity(
    //     authority,
    //     Pubkey::from_str("5nBpNCqkH8aKpUkJjruykZsuSjmLKSzCYEnAb2p8TB13").unwrap(),
    //     Pubkey::from_str("3m1y5h2uv7EQL3KaJZehvAJa4yDNvgc5yAdL9KPMKwvk").unwrap(),
    //     Pubkey::from_str("HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J").unwrap(),
    //     Pubkey::from_str("9bRWBCW4BHHoLXFLFcLU3FQCDXXLNds1SJBmpeKYFeBZ").unwrap(),
    //     Pubkey::from_str("6weJxYMjio6qAoXvNafpzgwCF3fi1knQkgm6DHg1WN1J").unwrap(),
    //     CollateralConfig {
    //         liquidation_1_fee_rate: 25_000_000_000_000_000, // 2.5%
    //         liquidation_2_repay_rate: 950_000_000_000_000_000,  // 95%
    //         borrow_value_ratio: 70, 
    //         liquidation_value_ratio: 85,
    //         close_factor: 60,
    //     },
    //     LiquidityConfig {
    //         interest_fee_rate: 50_000_000_000_000_000, // 5%
    //         max_borrow_utilization_rate: 60, // 60%
    //     },
    //     reserve_lamports,
    //     account_lamports, 
    //     block_hash
    // ).unwrap();
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // create user obligation
    // let lamports = client.get_minimum_balance_for_rent_exemption(UserObligation::LEN).unwrap();
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_user_obligation(
    //     authority,
    //     &Pubkey::from_str("BL1GswxJmUvNwoxWy77B7gdL9744YJqm4oGFjj94fNxk").unwrap(),
    //     lamports, 
    //     block_hash
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // create user asset
    // let lamports = client.get_minimum_balance_for_rent_exemption(UserAsset::LEN).unwrap();
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_user_asset(
    //     authority,
    //     &Pubkey::from_str("BL1GswxJmUvNwoxWy77B7gdL9744YJqm4oGFjj94fNxk").unwrap(),
    //     lamports, 
    //     block_hash
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // fee rate oracle
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_feed_rate_oracle(
    //     authority,
    //     &Pubkey::from_str("6weJxYMjio6qAoXvNafpzgwCF3fi1knQkgm6DHg1WN1J").unwrap(),
    //     200_000_000_000_000_000,
    //     300_000_000_000_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // deposit liquidity
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_deposit_liquidity(
    //     authority,
    //     &Pubkey::from_str("BL1GswxJmUvNwoxWy77B7gdL9744YJqm4oGFjj94fNxk").unwrap(),
    //     &Pubkey::from_str("7zFv7xf1iczcEdDAKyDu5qBeVDs688pRy6izbkpejmEk").unwrap(),
    //     &Pubkey::from_str("6weJxYMjio6qAoXvNafpzgwCF3fi1knQkgm6DHg1WN1J").unwrap(),
    //     &Pubkey::from_str("CpE7sLcgUorqqgHdmsKerTPW1yWRaLPfQj9pWj7YojHG").unwrap(),
    //     &Pubkey::from_str("GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN").unwrap(),
    //     10_000_000_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // withdraw liquidity
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_withdraw_liquidity(
    //     authority,
    //     &Pubkey::from_str("5nBpNCqkH8aKpUkJjruykZsuSjmLKSzCYEnAb2p8TB13").unwrap(),
    //     &Pubkey::from_str("BL1GswxJmUvNwoxWy77B7gdL9744YJqm4oGFjj94fNxk").unwrap(),
    //     &Pubkey::from_str("7zFv7xf1iczcEdDAKyDu5qBeVDs688pRy6izbkpejmEk").unwrap(),
    //     &Pubkey::from_str("6weJxYMjio6qAoXvNafpzgwCF3fi1knQkgm6DHg1WN1J").unwrap(),
    //     &Pubkey::from_str("CpE7sLcgUorqqgHdmsKerTPW1yWRaLPfQj9pWj7YojHG").unwrap(),
    //     &Pubkey::from_str("GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN").unwrap(),
    //     1_000_000_000_000,
    //     block_hash,
    // );
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // deposit collateral
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = do_deposit_collateral(
    //     authority,
    //     &Pubkey::from_str("7qZBgT8AKpiiCBGzmN5Z1KuqNfk7xtaDSJxnskYRv2A2").unwrap(),
    //     &Pubkey::from_str("743oo4CaCA6AQXE6e29VA1EQiqPupo2wD4kHcvdKzj5o").unwrap(),
    //     &Pubkey::from_str("HqDgtCXwyyaARnURGWeBAC2ZfNATivPKr5YkYZeFRcQm").unwrap(),
    //     &Pubkey::from_str("EnpPrZtpsKb2CK6Jyue6tYi4vPmztLXPDKdco3WnRYuS").unwrap(),
    //     500_000_000_000,
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
    //     Pubkey::from_str("BL1GswxJmUvNwoxWy77B7gdL9744YJqm4oGFjj94fNxk").unwrap(),
    //     Pubkey::from_str("7zFv7xf1iczcEdDAKyDu5qBeVDs688pRy6izbkpejmEk").unwrap(),
    //     Pubkey::from_str("HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J").unwrap(),
    //     Pubkey::from_str("6weJxYMjio6qAoXvNafpzgwCF3fi1knQkgm6DHg1WN1J").unwrap(),
    //     Pubkey::from_str("HqDgtCXwyyaARnURGWeBAC2ZfNATivPKr5YkYZeFRcQm").unwrap(),
    //     Pubkey::from_str("GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN").unwrap(),
    //     vec![Pubkey::from_str("GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b").unwrap()],
    //     1_000_000,
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

// fn get_pyth_price(pyth_price_data: &[u8]) -> Result<u64, ProgramError> {
//     const STALE_AFTER_SLOTS_ELAPSED: u64 = 5;

//     let pyth_price = pyth::load::<pyth::Price>(pyth_price_data)
//         .map_err(|_| ProgramError::InvalidAccountData)?;

//     println!("{:?}", pyth_price.ptype);
//     Ok(0)
// }
