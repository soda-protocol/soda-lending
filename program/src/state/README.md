# 数据结构
## ReserveLiquidity
```rust
/// Reserve liquidity
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReserveLiquidity {
    /// 铸币公钥
    pub mint_pubkey: Pubkey,
    /// Reserve LP 精度
    pub mint_decimals: u8,
    /// Reserve LP 提供公钥
    pub supply_pubkey: Pubkey,
    /// Reserve LP 手续费接收公钥
    pub fee_receiver: Pubkey,
    /// Reserve LP 预言机公钥
    pub oracle_pubkey: COption<Pubkey>,
    /// Reserve LP 数额
    pub available_amount: u64,
    /// Reserve LP 借出的
    pub borrowed_amount_wads: Decimal,
    /// Reserve LP 累积借款利率
    pub cumulative_borrow_rate_wads: Decimal,
    /// Reserve LP 报价币的标记价格
    pub market_price: u64,
}

impl ReserveLiquidity {
    // borrowed_amount_wads + available_amount
    pub fn total_supply(&self) -> Result<Decimal, ProgramError>;
    // 添加流动性
    pub fn deposit(&mut self, liquidity_amount: u64) -> ProgramResult;
    // 从可用的流动性中减去借款金额，然后增加借款金额
    pub fn borrow(&mut self, borrow_decimal: Decimal) -> ProgramResult;
    // 将偿还金额加到可用流动性中，并从借款总额中减去结算金额
    pub fn repay(&mut self, repay_amount: u64, settle_amount: Decimal) -> ProgramResult;
    // 计算准备金的流动性利用率 borrowed_amount_wads / (borrowed_amount_wads + available_amount)
    pub fn utilization_rate(&self) -> Result<Rate, ProgramError>;
    // 
}
```