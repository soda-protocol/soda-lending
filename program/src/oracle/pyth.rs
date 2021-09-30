#![allow(missing_docs)]
/// Derived from https://github.com/project-serum/anchor/blob/9224e0fa99093943a6190e396bccbc3387e5b230/examples/pyth/programs/pyth/src/pc.rs
use bytemuck::{
    cast_slice, from_bytes, try_cast_slice,
    Pod, PodCastError, Zeroable,
};
use std::{mem::size_of, convert::TryInto};
use solana_program::{
    msg,
    clock::Clock,
    program_error::ProgramError,
};
use crate::{
    math::{Decimal, TryMul, TryDiv},
    error::LendingError,
};

#[derive(Copy, Clone)]
#[repr(C)]
pub struct AccKey {
    pub val: [u8; 32],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum AccountType {
    Unknown,
    Mapping,
    Product,
    Price,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum PriceStatus {
    Unknown,
    Trading,
    Halted,
    Auction,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum CorpAction {
    NoCorpAct,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct PriceInfo {
    pub price: i64,
    pub conf: u64,
    pub status: PriceStatus,
    pub corp_act: CorpAction,
    pub pub_slot: u64,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct PriceComp {
    publisher: AccKey,
    agg: PriceInfo,
    latest: PriceInfo,
}

#[derive(PartialEq, Copy, Clone)]
#[repr(C)]
pub enum PriceType {
    Unknown,
    Price,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Price {
    pub magic: u32,       // pyth magic number
    pub ver: u32,         // program version
    pub atype: u32,       // account type
    pub size: u32,        // price account size
    pub ptype: PriceType, // price or calculation type
    pub expo: i32,        // price exponent
    pub num: u32,         // number of component prices
    pub unused: u32,
    pub curr_slot: u64,        // currently accumulating price slot
    pub valid_slot: u64,       // valid slot-time of agg. price
    pub twap: i64,             // time-weighted average price
    pub avol: u64,             // annualized price volatility
    pub drv0: i64,             // space for future derived values
    pub drv1: i64,             // space for future derived values
    pub drv2: i64,             // space for future derived values
    pub drv3: i64,             // space for future derived values
    pub drv4: i64,             // space for future derived values
    pub drv5: i64,             // space for future derived values
    pub prod: AccKey,          // product account key
    pub next: AccKey,          // next Price account in linked list
    pub agg_pub: AccKey,       // quoter who computed last aggregate price
    pub agg: PriceInfo,        // aggregate price info
    pub comp: [PriceComp; 32], // price components one per quoter
}

#[cfg(target_endian = "little")]
unsafe impl Zeroable for Price {}

#[cfg(target_endian = "little")]
unsafe impl Pod for Price {}

fn load<T: Pod>(data: &[u8]) -> Result<&T, PodCastError> {
    let size = size_of::<T>();
    Ok(from_bytes(cast_slice::<u8, u8>(try_cast_slice(
        &data[0..size],
    )?)))
}

pub fn get_pyth_price(data: &[u8], clock: &Clock) -> Result<Decimal, ProgramError> {
    #[cfg(not(feature = "devnet"))]
    const STALE_AFTER_SLOTS_ELAPSED: u64 = 10;
    #[cfg(feature = "devnet")]
    const STALE_AFTER_SLOTS_ELAPSED: u64 = 20;

    let pyth_price = load::<Price>(data).map_err(|_| ProgramError::InvalidAccountData)?;
    if pyth_price.ptype != PriceType::Price {
        msg!("Pyth oracle price type is invalid");
        return Err(LendingError::InvalidPriceOracle.into());
    }

    let slots_elapsed = clock.slot
        .checked_sub(pyth_price.valid_slot)
        .ok_or(LendingError::MathOverflow)?;
    if slots_elapsed >= STALE_AFTER_SLOTS_ELAPSED {
        msg!("Pyth oracle price is stale");
        return Err(LendingError::InvalidPriceOracle.into());
    }

    let price: u64 = pyth_price.agg.price.try_into().map_err(|_| {
        msg!("Pyth oracle price cannot be negative");
        LendingError::InvalidPriceOracle
    })?;

    if pyth_price.expo >= 0 {
        let exponent = pyth_price.expo
            .try_into()
            .map_err(|_| LendingError::MathOverflow)?;
        let zeros = 10u64
            .checked_pow(exponent)
            .ok_or(LendingError::MathOverflow)?;
        Decimal::from(price).try_mul(zeros)
    } else {
        let exponent = pyth_price.expo
            .checked_abs()
            .ok_or(LendingError::MathOverflow)?
            .try_into()
            .map_err(|_| LendingError::MathOverflow)?;
        let decimals = 10u64
            .checked_pow(exponent)
            .ok_or(LendingError::MathOverflow)?;
        Decimal::from(price).try_div(decimals)
    }
}