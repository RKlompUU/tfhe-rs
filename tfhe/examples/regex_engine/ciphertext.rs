use anyhow::{anyhow, Result};
use tfhe::integer::{gen_keys_radix, RadixCiphertextBig, RadixClientKey, ServerKey};
use tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2;

pub type StringCiphertext = Vec<RadixCiphertextBig>;

pub fn encrypt_str(client_key: &RadixClientKey, s: &str) -> Result<StringCiphertext> {
    if !s.is_ascii() {
        return Err(anyhow!("content contains non-ascii characters"));
    }
    Ok(s.as_bytes()
        .iter()
        .map(|byte| client_key.encrypt(*byte as u64))
        .collect())
}

pub fn gen_keys() -> (RadixClientKey, ServerKey) {
    let num_block = 4;
    gen_keys_radix(PARAM_MESSAGE_2_CARRY_2, num_block)
}
