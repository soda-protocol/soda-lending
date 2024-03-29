use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_pack::Pack,
    program_error::ProgramError, pubkey::Pubkey,
};
use spl_token::state::Account;
use crate::{Data, invoker::process_invoke, check_pubkey};

use super::Swapper;

// mainnet
const RAYDIUM_PROGRAM: Pubkey = solana_program::pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
const SERUM_PROGRAM: Pubkey = solana_program::pubkey!("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin");
const RAYDIUM_POOL_SOL_USDC: Pubkey = solana_program::pubkey!("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2");
const RAYDIUM_POOL_SOL_USDT: Pubkey = solana_program::pubkey!("7XawhbbxtsRcQA8KTkHT9f9nc6d69UwqCDh6U5EEbEmX");
const RAYDIUM_POOL_BTC_MSOL: Pubkey = solana_program::pubkey!("ynV2H2b7FcRBho2TvE25Zc4gDeuu2N45rUw9DuJYjJ9");
const RAYDIUM_POOL_BTC_SRM: Pubkey = solana_program::pubkey!("DvxLb4NnQUYq1gErk35HVt9g8kxjNbviJfiZX1wqraMv");
const RAYDIUM_POOL_BTC_USDC: Pubkey = solana_program::pubkey!("6kbC5epG18DF2DwPEW34tBy5pGFS7pEGALR3v5MGxgc5");
const RAYDIUM_POOL_ETH_MSOL: Pubkey = solana_program::pubkey!("Ghj3v2qYbSp6XqmH4NV4KRu4Rrgqoh2Ra7L9jEdsbNzF");
const RAYDIUM_POOL_ETH_SOL: Pubkey = solana_program::pubkey!("9Hm8QX7ZhE9uB8L2arChmmagZZBtBmnzBbpfxzkQp85D");
const RAYDIUM_POOL_ETH_SRM: Pubkey = solana_program::pubkey!("3XwxHcbyqcd1xkdczaPv3TNCZsevELD4Zux3pu4sF2D8");
const RAYDIUM_POOL_ETH_USDC: Pubkey = solana_program::pubkey!("AoPebtuJC4f2RweZSxcVCcdeTgaEXY64Uho8b5HdPxAR");
const RAYDIUM_POOL_ETH_USDT: Pubkey = solana_program::pubkey!("He3iAEV5rYjv6Xf7PxKro19eVrC3QAcdic5CF2D2obPt");
const RAYDIUM_POOL_RAY_ETH: Pubkey = solana_program::pubkey!("8iQFhWyceGREsWnLM8NkG9GC8DvZunGZyMzuyUScgkMK");
const RAYDIUM_POOL_RAY_SOL: Pubkey = solana_program::pubkey!("AVs9TA4nWDzfPJE9gGVNJMVhcQy3V9PGazuz33BfG2RA");
const RAYDIUM_POOL_RAY_SRM: Pubkey = solana_program::pubkey!("GaqgfieVmnmY4ZsZHHA6L5RSVzCGL3sKx4UgHBaYNy8m");
const RAYDIUM_POOL_RAY_USDC: Pubkey = solana_program::pubkey!("6UmmUiYoBjSrhakAobJw8BvkmJtDVxaeBtbt7rxWo1mg");
const RAYDIUM_POOL_RAY_USDT: Pubkey = solana_program::pubkey!("DVa7Qmb5ct9RCpaU7UTpSaf3GVMYz17vNVU67XpdCRut");
const RAYDIUM_POOL_SRM_SOL: Pubkey = solana_program::pubkey!("EvWJC2mnmu9C9aQrsJLXw8FhUcwBzFEUQsP1E5Y6a5N7");
const RAYDIUM_POOL_SRM_USDC: Pubkey = solana_program::pubkey!("8tzS7SkUZyHPQY7gLqsMCXZ5EDCgjESUHcB17tiR1h3Z");
const RAYDIUM_POOL_SRM_USDT: Pubkey = solana_program::pubkey!("af8HJg2ffWoKJ6vKvkWJUJ9iWbRR83WgXs8HPs26WGr");
const RAYDIUM_POOL_MSOL_RAY: Pubkey = solana_program::pubkey!("6gpZ9JkLoYvpA5cwdyPZFsDw6tkbPyyXM5FqRqHxMCny");
const RAYDIUM_POOL_MSOL_SOL: Pubkey = solana_program::pubkey!("EGyhb2uLAsRUbRx9dNFBjMVYnFaASWMvD6RE1aEf2LxL");
const RAYDIUM_POOL_MSOL_USDC: Pubkey = solana_program::pubkey!("ZfvDXXUhZDzDVsapffUyXHj9ByCoPjP4thL6YXcZ9ix");
const RAYDIUM_POOL_MSOL_USDT: Pubkey = solana_program::pubkey!("BhuMVCzwFVZMSuc1kBbdcAnXwFg9p4HJp7A9ddwYjsaF");
const RAYDIUM_POOL_WBWBNB_USDC: Pubkey = solana_program::pubkey!("Fb1WR1kYvG1tHu4pwAxXQpdKT8Grh9i7ES9rZusLg7D6");
const RAYDIUM_POOL_WEWETH_SOL: Pubkey = solana_program::pubkey!("4yrHms7ekgTBgJg77zJ33TsWrraqHsCXDtuSZqUsuGHb");
const RAYDIUM_POOL_WEWETH_USDC: Pubkey = solana_program::pubkey!("EoNrn8iUhwgJySD1pHu8Qxm5gSQqLK3za4m8xzD2RuEb");

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SwapInstructionBaseIn {
    // SOURCE amount to transfer, output to DESTINATION is based on the exchange rate
    pub amount_in: u64,
    /// Minimum amount of DESTINATION token to output, prevents excessive slippage
    pub minimum_amount_out: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SwapInstructionBaseOut {
    // SOURCE amount to transfer, output to DESTINATION is based on the exchange rate
    pub max_amount_in: u64,
    /// Minimum amount of DESTINATION token to output, prevents excessive slippage
    pub amount_out: u64,
}

impl Data for SwapInstructionBaseIn {
    fn to_vec(self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + 8 + 8);
        buf.push(9);
        buf.extend_from_slice(&self.amount_in.to_le_bytes());
        buf.extend_from_slice(&self.minimum_amount_out.to_le_bytes());

        buf
    }
}

