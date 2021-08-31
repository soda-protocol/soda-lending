use super::*;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TokenInfo {
    ///
    pub mint_pubkey: Pubkey,
    ///
    pub account: Pubkey,
    ///
    pub price_oracle: Pubkey,
    ///
    pub decimal: u8,
}

///
#[derive(Clone, Debug, Copy, Default, PartialEq)]
pub struct CollateralConfig {
    ///
    pub borrow_value_ratio: u8,
    ///
    pub liquidation_value_ratio: u8,
    ///
    pub close_factor: u8,
}

///
#[derive(Clone, Debug, Copy, Default, PartialEq)]
pub struct LiquidityConfig {
    ///
    pub borrow_fee_rate: u64,
    ///
    pub liquidation_fee_rate: u64,
    ///
    pub flash_loan_fee_rate: u64,
    ///
    pub max_deposit: u64,
    ///
    pub max_acc_deposit: u64,
}

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiquidityInfo {
    ///
    pub rate_oracle: Pubkey,
    ///
    pub available: u64,
    ///
    pub borrowed_amount_wads: Decimal,
    ///
    pub acc_borrow_rate_wads: Decimal,
    ///
    pub fee: u64,
    ///
    pub config: LiquidityConfig,
}

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollateralInfo {
    ///
    pub sotoken_mint_pubkey: Pubkey,
    ///
    pub total_mint: u64,
    ///
    pub config: CollateralConfig,
}

/// Lending market reserve state
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarketReserve {
    /// Version of the struct
    pub version: u8,
    ///
    pub enable: bool,
    ///
    pub last_update: LastUpdate,
    /// 
    pub manager: Pubkey,
    ///
    pub market_price: Decimal,
    ///
    pub token_info: TokenInfo,
    ///
    pub collateral_info: CollateralInfo,
    ///
    pub liquidity_info: LiquidityInfo,
}

impl Sealed for MarketReserve {}
impl IsInitialized for MarketReserve {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const MARKET_RESERVE_PADDING_LEN: usize = 128;
const MARKET_RESERVE_LEN: usize = 447;

impl Pack for MarketReserve {
    const LEN: usize = MARKET_RESERVE_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, MARKET_RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            enable,
            last_update,
            manager,
            market_price,
            mint_pubkey,
            account,
            price_oracle,
            decimal,
            sotoken_mint_pubkey,
            total_mint,
            borrow_value_ratio,
            liquidation_value_ratio,
            close_factor,
            rate_oracle,
            available,
            borrowed_amount_wads,
            acc_borrow_rate_wads,
            fee,
            borrow_fee_rate,
            liquidation_fee_rate,
            flash_loan_fee_rate,
            max_deposit,
            max_acc_deposit,
            _padding,
        ) = mut_array_refs![
            output,
            1,
            1,
            LAST_UPDATE_LEN,
            PUBKEY_BYTES,
            16,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1,
            PUBKEY_BYTES,
            8,
            1,
            1,
            1,
            PUBKEY_BYTES,
            8,
            16,
            16,
            8,
            8,
            8,
            8,
            8,
            8,
            MARKET_RESERVE_PADDING_LEN
        ];

        *version = self.version.to_le_bytes();
        pack_bool(self.enable, enable);
        self.last_update.pack_into_slice(&mut last_update[..]);
        manager.copy_from_slice(self.manager.as_ref());
        pack_decimal(self.market_price, market_price);

        mint_pubkey.copy_from_slice(self.token_info.mint_pubkey.as_ref());
        account.copy_from_slice(self.token_info.account.as_ref());
        price_oracle.copy_from_slice(self.token_info.price_oracle.as_ref());
        *decimal = self.token_info.decimal.to_le_bytes();

        sotoken_mint_pubkey.copy_from_slice(self.collateral_info.sotoken_mint_pubkey.as_ref());
        *total_mint = self.collateral_info.total_mint.to_le_bytes();

        *borrow_value_ratio = self.collateral_info.config.borrow_value_ratio.to_le_bytes();
        *liquidation_value_ratio = self.collateral_info.config.liquidation_value_ratio.to_le_bytes();
        *close_factor = self.collateral_info.config.close_factor.to_le_bytes();

