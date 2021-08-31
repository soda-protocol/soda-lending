use std::{fmt, convert::TryFrom, str::FromStr, mem};
use bs58;

use crate::error::SodaError;

const MAX_BASE58_LEN: usize = 44;
pub const PUBKEY_BYTES: usize = 32;

#[derive(Clone, Default, PartialEq)]
pub struct Pubkey([u8; 32]);

impl Pubkey {
    pub fn new(pubkey_vec: &[u8]) -> Self {
        Self(
            <[u8; 32]>::try_from(<&[u8]>::clone(&pubkey_vec))
                .expect("Slice must be the same length as a Pubkey"),
        )
    }

    pub const fn new_from_array(pubkey_array: [u8; 32]) -> Self {
        Self(pubkey_array)
    }
}

impl FromStr for Pubkey {
    type Err = SodaError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() > MAX_BASE58_LEN {
            return Err(SodaError::InvalidPubkey);
        }
        let pubkey_vec = bs58::decode(s)
            .into_vec()
            .map_err(|_| SodaError::InvalidPubkey)?;
        if pubkey_vec.len() != mem::size_of::<Pubkey>() {
            Err(SodaError::InvalidPubkey)
        } else {
            Ok(Pubkey::new(&pubkey_vec))
        }
    }
}

impl AsRef<[u8]> for Pubkey {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl AsMut<[u8]> for Pubkey {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0[..]
    }
}

impl fmt::Debug for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", bs58::encode(self.0).into_string())
    }
}

impl fmt::Display for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", bs58::encode(self.0).into_string())
    }
}