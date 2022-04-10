#![allow(missing_docs)]

#[macro_export]
macro_rules! get_rent {
    ($ri:ident, $rent:ident; $iter:expr) => {
        let $ri = next_account_info($iter)?;
        let $rent = &Rent::from_account_info($ri)?;
    };
}

#[macro_export]
macro_rules! get_clock {
    ($ci:ident, $clock:ident; $iter:expr) => {
        let $ci = next_account_info($iter)?;
        let $clock = &Clock::from_account_info($ci)?;
    };
}

#[macro_export]
macro_rules! create_manager {
    ($mi:ident; $iter:expr, $id:expr, $rent:expr) => {
        let $mi = next_account_info($iter)?;
        if $mi.owner != $id {
            msg!("Manager provided is not owned by the lending program");
            return Err(LendingError::InvalidAccountOwner.into());
        }
        assert_rent_exempt($rent, $mi)?;
        assert_uninitialized::<Manager>($mi)?;
    };
}

#[macro_export]
macro_rules! create_market_reserve {
    ($mi:ident; $iter:expr, $id:expr, $rent:expr) => {
        let $mi = next_account_info($iter)?;
        if $mi.owner != $id {
            msg!("Market reserve provided is not owned by the lending program");
            return Err(LendingError::InvalidAccountOwner.into());
        }
        assert_rent_exempt($rent, $mi)?;
        assert_uninitialized::<MarketReserve>($mi)?;
    };
}

#[macro_export]
macro_rules! create_user_obligation {
    ($ui:ident; $iter:expr, $id:expr, $rent:expr) => {
        let $ui = next_account_info($iter)?;
        if $ui.owner != $id {
            msg!("User obligation provided is not owned by the lending program");
            return Err(LendingError::InvalidAccountOwner.into());
        }
        assert_rent_exempt($rent, $ui)?;
        assert_uninitialized::<UserObligation>($ui)?;
    };
}

#[macro_export]
#[cfg(feature = "unique-credit")]
macro_rules! create_unique_credit {
    ($ui:ident; $iter:expr, $id:expr, $rent:expr) => {
        let $ui = next_account_info($iter)?;
        if $ui.owner != $id {
            msg!("Unique credit provided is not owned by the lending program");
            return Err(LendingError::InvalidAccountOwner.into());
        }
        assert_rent_exempt($rent, $ui)?;
        assert_uninitialized::<UniqueCredit>($ui)?;
    };
}

#[macro_export]
macro_rules! get_manager {
    ($mi:ident, $m:ident; $iter:expr, $id:expr) => {
        let $mi = next_account_info($iter)?;
        if $mi.owner != $id {
            msg!("Manager provided is not owned by the lending program");
            return Err(LendingError::InvalidAccountOwner.into());
        }
        let $m = Manager::unpack(&$mi.try_borrow_data()?)?;
    };
}

#[macro_export]
macro_rules! get_mut_manager {
    ($mi:ident, $m:ident; $iter:expr, $id:expr) => {
        let $mi = next_account_info($iter)?;
        if $mi.owner != $id {
            msg!("Manager provided is not owned by the lending program");
            return Err(LendingError::InvalidAccountOwner.into());
        }
        let mut $m = Manager::unpack(&$mi.try_borrow_data()?)?;
    };
}

#[macro_export]
macro_rules! get_market_reserve {
    ($mi:ident, $mr:ident; $iter:expr, $id:expr) => {
        let $mi = next_account_info($iter)?;
        if $mi.owner != $id {
            msg!("Market reserve provided is not owned by the lending program");
            return Err(LendingError::InvalidAccountOwner.into());
        }
        let $mr = MarketReserve::unpack(&$mi.try_borrow_data()?)?;
    };
    ($mi:ident, $mr:ident; $iter:expr, $id:expr, $m:expr) => {
        get_market_reserve!($mi, $mr; $iter, $id);
        if &$mr.manager != $m {
            msg!("Manager of market reserve is not matched with manager provided");
            return Err(LendingError::UnmatchedAccounts.into());
        }
    };
    ($mi:ident, $mr:ident; $iter:expr, $id:expr, $m:expr, $clock:expr) => {
        get_market_reserve!($mi, $mr; $iter, $id, $m);
        if $mr.last_update.is_lax_stale($clock.slot)? {
            return Err(LendingError::MarketReserveStale.into());
        }
    };
}