impl Data for SwapInstructionBaseOut {
    fn to_vec(self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + 8 + 8);
        buf.push(11);
        buf.extend_from_slice(&self.max_amount_in.to_le_bytes());
        buf.extend_from_slice(&self.amount_out.to_le_bytes());

        buf
    }
}

#[derive(Debug)]
pub struct RaydiumSwapContext<'a, 'b> {
    pub swap_program: &'a AccountInfo<'b>,
    pub token_program: &'a AccountInfo<'b>,
    pub amm_info: &'a AccountInfo<'b>,
    pub amm_authority: &'a AccountInfo<'b>,
    pub amm_open_orders: &'a AccountInfo<'b>,
    pub amm_target_orders: &'a AccountInfo<'b>,
    pub pool_source_token_account: &'a AccountInfo<'b>,
    pub pool_dest_token_account: &'a AccountInfo<'b>,
    pub serum_program: &'a AccountInfo<'b>,
    pub serum_market: &'a AccountInfo<'b>,
    pub serum_bids: &'a AccountInfo<'b>,
    pub serum_asks: &'a AccountInfo<'b>,
    pub serum_event_queue: &'a AccountInfo<'b>,
    pub serum_source_token_account: &'a AccountInfo<'b>,
    pub serum_dest_token_account: &'a AccountInfo<'b>,
    pub serum_vault_signer: &'a AccountInfo<'b>,
    pub user_source_token_account: &'a AccountInfo<'b>,
    pub user_dest_token_account: &'a AccountInfo<'b>,
    pub user_authority: &'a AccountInfo<'b>,
    pub signer_seeds: &'a [&'a [u8]],
}

