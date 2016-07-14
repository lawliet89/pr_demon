extern crate hyper;
extern crate rustc_serialize;

mod bitbucket;

use std::env;
use std::fs::File;
use std::io::Read;
use rustc_serialize::json;
use hyper::client::Client;
use hyper::header::{Headers, Authorization, Basic};

trait UsernameAndPassword {
    fn username(&self) -> &String;
    fn password(&self) -> &String;
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct BitbucketCredentials {
    username: String,
    password: String,
    endpoint: String
}

impl UsernameAndPassword for BitbucketCredentials {
    fn username(&self) -> &String {
        &self.username
    }

    fn password(&self) -> &String {
        &self.password
    }
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

    let parsed_result = get_pr(&config);
    println!("{:#?}", parsed_result);
}

fn read_config(path: &str) -> Result<BitbucketCredentials, String> {
    let mut file = match File::open(path) {
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

fn add_authorization_headers(headers: &mut Headers, credentials: &UsernameAndPassword) {
    headers.set(
       Authorization(
           Basic {
               username: credentials.username().clone(),
               password: Some(credentials.password().clone())
           }
       )
    );
}

fn get_pr(config: &BitbucketCredentials)
    -> Result<bitbucket::PagedApi<bitbucket::PullRequest>, String> {

    let mut headers = Headers::new();
    add_authorization_headers(&mut headers, config as &UsernameAndPassword);
    let client = Client::new();
    let mut response = match client.get(&config.endpoint).headers(headers).send() {
        Ok(x) => x,
        Err(err) => return Err(format!("Unable to get list of PR: {}", err))
    };

    let mut json_string = String::new();
    if let Err(err) = response.read_to_string(&mut json_string) {
        return Err(format!("Unable to get a list of PR: {}", err))
    }

    match json::decode(&json_string) {
        Ok(x) => Ok(x),
        Err(err) =>  Err(format!("Error parsing response: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::{BitbucketCredentials, read_config};

    #[test]
    fn it_reads_and_parses_a_config_file() {
        let expected = BitbucketCredentials {
            username: "username".to_owned(),
            password: "password".to_owned()
        };

        let actual = read_config("test/fixtures/config.json").unwrap();

        assert_eq!(expected, actual);
    }
}