#[macro_export]
macro_rules! get_sotoken_mint {
    ($smi:ident; $iter:expr, $mr:expr) => {
        let $smi = next_account_info($iter)?;
        if $smi.key != &$mr.collateral_info.sotoken_mint_pubkey {
            msg!("Sotoken mint of market reserve is not matched with sotoken mint acount provided");
            return Err(LendingError::UnmatchedAccounts.into())
        }
    };
}

#[macro_export]
macro_rules! get_supply_account {
    ($sma:ident; $iter:expr, $mr:expr) => {
        let $sma = next_account_info($iter)?;
        if $sma.key != &$mr.token_config.supply_account {
            msg!("Supply token account in market reserve is not matched with supply token account provided");
            return Err(LendingError::UnmatchedAccounts.into()); 
        }
    };
}

#[macro_export]
macro_rules! get_mut_market_reserve {
    ($mi:ident, $mr:ident; $iter:expr, $id:expr) => {
        let $mi = next_account_info($iter)?;
        if $mi.owner != $id {
            msg!("Market reserve provided is not owned by the lending program");
            return Err(LendingError::InvalidAccountOwner.into());
        }
        let mut $mr = MarketReserve::unpack(&$mi.try_borrow_data()?)?;
    };
    ($mi:ident, $mr:ident; $iter:expr, $id:expr, $m:expr) => {
        get_mut_market_reserve!($mi, $mr; $iter, $id);
        if &$mr.manager != $m {
            msg!("Manager of market reserve is not matched with manager provided");
            return Err(LendingError::UnmatchedAccounts.into());
        }
    };
    ($mi:ident, $mr:ident; $iter:expr, $id:expr, $m:expr, $clock:expr) => {
        get_mut_market_reserve!($mi, $mr; $iter, $id, $m);
        if $mr.last_update.is_lax_stale($clock.slot)? {
            return Err(LendingError::MarketReserveStale.into());
        }
    };
    ($mi:ident, $mr:ident; $iter:expr, $id:expr, $m:expr, $clock:expr, $wl:expr) => {
        get_mut_market_reserve!($mi, $mr; $iter, $id, $m);
        if $wl && $mr.last_update.is_lax_stale($clock.slot)? {
            return Err(LendingError::MarketReserveStale.into());
        }
    };
}

#[macro_export]
macro_rules! get_mut_user_obligation {
    ($ui:ident, $uo:ident; $iter:expr, $id:expr) => {
        let $ui = next_account_info($iter)?;
        if $ui.owner != $id {
            msg!("User obliagtion provided is not owned by the lending program");
            return Err(LendingError::InvalidAccountOwner.into());
        }
        let mut $uo = UserObligation::unpack(&$ui.try_borrow_data()?)?;
    };
    ($ui:ident, $uo:ident; $iter:expr, $id:expr, $m:expr) => {
        get_mut_user_obligation!($ui, $uo; $iter, $id);
        if &$uo.manager != $m {
            msg!("Manager of user obligation is not matched with manager provided");
            return Err(LendingError::UnmatchedAccounts.into());
        }
    };
    ($ui:ident, $uo:ident; $iter:expr, $id:expr, $m:expr, $clock:expr) => {
        get_mut_user_obligation!($ui, $uo; $iter, $id, $m);
        if $uo.last_update.is_lax_stale($clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }
    };
    ($ui:ident, $uo:ident; $iter:expr, $id:expr, $m:expr, $clock:expr, $wl:expr) => {
        get_mut_user_obligation!($ui, $uo; $iter, $id, $m);
        if $wl && $uo.last_update.is_lax_stale($clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }
    };
}

