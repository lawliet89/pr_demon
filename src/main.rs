extern crate hyper;
extern crate rustc_serialize;
extern crate url;

mod bitbucket;
mod teamcity;

use std::env;
use std::fs::File;
use std::io::Read;
use std::iter;
use rustc_serialize::json;
use hyper::client::Client;
use hyper::header::{Headers, Authorization, Basic, Accept, qitem, ContentType};
use hyper::mime::{Mime, TopLevel, SubLevel, Attr, Value};
use url::percent_encoding::{utf8_percent_encode, QUERY_ENCODE_SET};

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
    base_url: String,
    project_slug: String,
    repo_slug: String
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
    base_url: String,
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
            let branch_name: String = git_ref.split('/').skip(2).collect::<Vec<_>>().join("/");
            let pr_commit = &pr.fromRef.latestCommit;
            println!("{}Branch: {}", tabs(2), branch_name);
            println!("{}Commit: {}", tabs(2), pr_commit);
            println!("{}Finding latest build from branch", tabs(2));

            let latest_build = match get_build_list(&config.teamcity, &branch_name) {
                Ok(ref build_list) => {
                    match build_list.build {
                        Some(ref builds) => {
                            let latest_build_id = builds.first().unwrap().id;
                            match get_build(&config.teamcity, latest_build_id) {
                                Ok(x) =>  {
                                    println!("{}Latest Build Found {}", tabs(2), x.webUrl);
                                    Some(x)
                                },
                                Err(err) => {
                                    println!("{}Unable to retrieve information for build ID {}: {}", tabs(2), latest_build_id, err);
                                    None
                                }
                            }
                        },
                        None => {
                            println!("{}Build does not exist -- running build", tabs(2));
                            None
                        }
                    }
                },
                Err(err) => {
                    println!("{}Error fetching builds -- queuing anyway: {}", tabs(2), err);
                    None
                }
            };

            let build_found = match latest_build {
                None => None,
                Some(ref build) => {
                    match build.revisions.revision.as_ref() {
                        Some(ref x) => {
                            if let Some(y) = x.first() {
                                if &y.version == pr_commit {
                                    println!("{}Commit matches -- skipping", tabs(2));
                                    Some(build.to_owned())
                                } else {
                                    println!("{}Commit does not match with {} -- scheduling build", tabs(2), y.version);
                                    None
                                }
                            } else {
                                None
                            }
                        },
                        None if build.state == teamcity::BuildState::queued => {
                            println!("{}Build is queued -- skipping", tabs(2));
                            Some(build.to_owned())
                        },
                        _ => {
                            println!("{}Unknown error -- scheduling build", tabs(2));
                            None
                        }
                    }
                }
            };

            match build_found {
                None => {
                    println!("{}Scheduling build", tabs(2));
                    let queued_build = queue_build(&config.teamcity, &branch_name);
                    match queued_build {
                        Err(err) => {
                            println!("{}Error queuing build: {}", tabs(2), err);
                            continue;
                        },
                        Ok(queued) => {
                            println!("{}Build Queued: {}", tabs(2), queued.webUrl);
                            match post_queued_comment(&queued.webUrl, pr_commit, pr.id, &config.bitbucket) {
                                Ok(_) => {},
                                Err(err) => println!("{}Error submitting comment: {}", tabs(2), err)
                            };
                        }
                    }
                },
                Some(build) => {
                    println!("{}Build exists: {}", tabs(2), build.webUrl);
                    match build.status {
                        None => {},
                        Some(status) => {
                            match status {
                                teamcity::BuildStatus::SUCCESS => {
                                    match post_success_comment(&build.webUrl, pr_commit, pr.id, &config.bitbucket) {
                                        Ok(_) => {},
                                        Err(err) => println!("{}Error submitting comment: {}", tabs(2), err)
                                    };
                                },
                                teamcity::BuildStatus::FAILURE | teamcity::BuildStatus::UNKNOWN => {
                                    let status_text = match build.statusText {
                                        None => "".to_owned(),
                                        Some(x) => x.to_owned()
                                    };
                                    match post_failure_comment(&build.webUrl, pr_commit, &status_text, pr.id, &config.bitbucket) {
                                        Ok(_) => {},
                                        Err(err) => println!("{}Error submitting comment: {}", tabs(2), err)
                                    };
                                }
                            };
                        }
                    }
                }
            };
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

fn add_content_type_json_header(headers: &mut Headers) {
    headers.set(
        ContentType(Mime(TopLevel::Application, SubLevel::Json,
                         vec![(Attr::Charset, Value::Utf8)]))
    );
}

