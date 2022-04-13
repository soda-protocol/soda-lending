use solana_program::clock::Slot;

use crate::math::Decimal;



pub struct DexOracle {
    pub last_slot: Slot,
    pub price: Option<Decimal>,
}