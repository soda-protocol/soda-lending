use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_pack::Pack,
    program_error::ProgramError, pubkey::Pubkey,
};
use spl_token::state::Account;
use crate::{Data, invoker::process_invoke};

use super::Swapper;

// Orca mainnet
const ORCA_PROGRAM: Pubkey = solana_program::pubkey!("9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP");
const ORCA_POOL_SOL_USDT: Pubkey = solana_program::pubkey!("Dqk7mHQBx2ZWExmyrR2S8X6UG75CrbbpK2FSBZsNYsw6");
const ORCA_POOL_SOL_USDC: Pubkey = solana_program::pubkey!("EGZ7tiLeH62TPV1gL8WwbXGzEPa9zmcpVnnkPKKnrE2U");
const ORCA_POOL_ORCA_USDT: Pubkey = solana_program::pubkey!("4YnaUPeZ2fYqpoLrCyprSai8LaDWZxmgb6cGfNHJmyP6");
const ORCA_POOL_ORCA_USDC: Pubkey = solana_program::pubkey!("2p7nYbtPBgtmY69NsE8DAW6szpRJn7tQvDnqvoEWQvjY");
const ORCA_POOL_ORCA_SOL: Pubkey = solana_program::pubkey!("2ZnVuidTHpi5WWKUwFXauYGhvdT9jRKYv5MDahtbwtYr");
const ORCA_POOL_BTC_USDC: Pubkey = solana_program::pubkey!("2dwHmCoAGxCXvTbLTMjqAhvEFAHWUt9kZaroJJJdmoD4");
const ORCA_POOL_BTC_SOL: Pubkey = solana_program::pubkey!("7N2AEJ98qBs4PwEwZ6k5pj8uZBKMkZrKZeiC7A64B47u");
const ORCA_POOL_ETH_USDC: Pubkey = solana_program::pubkey!("FgZut2qVQEyPBibaTJbbX2PxaMZvT1vjDebiVaDp5BWP");
const ORCA_POOL_ETH_SOL: Pubkey = solana_program::pubkey!("EuK3xDa4rWuHeMQCBsHf1ETZNiEQb5C476oE9u9kp8Ji");

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct OrcaSwapData {
    /// SOURCE amount to transfer, output to DESTINATION is based on the exchange rate
    amount_in: u64,
    /// Minimum amount of DESTINATION token to output, prevents excessive slippage
    minimum_amount_out: u64,
}

impl Data for OrcaSwapData {
    fn to_vec(self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + 8 + 8);
        buf.push(1);
        buf.extend_from_slice(&self.amount_in.to_le_bytes());
        buf.extend_from_slice(&self.minimum_amount_out.to_le_bytes());

        buf
    }
}

#[derive(Debug)]
pub struct OrcaSwapContext<'a, 'b> {
    pub swap_program: &'a AccountInfo<'b>,
    pub token_program: &'a AccountInfo<'b>,
    pub pool_info: &'a AccountInfo<'b>,
    pub pool_authority: &'a AccountInfo<'b>,
    pub pool_lp_token_mint: &'a AccountInfo<'b>,
    pub pool_source_token_account: &'a AccountInfo<'b>,
    pub pool_dest_token_account: &'a AccountInfo<'b>,
    pub pool_fee_account: &'a AccountInfo<'b>,
    pub user_authority: &'a AccountInfo<'b>,
    pub user_source_token_account: &'a AccountInfo<'b>,
    pub user_dest_token_account: &'a AccountInfo<'b>,
    pub signer_seeds: &'a [&'a [u8]],
}

impl<'a, 'b> Swapper<'a, 'b> for OrcaSwapContext<'a, 'b> {
    fn is_supported(&self) -> bool {
        let a = self.swap_program.key == &ORCA_PROGRAM;
        let b = (self.pool_info.key == &ORCA_POOL_SOL_USDT) ||
            (self.pool_info.key == &ORCA_POOL_SOL_USDC) ||
            (self.pool_info.key == &ORCA_POOL_ORCA_USDT) ||
            (self.pool_info.key == &ORCA_POOL_ORCA_USDC) ||
            (self.pool_info.key == &ORCA_POOL_ORCA_SOL) ||
            (self.pool_info.key == &ORCA_POOL_BTC_USDC) ||
            (self.pool_info.key == &ORCA_POOL_BTC_SOL) ||
            (self.pool_info.key == &ORCA_POOL_ETH_USDC) ||
            (self.pool_info.key == &ORCA_POOL_ETH_SOL);
        a && b
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

    fn swap(&self, amount_in: u64, minimum_amount_out: u64) -> ProgramResult {
        let data = OrcaSwapData { amount_in, minimum_amount_out };
        let mut user_authority = self.user_authority.clone();
        user_authority.is_signer = true;

        process_invoke(
            data,
            self.swap_program,
            vec![
                self.pool_info.clone(),
                self.pool_authority.clone(),
                user_authority,
                self.user_source_token_account.clone(),
                self.pool_source_token_account.clone(),
                self.pool_dest_token_account.clone(),
                self.user_dest_token_account.clone(),
                self.pool_lp_token_mint.clone(),
                self.pool_fee_account.clone(),
                self.token_program.clone(),
            ],
            self.signer_seeds,
        )
    }
}
