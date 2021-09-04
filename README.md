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