impl<'a, 'b> Swapper<'a, 'b> for RaydiumSwapContext<'a, 'b> {
    fn is_supported(&self) -> bool {       
        check_pubkey!(
            self.swap_program.key == &RAYDIUM_PROGRAM && self.serum_program.key == &SERUM_PROGRAM,
            self.amm_info.key,
            &RAYDIUM_POOL_SOL_USDC,
            &RAYDIUM_POOL_SOL_USDT,
            &RAYDIUM_POOL_BTC_MSOL,
            &RAYDIUM_POOL_BTC_SRM,
            &RAYDIUM_POOL_BTC_USDC,
            &RAYDIUM_POOL_ETH_SOL,
            &RAYDIUM_POOL_ETH_MSOL,
            &RAYDIUM_POOL_ETH_SRM,
            &RAYDIUM_POOL_ETH_USDC,
            &RAYDIUM_POOL_ETH_USDT,
            &RAYDIUM_POOL_RAY_ETH,
            &RAYDIUM_POOL_RAY_SRM,
            &RAYDIUM_POOL_RAY_USDC,
            &RAYDIUM_POOL_RAY_USDT,
            &RAYDIUM_POOL_RAY_SOL,
            &RAYDIUM_POOL_SRM_SOL,
            &RAYDIUM_POOL_SRM_USDC,
            &RAYDIUM_POOL_SRM_USDT,
            &RAYDIUM_POOL_MSOL_RAY,
            &RAYDIUM_POOL_MSOL_SOL,
            &RAYDIUM_POOL_MSOL_USDC,
            &RAYDIUM_POOL_MSOL_USDT,
            &RAYDIUM_POOL_WBWBNB_USDC,
            &RAYDIUM_POOL_WEWETH_SOL,
            &RAYDIUM_POOL_WEWETH_USDC
        )
    }

    fn get_user_source_token_balance(&self) -> Result<u64, ProgramError> {
        Ok(Account::unpack(&self.user_source_token_account.try_borrow_data()?)?.amount)
    }

    fn get_user_dest_token_balance(&self) -> Result<u64, ProgramError> {
        Ok(Account::unpack(&self.user_dest_token_account.try_borrow_data()?)?.amount)
    }

    fn get_pool_source_token_balance(&self) -> Result<u64, ProgramError> {
        Ok(Account::unpack(&self.pool_source_token_account.try_borrow_data()?)?.amount)
    }

    fn get_pool_dest_token_balance(&self) -> Result<u64, ProgramError> {
        Ok(Account::unpack(&self.pool_dest_token_account.try_borrow_data()?)?.amount)
    }

    fn swap_base_in(&self, amount_in: u64, minimum_amount_out: u64) -> ProgramResult {
        let data = SwapInstructionBaseIn { amount_in, minimum_amount_out };
        let mut user_authority = self.user_authority.clone();
        user_authority.is_signer = true;

        process_invoke(
            data,
            self.swap_program,
            vec![
                self.token_program.clone(),
                self.amm_info.clone(),
                self.amm_authority.clone(),
                self.amm_open_orders.clone(),
                self.amm_target_orders.clone(),
                self.pool_source_token_account.clone(),
                self.pool_dest_token_account.clone(),
                self.serum_program.clone(),
                self.serum_market.clone(),
                self.serum_bids.clone(),
                self.serum_asks.clone(),
                self.serum_event_queue.clone(),
                self.serum_source_token_account.clone(),
                self.serum_dest_token_account.clone(),
                self.serum_vault_signer.clone(),
                self.user_source_token_account.clone(),
                self.user_dest_token_account.clone(),
                user_authority,
            ],
            self.signer_seeds,
        )
    }

    fn swap_base_out(&self, max_amount_in: u64, amount_out: u64) -> ProgramResult {
        let data = SwapInstructionBaseOut { max_amount_in, amount_out };
        let mut user_authority = self.user_authority.clone();
        user_authority.is_signer = true;

        process_invoke(
            data,
            self.swap_program,
            vec![
                self.token_program.clone(),
                self.amm_info.clone(),
                self.amm_authority.clone(),
                self.amm_open_orders.clone(),
                self.amm_target_orders.clone(),
                self.pool_source_token_account.clone(),
                self.pool_dest_token_account.clone(),
                self.serum_program.clone(),
                self.serum_market.clone(),
                self.serum_bids.clone(),
                self.serum_asks.clone(),
                self.serum_event_queue.clone(),
                self.serum_source_token_account.clone(),
                self.serum_dest_token_account.clone(),
                self.serum_vault_signer.clone(),
                self.user_source_token_account.clone(),
                self.user_dest_token_account.clone(),
                user_authority,
            ],
            self.signer_seeds,
        )
    }
}
