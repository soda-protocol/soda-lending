use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_pack::Pack,
    program_error::ProgramError, pubkey::Pubkey,
};
use spl_token::state::Account;
use crate::{Data, invoker::process_invoke, check_pubkey};

use super::Swapper;

// Raydium devnet does not exist!
const RAYDIUM_PROGRAM: Pubkey = solana_program::pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
const SERUM_PROGRAM: Pubkey = solana_program::pubkey!("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin");
const RAYDIUM_POOL_RAY_USDT: Pubkey = solana_program::pubkey!("DVa7Qmb5ct9RCpaU7UTpSaf3GVMYz17vNVU67XpdCRut");
const RAYDIUM_POOL_RAY_SOL: Pubkey = solana_program::pubkey!("AVs9TA4nWDzfPJE9gGVNJMVhcQy3V9PGazuz33BfG2RA");
const RAYDIUM_POOL_SOL_USDT: Pubkey = solana_program::pubkey!("7XawhbbxtsRcQA8KTkHT9f9nc6d69UwqCDh6U5EEbEmX");
const RAYDIUM_POOL_ETH_SOL: Pubkey = solana_program::pubkey!("9Hm8QX7ZhE9uB8L2arChmmagZZBtBmnzBbpfxzkQp85D");

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
            &RAYDIUM_POOL_RAY_USDT,
            &RAYDIUM_POOL_RAY_SOL,
            &RAYDIUM_POOL_SOL_USDT,
            &RAYDIUM_POOL_ETH_SOL
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
