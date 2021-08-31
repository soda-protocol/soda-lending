use wasm_bindgen::prelude::*;
use js_sys::Uint8Array;
use soda_lending_contract::{solana_program::program_pack::Pack, state::MarketReserve};

// #[wasm_bindgen]
// extern "C" {
//     #[wasm_bindgen(js_namespace = console)]
//     fn log(s: &str);
//     #[wasm_bindgen(js_namespace = console)]
//     fn error(s: &str);
// }

#[wasm_bindgen]
pub fn test_for_market_reserve(
    reserve_array: Uint8Array,
) -> JsValue {
    let market_reserve_data = reserve_array.to_vec();

    match MarketReserve::unpack(&market_reserve_data) {
        Ok(market_reserve) => {
            JsValue::from_f64(market_reserve.liquidity_info.available as f64)
        }
        Err(e) => {
            JsValue::from_str(e.to_string().as_str())
        }
    }
}

// #[wasm_bindgen]
// pub fn parse_obligation(
//     clock_array: Uint8Array,
//     obligation_array: Uint8Array,
//     reserve_map: Map,
//     price_oracle_map: Map,
//     rate_oracle_map: Map,
// ) -> JSON {
//     let clock_data = clock_array.to_vec();
//     let obligation_data = obligation_array.to_vec();


// }
