# Soda Lending Contract
## program id
SodatMqSurD1AuSB8MBrYKe29Du25nzqTPGk6xhJyNJ

---

## pack transaction
### init obligation
- create obligation keypair
- instructions
    - create account
    - [init obligation](#init_obligation)
- signing keypairs
    - obligation
    - authority

### deposit + pledge
- instructions
    - [deposit](#deposit)
    - [pledge](#pledge)
- signing keypairs
    - authority

### borrow
- instructions
    - [update market reserves (1)](#update_market_reserves)
    - [update market reserves (1)](#update_market_reserves)
    - [update user obligation](#update_user_obligation)
    - [borrow](#borrow)
- signing keypairs
    - authorrity

### repay
- instructions
    - [repay](#repay)
- signing keypairs
    - authorrity

### withdraw 1
- 当然loans不为空的时候
- instructions
    - [update market reserves (1)](#update_market_reserves)
    - [update market reserves (1)](#update_market_reserves)
    - [update user obligation](#update_user_obligation)
    - [redeem](#redeem)
    - [withdraw](#withdraw)
- signing keypairs
    - authorrity

### withdraw 2
- 当然loans为空的时候
- instructions
    - [redeem without loan](#redeem_without_loan)
    - [withdraw](#withdraw)
- signing keypairs
    - authorrity

---


## instructions for front end
### <span id = "init_obligation">init obligation</span>
- accounts
    - rent pubkey
    - clock pubkey
    - manager pubkey
    - obligation pubkey *Writable*
    - user authority pubkey
- data
    - InitUserObligation

### <span id = "update_obligation">update obligation</span>
- accounts
    - clock pubkey
    - obligation pubkey *Writable*
    - market reserve 1 pubkey
    - market reserve 2 pubkey
    - market reserve .. pubkey
- data
    - RefreshUserObligation
- remark
    - **market reserve pubkeys 是用户obligation中collaterals和loans里包含的所有market reserve**

### <span id = "update_market_reserves">update market reserves</span>
- accounts
    - clock pubkey
    - market reserve 1 pubkey *Writable*
    - price oracle 1 pubkey
    - rate oracle 1 pubkey
    - market reserve 2 pubkey *Writable*
    - price oracle 2 pubkey
    - rate oracle 2 pubkey
    - market reserve .. pubkey *Writable*
    - price oracle .. pubkey
    - rate oracle .. pubkey
- data
    - RefreshMarketReserves
- remark
    - **market reserve pubkey 要按照用户obligation中collaterals和loans里的market reserve依次排序，rate oracle和price oracle要和market reserve对应**

### <span id = "deposit">deposit</span>
- accounts
    - clock pubkey
    - manager pubkey
    - manager authority pubkey
    - market reserve pubkey *Writable*
    - sotoken mint pubkey *Writable*
    - manager token account pubkey *Writable*
    - rate oracle pubkey
    - user authority pubkey **Signer**
    - user token account pubkey *Writable*
    - user sotoken account pubkey *Writable*
    - spl token program
- data
    - Deposit{ amount }

### <span id = "pledge">pledge</span>
- accounts
    - market reserve pubkey
    - sotoken mint pubkey *Writable*
    - user obligation pubkey *Writable*
    - user authority pubkey  **Signer**
    - user sotoken account pubkey *Writable*
    - spl token program
- data
    - PledgeCollateral{ amount }

### <span id = "borrow">borrow</span>
- accounts
    - clock pubkey
    - manager pubkey
    - manager authority pubkey 
    - market reserve pubkey *Writable*
    - manager token account key *Writable*
    - user obligation pubkey *Writable*
    - user authority pubkey  **Signer**
    - user token account key *Writable*
    - spl token program
- data
    - BorrowLiquidity{ amount }

### <span id = "repay">repay</span>
- accounts
    - clock pubkey
    - market reserve pubkey *Writable*
    - manager token account key *Writable*
    - rate oracle pubkey
    - user obligation pubkey *Writable*
    - user authority pubkey  **Signer**
    - user token account key *Writable*
    - spl token program
- data
    - RepayLoan{ amount }

### <span id = "redeem">redeem</span>
- accounts
    - clock pubkey
    - manager pubkey
    - manager authority pubkey
    - market reserve pubkey
    - sotoken mint pubkey *Writable*
    - user obligation pubkey *Writable*
    - user authority pubkey  **Signer**
    - user sotoken account pubkey *Writable*
    - spl token program
- data
    - RedeemCollateral{ amount }

### <span id = "redeem_without_loan">redeem without loan</span>
- accounts
    - manager pubkey
    - manager authority pubkey
    - market reserve pubkey
    - sotoken mint pubkey *Writable*
    - user obligation pubkey *Writable*
    - user authority pubkey  **Signer**
    - user sotoken account pubkey *Writable*
    - spl token program
- data
    - RedeemCollateralWithoutLoan{ amount }

### <span id = "withdraw">withdraw</span>
- accounts
    - clock pubkey
    - manager pubkey
    - manager authority pubkey
    - market reserve pubkey *Writable*
    - sotoken mint pubkey *Writable*
    - manager token account pubkey *Writable*
    - rate oracle pubkey
    - user authority pubkey  **Signer**
    - user token account pubkey *Writable*
    - user sotoken account pubkey *Writable*
    - spl token program
- data
    - Withdraw{ amount }

## instructions for unique credit
- data struct
```rust
pub struct UniqueCredit {
    /// 
    pub version: u8,
    ///
    pub owner: Pubkey,
    ///
    pub reserve: Pubkey,
    ///
    pub borrow_limit: u64,
    ///
    pub acc_borrow_rate_wads: Decimal,
    ///
    pub borrowed_amount_wads: Decimal,
}

pub struct MarketReserve {
    ///
    pub version: u8,
    ///
    pub last_update: LastUpdate,
    /// size: 32 byte (used in instruction)
    pub manager: Pubkey,
    /// 
    pub market_price: Decimal,
    /// 
    pub token_info: TokenInfo,
    /// size: 43 byte
    pub collateral_info: CollateralInfo,
    /// size: 83 byte
    pub liquidity_info: LiquidityInfo,
    /// size: 33 byte
    pub rate_model: RateModel,
    /// padding size: 256 byte
}

pub struct TokenInfo {
    ///
    pub mint_pubkey: Pubkey,
    ///
    pub supply_account: Pubkey,
    ///
    pub price_oracle: Pubkey,
    ///
    pub decimal: u8,
}
```

- borrow liquidity
    - *soda will approve final amount from `supply_token_account_key` for `authority_key` as delegate*
```rust
pub fn borrow_liquidity_by_unique_credit(
    // manager pubkey which owns by soda program (should equals to field in MarketReserve)
    manager_key: Pubkey,
    // market reserve pubkey which owns by soda program
    market_reserve_key: Pubkey,
    // supply token account of reserve which owns by soda program (should equals to field in TokenInfo)
    supply_token_account_key: Pubkey, 
    // credit pubket which owns by soda program (created by soda admin)
    unique_credit_key: Pubkey,
    // authority of credit owner
    authority_key: Pubkey,
    // borrow amount (u64::MAX represents borrow all liquidity from reserve)
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new_readonly(manager_authority_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new(unique_credit_key, false),
            AccountMeta::new_readonly(authority_key, true),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::BorrowLiquidityByUniqueCredit{ amount }.pack(),
    }
}
```
- repay loan
    - *credit owner should approve repaying amount from `source_token_account_key` for `manager_authority_key` as delegate*
```rust
pub fn repay_loan_by_unique_credit(
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    supply_token_account_key: Pubkey,
    unique_credit_key: Pubkey,
    // repaying token account
    source_token_account_key: Pubkey,
    // borrow amount (u64::MAX represents repaying as much as possible,
    // both considering token account balance and dept amount)
    amount: u64,
) -> Instruction {
    let program_id = id();
    let (manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &program_id,
    );

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(manager_key, false),
            AccountMeta::new_readonly(manager_authority_key, false),
            AccountMeta::new(market_reserve_key, false),
            AccountMeta::new(supply_token_account_key, false),
            AccountMeta::new(unique_credit_key, false),
            AccountMeta::new(source_token_account_key, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: LendingInstruction::RepayLoanByUniqueCredit{ amount }.pack(),
    }
}
```