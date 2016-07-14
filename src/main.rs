extern crate hyper;
extern crate rustc_serialize;

mod bitbucket;
mod teamcity;

use std::env;
use std::fs::File;
use std::io::Read;
use rustc_serialize::json;
use hyper::client::Client;
use hyper::header::{Headers, Authorization, Basic, Accept, qitem};
use hyper::mime::{Mime, TopLevel, SubLevel, Attr, Value};


#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Config {
    teamcity: TeamcityCredentials,
    bitbucket: BitbucketCredentials
}

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

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct TeamcityCredentials {
    username: String,
    password: String,
    endpoint: String
}

impl UsernameAndPassword for TeamcityCredentials {
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

    let parsed_result = get_pr(&config.bitbucket);
    println!("{:#?}", parsed_result);

    let parsed_result = get_builds(&config.teamcity);
    println!("{:#?}", parsed_result);
}

fn read_config(path: &str) -> Result<Config, String> {
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

fn add_headers(headers: &mut Headers, credentials: &UsernameAndPassword) {
    headers.set(
       Authorization(
           Basic {
               username: credentials.username().clone(),
               password: Some(credentials.password().clone())
           }
       )
    );
    headers.set(
        Accept(vec![
            qitem(Mime(TopLevel::Application, SubLevel::Json,
                       vec![(Attr::Charset, Value::Utf8)])),
        ])
    );
}

fn get_pr(config: &BitbucketCredentials)
    -> Result<bitbucket::PagedApi<bitbucket::PullRequest>, String> {

    let mut headers = Headers::new();
    add_headers(&mut headers, config as &UsernameAndPassword);
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

fn get_builds(config: &TeamcityCredentials)
    -> Result<teamcity::Build, String> {
    let mut headers = Headers::new();
    add_headers(&mut headers, config as &UsernameAndPassword);
    let client = Client::new();
    let mut response = match client.get(&config.endpoint).headers(headers).send() {
        Ok(x) => x,
        Err(err) => return Err(format!("Unable to get list of Builds: {}", err))
    };

    let mut json_string = String::new();
    if let Err(err) = response.read_to_string(&mut json_string) {
        return Err(format!("Unable to get a list of Builds: {}", err))
    }

    match json::decode(&json_string) {
        Ok(x) => Ok(x),
        Err(err) =>  Err(format!("Error parsing response: {}", err))
    }

}

#[cfg(test)]
mod tests {
    use super::{Config, TeamcityCredentials, BitbucketCredentials, read_config};

    #[test]
    fn it_reads_and_parses_a_config_file() {
        let expected = Config {
            bitbucket: BitbucketCredentials {
                username: "username".to_owned(),
                password: "password".to_owned(),
                endpoint: "http://www.foobar.com/rest".to_owned()
            },
            teamcity: TeamcityCredentials {
                username: "username".to_owned(),
                password: "password".to_owned(),
                endpoint: "http://www.foobar.com/rest".to_owned()
            }
        };

        let actual = read_config("tests/fixtures/config.json").unwrap();

        assert_eq!(expected, actual);
    }
}
