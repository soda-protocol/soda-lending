use solana_sdk::signer::{Signer, keypair::Keypair};

const SODA: &str = "Soda666";

fn main() {
    loop {
        let keypair = &Keypair::new();
        let pubkey = keypair.pubkey();
        // let magic: [u8; 3] = pubkey.as_ref()[..3].try_into().unwrap();
        let magic = &pubkey.to_string()[..4];
        if magic == SODA {
            println!("keypair: {:?}, pubkey: {:?}", keypair.to_base58_string(), pubkey);
            break;
        }
    }
}
