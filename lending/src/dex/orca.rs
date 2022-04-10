use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_pack::Pack,
    program_error::ProgramError, pubkey::Pubkey,
};
use spl_token::state::Account;
use crate::{Data, invoker::process_invoke, check_pubkey};

use super::Swapper;

// Orca devnet
const ORCA_PROGRAM: Pubkey = solana_program::pubkey!("3xQ8SWv2GaFXXpHZNqkXsdxq5DZciHBz6ZFoPPfbFd7U");
const ORCA_POOL_ORCA_USDT: Pubkey = solana_program::pubkey!("GaCKuVZyo6HxUf6bkcWzDETGHqqViF6H77ax7Uxq3LXU");
const ORCA_POOL_ORCA_SOL: Pubkey = solana_program::pubkey!("B4v9urCKnrdCMWt7rEPyA5xyuEeYQv4aDpCfGFVaCvox");
const ORCA_POOL_SOL_USDT: Pubkey = solana_program::pubkey!("65AsoozQfBedPU3rGCB7CfBbSFhiFGaVQaeoF9mLFM3g");
const ORCA_POOL_ETH_SOL: Pubkey = solana_program::pubkey!("F9MgdfFEshXCTGbppcVr2DzpVxqkiVowGqd95S4vpC6D");

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SwapData {
    /// SOURCE amount to transfer, output to DESTINATION is based on the exchange rate
    amount_in: u64,
    /// Minimum amount of DESTINATION token to output, prevents excessive slippage
    minimum_amount_out: u64,
}

impl Data for SwapData {
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
    pub user_source_token_account: &'a AccountInfo<'b>,
    pub user_dest_token_account: &'a AccountInfo<'b>,
    pub user_authority: &'a AccountInfo<'b>,
    pub signer_seeds: &'a [&'a [u8]],
}

impl<'a, 'b> Swapper<'a, 'b> for OrcaSwapContext<'a, 'b> {
    fn is_supported(&self) -> bool {
        check_pubkey!(
            self.swap_program.key == &ORCA_PROGRAM,
            self.pool_info.key,
            &ORCA_POOL_ORCA_USDT,
            &ORCA_POOL_ORCA_SOL,
            &ORCA_POOL_SOL_USDT,
            &ORCA_POOL_ETH_SOL
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
        let data = SwapData { amount_in, minimum_amount_out };
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

    fn swap_base_out(&self, _max_amount_in: u64, _amount_out: u64) -> ProgramResult {
        unreachable!("Orca is not support for swap base out")
    }
}
