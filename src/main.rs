extern crate hyper;
extern crate rustc_serialize;

mod bitbucket;
mod teamcity;

use std::env;
use std::fs::File;
use std::io::Read;
use std::iter;
use std::collections::BTreeMap;
use rustc_serialize::json;
use hyper::client::Client;
use hyper::header::{Headers, Authorization, Basic, Accept, qitem, ContentType};
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
    endpoints: BTreeMap<String, String>,
    build_id: String
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
        None => panic!("Usage ./pr_demon path_to_config.json")
    };
    let config = match read_config(&config_path) {
        Ok(x) => x,
        Err(err) => panic!(err)
    };

    let sleep_duration = std::time::Duration::new(5, 0);

    loop {
        let pull_requests = match get_pr(&config.bitbucket) {
            Err(err) => {
                println!("Error getting Pull Requests: {}", err);
                continue;
            },
            Ok(x) => x
        };

        println!("{} Open Pull Requests Found", pull_requests.size);

        for pr in &pull_requests.values {
            println!("{}Pull Request #{} ({})", tabs(1), pr.id, pr.links["self"][0].href);
            let git_ref = &pr.fromRef.id;
            let branch_name = git_ref.split('/').next_back().unwrap();
            let pr_commit = &pr.fromRef.latestCommit;
            println!("{}Branch: {}", tabs(2), branch_name);
            println!("{}Commit: {}", tabs(2), pr_commit);
            println!("{}Finding latest build from branch", tabs(2));

            let mut run_build = false;
            let builds = get_builds(&config.teamcity, &branch_name);

            match builds {
                Ok(ref build) => {
                    match build.revisions.revision.as_ref() {
                        Some(ref x) => {
                            if let Some(y) = x.first() {
                                if &y.version == pr_commit {
                                    println!("{}Commit matches -- skipping", tabs(2));
                                } else {
                                    println!("{}Commit does not match -- scheduling build", tabs(2));
                                    run_build = true;
                                }
                            }
                        },
                        None if build.state == teamcity::BuildState::queued => {
                            println!("{}Build is queued -- skipping", tabs(2));
                        },
                        _ => {
                            println!("{}Unknown error -- scheduling build", tabs(2));
                            run_build = true;
                        }
                    };
                },
                Err(ref err) if err == "404 Not Found" => {
                    println!("{}Build does not exist -- running build", tabs(2));
                    run_build = true;
                },
                Err(e @ _) => {
                    println!("{} Error fetching builds -- queuing anyway: {}", tabs(2), e);
                    run_build = true;
                }
            };

            if run_build {
                println!("{}Scheduling build", tabs(2));
                let queued_build = queue_build(&config.teamcity, &branch_name);
                match queued_build {
                    Err(err) => {
                        println!("{}Error queuing build: {}", tabs(2), err);
                        continue;
                    },
                    Ok(queued) => {
                        println!("{}Build Queued: {}", tabs(2), queued.webUrl)
                    }
                }
            } else {
                println!("{}Build exists", tabs(2));
            }
        }
        std::thread::sleep(sleep_duration);
    }
}

fn tabs(x: usize) -> String {
    // https://stackoverflow.com/questions/31216646/repeat-string-with-integer-multiplication
    iter::repeat("    ").take(x).collect()
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

fn add_authorization_header(headers: &mut Headers, credentials: &UsernameAndPassword) {
    headers.set(
       Authorization(
           Basic {
               username: credentials.username().clone(),
               password: Some(credentials.password().clone())
           }
       )
    );
}

fn add_accept_json_header(headers: &mut Headers) {
    headers.set(
        Accept(vec![
            qitem(Mime(TopLevel::Application, SubLevel::Json,
                       vec![(Attr::Charset, Value::Utf8)])),
        ])
    );
}

fn add_content_type_xml_header(headers: &mut Headers) {
    headers.set(
        ContentType(Mime(TopLevel::Application, SubLevel::Xml,
                         vec![(Attr::Charset, Value::Utf8)]))
    );
}

fn get_pr(config: &BitbucketCredentials)
    -> Result<bitbucket::PagedApi<bitbucket::PullRequest>, String> {

    let mut headers = Headers::new();
    add_authorization_header(&mut headers, config as &UsernameAndPassword);
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

fn get_builds(config: &TeamcityCredentials, branch: &str)
    -> Result<teamcity::Build, String> {
    let mut headers = Headers::new();
    add_authorization_header(&mut headers, config as &UsernameAndPassword);
    add_accept_json_header(&mut headers);
    let client = Client::new();
    // FIXME: Format a proper URL instead
    let mut response = match client
            .get(&(format!("{}{}", config.endpoints["builds.list"], branch)))
            .headers(headers).send() {
        Ok(x) => x,
        Err(err) => return Err(format!("Unable to get list of Builds: {}", err))
    };

    match response.status {
        hyper::status::StatusCode::Ok => (),
        e @ _ => return Err(e.to_string())
    };

    let mut json_string = String::new();
    if let Err(err) = response.read_to_string(&mut json_string) {
        return Err(format!("Unable to get a list of Builds: {}", err))
    }

    match json::decode(&json_string) {
        Ok(x) => Ok(x),
        Err(err) =>  Err(format!("Error parsing response: {} {}", json_string, err))
    }
}

fn queue_build(config: &TeamcityCredentials, branch: &str)
    -> Result<teamcity::Build, String> {
    let mut headers = Headers::new();
    add_authorization_header(&mut headers, config as &UsernameAndPassword);
    add_accept_json_header(&mut headers);
    add_content_type_xml_header(&mut headers);

    let client = Client::new();
    // FIXME: Format a proper template instead!
    let body = format!("<build branchName=\"{}\">
                      <buildType id=\"{}\"/>
                      <comment><text>Triggered by PR Demon</text></comment>
                    </build>", branch, config.build_id);
    let mut response = match client
            .post(&config.endpoints["build.queue"])
            .body(&body)
            .headers(headers).send() {
        Ok(x) => x,
        Err(err) => return Err(format!("Unable to schedule build: {}", err))
    };

    match response.status {
        hyper::status::StatusCode::Ok => (),
        e @ _ => return Err(e.to_string())
    };

    let mut json_string = String::new();
    if let Err(err) = response.read_to_string(&mut json_string) {
        return Err(format!("Unable to schedule build: {}", err))
    }

    match json::decode(&json_string) {
        Ok(x) => Ok(x),
        Err(err) =>  Err(format!("Error parsing response: {} {}", json_string, err))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use super::{Config, TeamcityCredentials, BitbucketCredentials, read_config};

    #[test]
    fn it_reads_and_parses_a_config_file() {
        let mut expected_teamcity_endpoints = BTreeMap::<String, String>::new();
        expected_teamcity_endpoints.insert("builds.list".to_owned(),
                    "http://www.foobar.com/rest".to_owned());
        expected_teamcity_endpoints.insert("build.queue".to_owned(),
                    "http://www.foobar.com/rest/queue".to_owned());

        let expected = Config {
            bitbucket: BitbucketCredentials {
                username: "username".to_owned(),
                password: "password".to_owned(),
                endpoint: "http://www.foobar.com/rest".to_owned()
            },
            teamcity: TeamcityCredentials {
                username: "username".to_owned(),
                password: "password".to_owned(),
                build_id: "foobar".to_owned(),
                endpoints: expected_teamcity_endpoints.to_owned()
            }
        };

        let actual = read_config("tests/fixtures/config.json").unwrap();

        assert_eq!(expected, actual);
    }
}
