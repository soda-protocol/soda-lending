#![allow(clippy::assign_op_pattern)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::manual_range_contains)]

use super::*;
use crate::error::SodaError;
use std::convert::TryFrom;
use uint::construct_uint;

// U192 with 192 bits consisting of 3 x 64-bit words
construct_uint! {
    pub struct U192(3);
}

/// Large decimal values, precise to 18 digits
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub struct Decimal(pub U192);

impl Decimal {
    /// One
    pub fn one() -> Self {
        Self(Self::wad())
    }

    /// Zero
    pub fn zero() -> Self {
        Self(U192::zero())
    }

    // OPTIMIZE: use const slice when fixed in BPF toolchain
    fn wad() -> U192 {
        U192::from(WAD)
    }

    // OPTIMIZE: use const slice when fixed in BPF toolchain
    fn half_wad() -> U192 {
        U192::from(HALF_WAD)
    }

    /// Create scaled decimal from percent value
    pub fn from_percent(percent: u8) -> Self {
        Self(U192::from(percent as u64 * PERCENT_SCALER))
    }

    /// Return raw scaled value if it fits within u128
    #[allow(clippy::wrong_self_convention)]
    pub fn to_scaled_val(&self) -> Result<u128, SodaError> {
        Ok(u128::try_from(self.0).map_err(|_| SodaError::MathOverflow)?)
    }

    /// Create decimal from scaled value
    pub fn from_scaled_val(scaled_val: u128) -> Self {
        Self(U192::from(scaled_val))
    }

    /// Round scaled decimal to u64
    pub fn try_round_u64(&self) -> Result<u64, SodaError> {
        let rounded_val = Self::half_wad()
            .checked_add(self.0)
            .ok_or(SodaError::MathOverflow)?
            .checked_div(Self::wad())
            .ok_or(SodaError::MathOverflow)?;
        Ok(u64::try_from(rounded_val).map_err(|_| SodaError::MathOverflow)?)
    }

    /// Ceiling scaled decimal to u64
    pub fn try_ceil_u64(&self) -> Result<u64, SodaError> {
        let ceil_val = Self::wad()
            .checked_sub(U192::from(1u64))
            .ok_or(SodaError::MathOverflow)?
            .checked_add(self.0)
            .ok_or(SodaError::MathOverflow)?
            .checked_div(Self::wad())
            .ok_or(SodaError::MathOverflow)?;
        Ok(u64::try_from(ceil_val).map_err(|_| SodaError::MathOverflow)?)
    }

    /// Floor scaled decimal to u64
    pub fn try_floor_u64(&self) -> Result<u64, SodaError> {
        let ceil_val = self
            .0
            .checked_div(Self::wad())
            .ok_or(SodaError::MathOverflow)?;
        Ok(u64::try_from(ceil_val).map_err(|_| SodaError::MathOverflow)?)
    }
}

impl From<u64> for Decimal {
    fn from(val: u64) -> Self {
        Self(Self::wad() * U192::from(val))
    }
}

impl From<u128> for Decimal {
    fn from(val: u128) -> Self {
        Self(Self::wad() * U192::from(val))
    }
}

impl TryAdd for Decimal {
    fn try_add(self, rhs: Self) -> Result<Self, SodaError> {
        Ok(Self(
            self.0
                .checked_add(rhs.0)
                .ok_or(SodaError::MathOverflow)?,
        ))
    }
}

impl TrySub for Decimal {
    fn try_sub(self, rhs: Self) -> Result<Self, SodaError> {
        Ok(Self(
            self.0
                .checked_sub(rhs.0)
                .ok_or(SodaError::MathOverflow)?,
        ))
    }
}

impl TryDiv<u64> for Decimal {
    fn try_div(self, rhs: u64) -> Result<Self, SodaError> {
        Ok(Self(
            self.0
                .checked_div(U192::from(rhs))
                .ok_or(SodaError::MathOverflow)?,
        ))
    }
}

impl TryDiv<Decimal> for Decimal {
    fn try_div(self, rhs: Self) -> Result<Self, SodaError> {
        Ok(Self(
            self.0
                .checked_mul(Self::wad())
                .ok_or(SodaError::MathOverflow)?
                .checked_div(rhs.0)
                .ok_or(SodaError::MathOverflow)?,
        ))
    }
}

impl TryMul<u64> for Decimal {
    fn try_mul(self, rhs: u64) -> Result<Self, SodaError> {
        Ok(Self(
            self.0
                .checked_mul(U192::from(rhs))
                .ok_or(SodaError::MathOverflow)?,
        ))
    }
}

impl TryMul<Decimal> for Decimal {
    fn try_mul(self, rhs: Self) -> Result<Self, SodaError> {
        Ok(Self(
            self.0
                .checked_mul(rhs.0)
                .ok_or(SodaError::MathOverflow)?
                .checked_div(Self::wad())
                .ok_or(SodaError::MathOverflow)?,
        ))
    }
}