fn get_pr(config: &BitbucketCredentials)
    -> Result<bitbucket::PagedApi<bitbucket::PullRequest>, String> {

    let mut headers = Headers::new();
    add_authorization_header(&mut headers, config as &UsernameAndPassword);
    let client = Client::new();
    let url = format!("{}/projects/{}/repos/{}/pull-requests",
        config.base_url, config.project_slug, config.repo_slug);
    let mut response = match client.get(&url).headers(headers).send() {
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

fn get_build_list(config: &TeamcityCredentials, branch: &str)
        -> Result<teamcity::BuildList, String> {

    let mut headers = Headers::new();
    add_authorization_header(&mut headers, config as &UsernameAndPassword);
    add_accept_json_header(&mut headers);
    let client = Client::new();

    let encoded_branch = utf8_percent_encode(branch, QUERY_ENCODE_SET).collect::<String>();
    let query_string = format!("state:any,branch:(name:{})", encoded_branch);
    let url = format!("{}/buildTypes/id:{}/builds?locator={}", config.base_url, config.build_id, query_string);
    let mut response = match client
            .get(&url)
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

fn get_build(config: &TeamcityCredentials, build_id: i32) -> Result<teamcity::Build, String> {
        let mut headers = Headers::new();
        add_authorization_header(&mut headers, config as &UsernameAndPassword);
        add_accept_json_header(&mut headers);
        let client = Client::new();

        let url = format!("{}/builds/id:{}", config.base_url, build_id);
        let mut response = match client
                .get(&url)
                .headers(headers).send() {
            Ok(x) => x,
            Err(err) => return Err(format!("Unable to retrieve build: {}", err))
        };

        match response.status {
            hyper::status::StatusCode::Ok => (),
            e @ _ => return Err(e.to_string())
        };

        let mut json_string = String::new();
        if let Err(err) = response.read_to_string(&mut json_string) {
            return Err(format!("Unable to retrieve build: {}", err))
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
    let url = format!("{}/buildQueue", config.base_url);
    let mut response = match client
            .post(&url)
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

fn get_comment(pr_id: i32, config: &BitbucketCredentials)
        -> Result<bitbucket::PagedApi<bitbucket::Activity>, String> {
    let mut headers = Headers::new();
    add_authorization_header(&mut headers, config as &UsernameAndPassword);
    add_accept_json_header(&mut headers);

    let client = Client::new();
    let url = format!("{}/projects/{}/repos/{}/pull-requests/{}/activities?fromType=COMMENT",
            config.base_url, config.project_slug, config.repo_slug, pr_id);
    let mut response = match client
            .get(&url)
            .headers(headers).send() {
        Ok(x) => x,
        Err(err) => return Err(format!("Unable to retrieve comments: {}", err))
    };

    match response.status {
        hyper::status::StatusCode::Ok => (),
        e @ _ => return Err(e.to_string())
    };

    let mut json_string = String::new();
    if let Err(err) = response.read_to_string(&mut json_string) {
        return Err(format!("Unable to retrieve comments: {}", err))
    }

    match json::decode(&json_string) {
        Ok(x) => Ok(x),
        Err(err) =>  Err(format!("Error parsing response: {} {}", json_string, err))
    }
}

fn post_comment(comment: &str, pr_id: i32, config: &BitbucketCredentials)
        -> Result<bitbucket::Comment, String> {
    match get_comment(pr_id, &config) {
        Ok(ref activities) => {
            let activity = activities.values.iter()
                .filter(|&activity| activity.comment.is_some())
                .find(|&activity| activity.comment.as_ref().unwrap().text == comment);

            match activity {
                None => {},
                Some(matching_activity) => {
                    return Ok(matching_activity.clone().comment.unwrap().to_owned())
                }
            };
        },
        Err(err) => { println!("Error getting list of comments {}", err); }
    };

    let mut headers = Headers::new();
    add_authorization_header(&mut headers, config as &UsernameAndPassword);
    add_accept_json_header(&mut headers);
    add_content_type_json_header(&mut headers);

    let client = Client::new();
    // FIXME: Format a proper template instead!
    let body = json::encode(&bitbucket::CommentSubmit {
        text: comment.to_owned()
    }).unwrap();
    let url = format!("{}/projects/{}/repos/{}/pull-requests/{}/comments",
            config.base_url, config.project_slug, config.repo_slug, pr_id);
    let mut response = match client
            .post(&url)
            .body(&body)
            .headers(headers).send() {
        Ok(x) => x,
        Err(err) => return Err(format!("Unable to submit comment: {}", err))
    };

    match response.status {
        hyper::status::StatusCode::Created => (),
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

fn post_queued_comment(build_url: &str, commit_id: &str, pr_id: i32, config: &BitbucketCredentials)
        -> Result<bitbucket::Comment, String> {
    let comment = format!("⏳ [Build]({}) for commit {} queued", build_url, commit_id);
    post_comment(&comment, pr_id, config)
}

fn post_success_comment(build_url: &str, commit_id: &str, pr_id: i32, config: &BitbucketCredentials)
        -> Result<bitbucket::Comment, String> {
    let comment = format!("✔️ [Build]({}) for commit {} is **successful**", build_url, commit_id);
    post_comment(&comment, pr_id, config)
}

fn post_failure_comment(build_url: &str, commit_id: &str, build_message: &str, pr_id: i32, config: &BitbucketCredentials)
        -> Result<bitbucket::Comment, String> {
    let comment = format!("❌ [Build]({}) for commit {} has **failed**: {}", build_url, commit_id, build_message);
    post_comment(&comment, pr_id, config)
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
                base_url: "https://www.example.com/bb/rest/api/latest".to_owned(),
                project_slug: "foo".to_owned(),
                repo_slug: "bar".to_owned()
            },
            teamcity: TeamcityCredentials {
                username: "username".to_owned(),
                password: "password".to_owned(),
                build_id: "foobar".to_owned(),
                base_url: "https://www.foobar.com/rest".to_owned()
            }
        };

        let actual = read_config("tests/fixtures/config.json").unwrap();

        assert_eq!(expected, actual);
    }
}