#[macro_export]
macro_rules! get_friend_obligation {
    ($fo:ident; $iter:expr, $uo:expr) => {
        let $fo = if let COption::Some(friend) = $uo.friend.as_ref() {
            let friend_obligation_info = next_account_info($iter)?;
            if friend_obligation_info.key != friend {
                msg!("Friend obligation provided is not matched with friend in user obligation");
                return Err(LendingError::UnmatchedAccounts.into());
            }
            let friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;
    
            Some(friend_obligation)
        } else {
            None
        };
    };
    ($fo:ident; $iter:expr, $uo:expr, $clock:expr) => {
        get_friend_obligation!($fo; $iter, $uo);
        let $fo = if let Some(friend_obligation) = $fo {
            if friend_obligation.last_update.is_lax_stale($clock.slot)? {
                return Err(LendingError::ObligationStale.into());
            }

            Some(friend_obligation)
        } else {
            None
        };
    };
    ($fo:ident; $iter:expr, $uo:expr, $clock:expr, $wl:expr) => {
        get_friend_obligation!($fo; $iter, $uo);
        let $fo = if let Some(friend_obligation) = $fo {
            if $wl && friend_obligation.last_update.is_lax_stale($clock.slot)? {
                return Err(LendingError::ObligationStale.into());
            }

            Some(friend_obligation)
        } else {
            None
        };
    };
}

#[macro_export]
#[cfg(feature = "unique-credit")]
macro_rules! get_unique_credit {
    ($ui:ident, $uc:ident; $iter:expr, $id:expr, $r:expr) => {
        let $ui = next_account_info($iter)?;
        if $ui.owner != $id {
            msg!("Unique credit provided is not owned by the lending program");
            return Err(LendingError::InvalidAccountOwner.into());
        }
        let mut $uc = UniqueCredit::unpack(&$ui.try_borrow_data()?)?;
        if &$uc.reserve != $r {
            msg!("Reserve of unique credit is not matched with market reserve provided");
            return Err(LendingError::UnmatchedAccounts.into());
        }
    };
}

#[macro_export]
macro_rules! get_manager_authority {
    ($ai:ident, $seeds:ident; $iter:expr, $id:expr, $m:expr, $mi:expr) => {
        let $seeds = &[
            $mi.key.as_ref(),
            &[$m.bump_seed],
        ];
        let $ai = next_account_info($iter)?;
        if $ai.key != &Pubkey::create_program_address($seeds, $id)? {
            msg!("Manager authority is not matched with program address derived from manager info");
            return Err(LendingError::UnmatchedAccounts.into());
        }
    };
}

#[macro_export]
macro_rules! get_signer {
    ($owner:ident; $iter:expr) => {
        let $owner = next_account_info($iter)?;
        if !$owner.is_signer {
            return Err(LendingError::InvalidAuthority.into());
        }
    };
}

#[macro_export]
macro_rules! get_manager_owner {
    ($owner:ident; $iter:expr, $m:expr) => {
        get_signer!($owner; $iter);
        if $owner.key != &$m.owner {
            msg!("Only manager owner can create market reserve");
            return Err(LendingError::UnmatchedAccounts.into());
        }
    };
}

#[macro_export]
macro_rules! get_user_obligation_owner {
    ($owner:ident; $iter:expr, $uo:expr) => {
        get_signer!($owner; $iter);
        if $owner.key != &$uo.owner {
            msg!("User authority provided is not matched with user obligation owner");
            return Err(LendingError::UnmatchedAccounts.into());
        }
    };
}

#[macro_export]
#[cfg(feature = "unique-credit")]
macro_rules! get_unique_credit_owner {
    ($owner:ident; $iter:expr, $uc:expr) => {
        get_signer!($owner; $iter);
        if $owner.key != &$uc.owner {
            msg!("User authority provided is not matched with unique credit owner");
            return Err(LendingError::UnmatchedAccounts.into());
        }
    };
}

#[macro_export]
macro_rules! get_receiver_program {
    ($receiver:ident; $iter:expr, $id:expr) => {
        let $receiver = next_account_info($iter)?;
        if $receiver.key == $id {
            msg!("Receiver program can not be lending program");
            return Err(ProgramError::IncorrectProgramId);
        }
    };
}