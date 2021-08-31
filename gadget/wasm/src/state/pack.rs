use crate::error::SodaError;

pub trait IsInitialized {
    /// Is initialized
    fn is_initialized(&self) -> bool;
}

/// Implementors must have a known size
pub trait Sealed: Sized {}

/// Safely and efficiently (de)serialize account state
pub trait Pack: Sealed {
    /// The length, in bytes, of the packed representation
    const LEN: usize;
    #[doc(hidden)]
    fn pack_into_slice(&self, dst: &mut [u8]);
    #[doc(hidden)]
    fn unpack_from_slice(src: &[u8]) -> Result<Self, SodaError>;

    /// Get the packed length
    fn get_packed_len() -> usize {
        Self::LEN
    }

    /// Unpack from slice and check if initialized
    fn unpack(input: &[u8]) -> Result<Self, SodaError>
    where
        Self: IsInitialized,
    {
        let value = Self::unpack_unchecked(input)?;
        if value.is_initialized() {
            Ok(value)
        } else {
            Err(SodaError::UnpackError)
        }
    }

    /// Unpack from slice without checking if initialized
    fn unpack_unchecked(input: &[u8]) -> Result<Self, SodaError> {
        if input.len() != Self::LEN {
            return Err(SodaError::UnpackError);
        }
        Self::unpack_from_slice(input)
    }

    /// Pack into slice
    fn pack(src: Self, dst: &mut [u8]) -> Result<(), SodaError> {
        if dst.len() != Self::LEN {
            return Err(SodaError::PackError);
        }
        src.pack_into_slice(dst);
        Ok(())
    }
}