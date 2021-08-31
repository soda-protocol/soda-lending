#![allow(missing_docs)]

mod pack;
mod pubkey;
mod reserve;
mod last_update;

pub use pack::*;
pub use pubkey::*;
pub use reserve::*;
pub use last_update::*;

use crate::{error::SodaError, math::Decimal};

const UNINITIALIZED_VERSION: u8 = 0;

const PROGRAM_VERSION: u8 = 0;

fn pack_decimal(decimal: Decimal, dst: &mut [u8; 16]) {
    *dst = decimal
        .to_scaled_val()
        .expect("Decimal cannot be packed")
        .to_le_bytes();
}

fn unpack_decimal(src: &[u8; 16]) -> Decimal {
    Decimal::from_scaled_val(u128::from_le_bytes(*src))
}

fn pack_bool(boolean: bool, dst: &mut [u8; 1]) {
    *dst = (boolean as u8).to_le_bytes()
}

fn unpack_bool(src: &[u8; 1]) -> Result<bool, SodaError> {
    match u8::from_le_bytes(*src) {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(SodaError::UnpackError),
    }
}