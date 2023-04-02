pub mod ciphertext;
pub mod engine;
pub mod parser;
pub mod execution;

use crate::regex::ciphertext::{gen_keys, encrypt_str};
use crate::regex::engine::has_match;

// a simple example application
pub(crate) fn main(content: &str, pattern: &str) {
    let (client_key, server_key) = gen_keys();

    println!("encrypting content..");
    let ct_content = encrypt_str(&client_key, content);

    println!("applying regex..");
    let ct_res = has_match(&server_key, &ct_content.unwrap(), pattern).unwrap();
    let res: u64 = client_key.decrypt(&ct_res);
    println!("res: {:?}", res);
}
