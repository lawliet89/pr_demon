extern crate hyper;
extern crate rustc_serialize;

use std::env;
use std::fs::File;
use std::io::Read;
use rustc_serialize::json;

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct BitbucketCredentials {
    username: String,
    password: String
}

fn main() {
    let config_path = match env::args().nth(1) {
        Some(x) => x,
        None => panic!("Usage ./app path_to_config.json")
    };
    let config = match read_config(&config_path) {
        Ok(x) => x,
        Err(err) => panic!(err)
    };
}

fn read_config(path: &str) -> Result<BitbucketCredentials, String> {
    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(err) => return Err(format!("Unable to read file because: {}", err))
    };

    let mut json = String::new();
    if let Err(err) = file.read_to_string(&mut json) {
        return Err(format!("Unable to read config: {}", err))
    }

    match json::decode(&json) {
        Ok(x) => Ok(x),
        Err(err) => return Err(format!("Unable to decode JSON value {}", err))
    }
}