        rate_oracle.copy_from_slice(self.liquidity_info.rate_oracle.as_ref());
        *available = self.liquidity_info.available.to_le_bytes();
        pack_decimal(self.liquidity_info.borrowed_amount_wads, borrowed_amount_wads);
        pack_decimal(self.liquidity_info.acc_borrow_rate_wads, acc_borrow_rate_wads);
        *fee = self.liquidity_info.fee.to_le_bytes();

        *borrow_fee_rate = self.liquidity_info.config.borrow_fee_rate.to_le_bytes();
        *liquidation_fee_rate = self.liquidity_info.config.liquidation_fee_rate.to_le_bytes();
        *flash_loan_fee_rate = self.liquidity_info.config.flash_loan_fee_rate.to_le_bytes();
        *max_deposit = self.liquidity_info.config.max_deposit.to_le_bytes();
        *max_acc_deposit = self.liquidity_info.config.max_acc_deposit.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, SodaError> {
        let input = array_ref![input, 0, MARKET_RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            enable,
            last_update,
            manager,
            market_price,
            mint_pubkey,
            account,
            price_oracle,
            decimal,
            sotoken_mint_pubkey,
            total_mint,
            borrow_value_ratio,
            liquidation_value_ratio,
            close_factor,
            rate_oracle,
            available,
            borrowed_amount_wads,
            acc_borrow_rate_wads,
            fee,
            borrow_fee_rate,
            liquidation_fee_rate,
            flash_loan_fee_rate,
            max_deposit,
            max_acc_deposit,
            _padding,
        ) = array_refs![
            input,
            1,
            1,
            LAST_UPDATE_LEN,
            PUBKEY_BYTES,
            16,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1,
            PUBKEY_BYTES,
            8,
            1,
            1,
            1,
            PUBKEY_BYTES,
            8,
            16,
            16,
            8,
            8,
            8,
            8,
            8,
            8,
            MARKET_RESERVE_PADDING_LEN
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            return Err(SodaError::UnpackError);
        }

        Ok(Self {
            version,
            enable: unpack_bool(enable)?,
            last_update: LastUpdate::unpack_from_slice(&last_update[..])?,
            manager: Pubkey::new_from_array(*manager),
            market_price: unpack_decimal(market_price),
            token_info: TokenInfo {
                mint_pubkey: Pubkey::new_from_array(*mint_pubkey),
                account: Pubkey::new_from_array(*account),
                price_oracle: Pubkey::new_from_array(*price_oracle),
                decimal: u8::from_le_bytes(*decimal),
            },
            collateral_info: CollateralInfo {
                sotoken_mint_pubkey: Pubkey::new_from_array(*sotoken_mint_pubkey),
                total_mint: u64::from_le_bytes(*total_mint),
                config: CollateralConfig {
                    borrow_value_ratio: u8::from_le_bytes(*borrow_value_ratio),
                    liquidation_value_ratio: u8::from_le_bytes(*liquidation_value_ratio),
                    close_factor: u8::from_le_bytes(*close_factor),
                },
            },
            liquidity_info: LiquidityInfo {
                rate_oracle: Pubkey::new_from_array(*rate_oracle),
                available: u64::from_le_bytes(*available),
                borrowed_amount_wads: unpack_decimal(borrowed_amount_wads),
                acc_borrow_rate_wads: unpack_decimal(acc_borrow_rate_wads),
                fee: u64::from_le_bytes(*fee),
                config: LiquidityConfig {
                    borrow_fee_rate: u64::from_le_bytes(*borrow_fee_rate),
                    liquidation_fee_rate: u64::from_le_bytes(*liquidation_fee_rate),
                    flash_loan_fee_rate: u64::from_le_bytes(*flash_loan_fee_rate),
                    max_deposit: u64::from_le_bytes(*max_deposit),
                    max_acc_deposit: u64::from_le_bytes(*max_acc_deposit),
                },
            },
        })
    }
}