extern crate hyper;
extern crate rustc_serialize;
extern crate url;
extern crate time;
extern crate timebomb;

mod rest;
mod fanout;
mod bitbucket;
mod teamcity;

use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::iter;
use std::boxed::Box;
use std::thread;
use rustc_serialize::json;
use fanout::{Fanout, Message, OpCode};

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Config { // TODO: Rename fields
    teamcity: teamcity::TeamcityCredentials,
    bitbucket: bitbucket::BitbucketCredentials,
    run_interval: u64
}

pub trait UsernameAndPassword {
    fn username(&self) -> &String;
    fn password(&self) -> &String;
}

#[derive(RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct PullRequest {
    pub id: i32,
    pub web_url: String,
    pub from_ref: String,
    pub from_commit: String
}

impl PullRequest {
    fn branch_name(&self) -> String {
        let git_ref = &self.from_ref;
        git_ref.split('/').skip(2).collect::<Vec<_>>().join("/")
    }
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Comment {
    pub id: i32,
    pub text: String
}

pub trait Repository {
    fn get_pr_list(&self) -> Result<Vec<PullRequest>, String>;
    fn build_queued(&self, pr: &PullRequest, build: &BuildDetails) -> Result<(), String>;
    fn build_running(&self, pr: &PullRequest, build: &BuildDetails) -> Result<(), String>;
    fn build_success(&self, pr: &PullRequest, build: &BuildDetails) -> Result<(), String>;
    fn build_failure(&self, pr: &PullRequest, build: &BuildDetails) -> Result<(), String>;
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Build {
    pub id: i32
}

#[derive(RustcEncodable, RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub enum BuildState {
    Queued,
    Finished,
    Running
}

#[derive(RustcEncodable, RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub enum BuildStatus {
    Success,
    Failure,
    Unknown
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct BuildDetails {
    pub id: i32,
    pub build_id: String,
    pub web_url: String,
    pub commit: Option<String>,
    pub state: BuildState,
    pub status: BuildStatus,
    pub status_text: Option<String>
}

pub trait ContinuousIntegrator {
    fn get_build_list(&self, branch: &str) -> Result<Vec<Build>, String>;
    fn get_build(&self, build_id: i32) -> Result<BuildDetails, String>;
    fn queue_build(&self, branch: &str) -> Result<BuildDetails, String>;
}

fn main() {
    let config_path = match env::args().nth(1) {
        Some(x) => x,
        None => panic!("Usage ./pr_demon path_to_config.json (Use - to read from stdin)")
    };
    let config_json = match read_config(&config_path, io::stdin()) {
        Ok(x) => x,
        Err(err) => panic!(err)
    };

    let config = match parse_config(&config_json) {
        Ok(x) => x,
        Err(err) => panic!(err)
    };

    let mut fanout = Fanout::<Message>::new();

    let subscriber = fanout.subscribe();
    thread::spawn(move || {
        for message in subscriber.iter() {
            println!("Broadcast received: {:?} {}", message.opcode, message.payload)
        }
    });

    let sleep_duration = std::time::Duration::new(config.run_interval, 0);

    loop {
        let pull_requests = match config.bitbucket.get_pr_list() {
            Err(err) => {
                println!("{}Error getting Pull Requests: {}", prefix(0), err);
                continue;
            },
            Ok(x) => x
        };

        println!("{}{} Open Pull Requests Found", prefix(0), pull_requests.len());

        for pr in &pull_requests {
            fanout.broadcast(&Message::new(OpCode::OpenPullRequest, &pr));
            println!("{}Pull Request #{} ({})", prefix(1), pr.id, pr.web_url);
            match get_latest_build(&pr, &config.teamcity) {
                None => {
                    fanout.broadcast(&Message::new(OpCode::BuildNotFound, &pr));
                    match schedule_build(&pr, &config.teamcity, &config.bitbucket) {
                        Err(err) => println!("{}{}", prefix(2), err),
                        Ok(build) => {
                            fanout.broadcast(&Message::new(OpCode::BuildScheduled, &build));
                        }
                    }
                },
                Some(build) =>  {
                    fanout.broadcast(&Message::new(OpCode::BuildFound, &build));
                    match check_build_status(&pr, &build, &config.bitbucket) {
                        Err(err) => println!("{}{}", prefix(2), err),
                        Ok(build_status_tuple) => {
                            let (build_state, build_status) = build_status_tuple;
                            let opcode = match build_state {
                                BuildState::Queued => OpCode::BuildQueued,
                                BuildState::Running => OpCode::BuildRunning,
                                BuildState::Finished => {
                                    let success = match build_status {
                                        BuildStatus::Success => true,
                                        _ => false
                                    };
                                    OpCode::BuildFinished { success: success }
                                }
                            };
                            fanout.broadcast(&Message::new(opcode, &build));
                        }
                    }
                }
            };
        }
        std::thread::sleep(sleep_duration);
    }
}

fn format_time() -> String {
    time::strftime("%Y-%m-%d %T %z", &time::now()).unwrap()
}

fn prefix(x: usize) -> String {
    // https://stackoverflow.com/questions/31216646/repeat-string-with-integer-multiplication
    format!("[{}]{} ", format_time(), iter::repeat("    ").take(x).collect::<String>())
}

fn read_config<R>(path: &str, reader: R) -> Result<String, String>
        where R : std::io::Read {
    let mut file : Box<std::io::Read> = match path {
        "-" => {
            Box::new(reader)
        },
        path @ _ => {
            match File::open(path) {
                Ok(f) => Box::new(f),
                Err(err) => return Err(format!("Unable to read file because: {}", err))
            }
        }
    };

    let mut json = String::new();
    match file.read_to_string(&mut json) {
        Ok(_) => Ok(json),
        Err(err) => Err(format!("Unable to read config: {}", err))
    }
}

fn parse_config(json: &str) -> Result<Config, String> {
    match json::decode(&json) {
        Ok(x) => Ok(x),
        Err(err) => return Err(format!("Unable to decode JSON value {}", err))
    }
}

fn get_latest_build(pr: &PullRequest, ci: &ContinuousIntegrator) -> Option<BuildDetails> {
    let branch_name = pr.branch_name();
    let pr_commit = &pr.from_commit;

    println!("{}Branch: {}", prefix(2), branch_name);
    println!("{}Commit: {}", prefix(2), pr_commit);
    println!("{}Finding latest build from branch", prefix(2));

    let latest_build = match ci.get_build_list(&branch_name) {
        Ok(ref build_list) => {
            if build_list.is_empty() {
                println!("{}Build does not exist -- running build", prefix(2));
                None
            } else {
                let latest_build_id = build_list.first().unwrap().id;
                match ci.get_build(latest_build_id) {
                    Ok(build) =>  {
                        println!("{}Latest Build Found {}", prefix(2), build.web_url);
                        Some(build)
                    },
                    Err(err) => {
                        println!("{}Unable to retrieve information for build ID {}: {}", prefix(2), latest_build_id, err);
                        None
                    }
                }
            }
        },
        Err(err) => {
            println!("{}Error fetching builds -- queuing anyway: {}", prefix(2), err);
            None
        }
    };

    match latest_build {
        None => None,
        Some(ref build) => {
            match build.commit {
                Some(ref commit) => {
                    if commit == pr_commit {
                        println!("{}Commit matches -- skipping", prefix(2));
                        Some(build.to_owned())
                    } else {
                        println!("{}Commit does not match with {} -- scheduling build", prefix(2), commit);
                        None
                    }
                },
                None if build.state == BuildState::Queued => {
                    println!("{}Build is queued -- skipping", prefix(2));
                    Some(build.to_owned())
                },
                _ => {
                    println!("{}Unknown error -- scheduling build", prefix(2));
                    None
                }
            }
        }
    }
}

fn schedule_build(pr: &PullRequest, ci: &ContinuousIntegrator, repo: &Repository)
    -> Result<BuildDetails, String> {
    println!("{}Scheduling build", prefix(2));
    let queued_build = ci.queue_build(&pr.branch_name());
    match queued_build {
        Err(err) => {
            println!("{}Error queuing build: {}", prefix(2), err);
            return Err(err)
        },
        Ok(queued) => {
            println!("{}Build Queued: {}", prefix(2), queued.web_url);
            match repo.build_queued(&pr, &queued) {
                Ok(_) => Ok(queued),
                Err(err) => Err(err)
            }
        }
    }
}

fn check_build_status(pr: &PullRequest, build: &BuildDetails, repo: &Repository)
    -> Result<(BuildState, BuildStatus), String> {
    println!("{}Build exists: {}", prefix(2), build.web_url);
    match build.state {
        BuildState::Finished => match build.status {
            BuildStatus::Success => {
                match repo.build_success(&pr, &build) {
                    Ok(_) => Ok((BuildState::Finished, BuildStatus::Success)),
                    Err(err) => Err(err)
                }
            },
            ref status @ _  => {
                match repo.build_failure(&pr, &build) {
                    Ok(_) => Ok((BuildState::Finished, status.to_owned())),
                    Err(err) => Err(err)
                }
            }
        },
        BuildState::Running => {
            match repo.build_running(&pr, &build) {
                Ok(_) => Ok((BuildState::Running, build.status.to_owned())),
                Err(err) => Err(err)
            }
        },
        BuildState::Queued  => {
            match repo.build_queued(&pr, &build) {
                Ok(_) => Ok((BuildState::Queued, build.status.to_owned())),
                Err(err) => Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{bitbucket, teamcity, Config, PullRequest, ContinuousIntegrator, Build};
    use super::{BuildDetails, BuildStatus, BuildState, Repository};
    use super::{read_config, parse_config, get_latest_build, schedule_build};
    use super::{check_build_status};
    use std::fs::File;
    use std::io::{Read, Cursor};

    struct StubBuild {
        build_list: Result<Vec<Build>, String>,
        build: Result<BuildDetails, String>,
        queued: Result<BuildDetails, String>
    }
    impl ContinuousIntegrator for StubBuild {
        fn get_build_list(&self, _: &str) -> Result<Vec<Build>, String> {
           self.build_list.clone().to_owned()
        }
        fn get_build(&self, _: i32) -> Result<BuildDetails, String> {
           self.build.clone().to_owned()
        }

        fn queue_build(&self, _: &str) -> Result<BuildDetails, String> {
           self.queued.clone().to_owned()
        }
    }

    struct StubRepository {
        pr_list: Result<Vec<PullRequest>, String>,
        queued: Result<(), String>,
        running: Result<(), String>,
        success: Result<(), String>,
        failure: Result<(), String>
    }

    impl Repository for StubRepository {
        fn get_pr_list(&self) -> Result<Vec<PullRequest>, String> {
            self.pr_list.clone().to_owned()
        }
        fn build_queued(&self, _: &PullRequest, _: &BuildDetails) -> Result<(), String> {
            self.queued.clone().to_owned()
        }
        fn build_running(&self, _: &PullRequest, _: &BuildDetails) -> Result<(), String> {
            self.running.clone().to_owned()
        }
        fn build_success(&self, _: &PullRequest, _: &BuildDetails) -> Result<(), String> {
            self.success.clone().to_owned()
        }
        fn build_failure(&self, _: &PullRequest, _: &BuildDetails) -> Result<(), String> {
            self.failure.clone().to_owned()
        }
    }

    fn pull_request() -> PullRequest {
        PullRequest {
            id: 111,
            web_url: "http://www.foobar.com/pr/111".to_owned(),
            from_ref: "refs/heads/branch_name".to_owned(),
            from_commit: "363c1dfda4cdf5a01c2d210e49942c8c8e7e898b".to_owned()
        }
    }

    fn build_success() -> BuildDetails {
        BuildDetails {
            id: 213232321,
            build_id: "somethingsomething".to_owned(),
            web_url: "http://www.goodbuilds.com/213213221".to_owned(),
            commit: Some("363c1dfda4cdf5a01c2d210e49942c8c8e7e898b".to_owned()),
            state: BuildState::Finished,
            status: BuildStatus::Success,
            status_text: Some("Build passed with flying colours".to_owned())
        }
    }

    fn build_queuing() -> BuildDetails {
        BuildDetails {
            id: 213232321,
            build_id: "somethingsomething".to_owned(),
            web_url: "http://www.goodbuilds.com/1111".to_owned(),
            commit: Some("363c1dfda4cdf5a01c2d210e49942c8c8e7e898b".to_owned()),
            state: BuildState::Queued,
            status: BuildStatus::Unknown,
            status_text: None
        }
    }

    fn build_running() -> BuildDetails {
        BuildDetails {
            id: 213232321,
            build_id: "somethingsomething".to_owned(),
            web_url: "http://www.goodbuilds.com/1111".to_owned(),
            commit: Some("363c1dfda4cdf5a01c2d210e49942c8c8e7e898b".to_owned()),
            state: BuildState::Running,
            status: BuildStatus::Success,
            status_text: None
        }
    }


    fn build_failure() -> BuildDetails {
        BuildDetails {
            id: 213232321,
            build_id: "somethingsomething".to_owned(),
            web_url: "http://www.goodbuilds.com/213213221".to_owned(),
            commit: Some("363c1dfda4cdf5a01c2d210e49942c8c8e7e898b".to_owned()),
            state: BuildState::Finished,
            status: BuildStatus::Failure,
            status_text: Some("Build failed with walking monochrome".to_owned())
        }
    }

    #[test]
    fn it_reads_from_config_file() {
        let mut expected = String::new();
        if let Err(err) = File::open("tests/fixtures/config.json")
                                .unwrap().read_to_string(&mut expected) {
                                    panic!("Unable to read fixture: {}", err);
                                }
        let actual = read_config("tests/fixtures/config.json", Cursor::new("")).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn it_reads_fron_stdin_when_presented_with_dash() {
        let payload = "foo bar baz";
        let input = Cursor::new(payload);

        let actual = read_config("-", input).unwrap();
        assert_eq!(payload, actual);
    }

    #[test]
    fn it_reads_and_parses_a_config_file() {
        let expected = Config {
            bitbucket: bitbucket::BitbucketCredentials {
                username: "username".to_owned(),
                password: "password".to_owned(),
                base_url: "https://www.example.com/bb/rest/api/latest".to_owned(),
                project_slug: "foo".to_owned(),
                repo_slug: "bar".to_owned(),
                post_build: false
            },
            teamcity: teamcity::TeamcityCredentials {
                username: "username".to_owned(),
                password: "password".to_owned(),
                build_id: "foobar".to_owned(),
                base_url: "https://www.foobar.com/rest".to_owned()
            },
            run_interval: 999
        };

        let json_string = read_config("tests/fixtures/config.json", Cursor::new("")).unwrap();
        let actual = parse_config(&json_string).unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn get_latest_build_returns_latest_buiild_successfully() {
        let expected = &build_success();
        let stub_build = StubBuild {
            build_list: Ok(vec![Build { id: 213232321 }, Build { id: 21323232}]),
            build: Ok(expected.to_owned()),
            queued: Err("This does not matter".to_owned())
        };

        let actual = get_latest_build(&pull_request(), &stub_build).unwrap();
        assert_eq!(expected, &actual);
    }

    #[test]
    fn get_latest_build_returns_none_if_no_builds_found() {
        let stub_build = StubBuild {
            build_list: Ok(vec![]),
            build: Err("ignored".to_owned()),
            queued: Err("This does not matter".to_owned())
        };
        let actual = get_latest_build(&pull_request(), &stub_build);
        assert_eq!(None, actual);
    }

    #[test]
    fn get_latest_build_returns_none_if_commit_mismatches() {
        let mut build = build_success();
        build.commit = Some("foobar".to_owned());

        let stub_build = StubBuild {
            build_list: Ok(vec![Build { id: 213232321 }, Build { id: 21323232}]),
            build: Ok(build.to_owned()),
            queued: Err("This does not matter".to_owned())
        };

        let actual = get_latest_build(&pull_request(), &stub_build);
        assert_eq!(None, actual);
    }

    #[test]
    fn get_latest_build_returns_build_if_build_queued() {
        let expected = &build_queuing();
        let stub_build = StubBuild {
            build_list: Ok(vec![Build { id: 213232321 }, Build { id: 21323232}]),
            build: Ok(expected.to_owned()),
            queued: Err("This does not matter".to_owned())
        };

        let actual = get_latest_build(&pull_request(), &stub_build).unwrap();
        assert_eq!(expected, &actual);
    }

    #[test]
    fn get_latest_build_returns_none_for_error_fetching_build_list() {
        let stub_build = StubBuild {
            build_list: Err("foobar".to_owned()),
            build: Err("This does not matter".to_owned()),
            queued: Err("This does not matter".to_owned())
        };

        let actual = get_latest_build(&pull_request(), &stub_build);
        assert_eq!(None, actual);
    }

    #[test]
    fn get_latest_build_returns_none_for_error_fetching_build() {
        let stub_build = StubBuild {
            build_list: Ok(vec![Build { id: 213232321 }, Build { id: 21323232}]),
            build: Err("foobar".to_owned()),
            queued: Err("This does not matter".to_owned())
        };

        let actual = get_latest_build(&pull_request(), &stub_build);
        assert_eq!(None, actual);
    }

    #[test]
    fn get_latest_build_returns_none_for_pathlogical_errors() {
        let mut build = build_success();
        build.commit = None;

        let stub_build = StubBuild {
            build_list: Ok(vec![Build { id: 213232321 }, Build { id: 21323232}]),
            build: Ok(build.to_owned()),
            queued: Err("This does not matter".to_owned())
        };

        let actual = get_latest_build(&pull_request(), &stub_build);
        assert_eq!(None, actual);
    }

    #[test]
    fn schedule_build_returns_build_on_scheduling() {
        let build = build_queuing();
        let stub_build = StubBuild {
            build_list: Err("This does not matter".to_owned()),
            build: Err("This does not matter".to_owned()),
            queued: Ok(build.to_owned())
        };

        let stub_repo = StubRepository {
            pr_list: Err("This does not matter".to_owned()),
            success: Ok(()),
            running: Ok(()),
            failure: Ok(()),
            queued: Ok(())
        };

        let actual = schedule_build(&pull_request(), &stub_build, &stub_repo);
        assert_eq!(Ok(build), actual);
    }

    #[test]
    fn check_build_status_returns_correct_state_and_status_on_build_success() {
        let build = build_success();
        let stub_repo = StubRepository {
            pr_list: Err("This does not matter".to_owned()),
            success: Ok(()),
            running: Ok(()),
            failure: Ok(()),
            queued: Ok(())
        };

        let actual = check_build_status(&pull_request(), &build, &stub_repo);
        assert_eq!(Ok((BuildState::Finished, BuildStatus::Success)), actual);
    }

    #[test]
    fn check_build_status_returns_correct_state_and_status_on_build_failure() {
        let build = build_failure();
        let stub_repo = StubRepository {
            pr_list: Err("This does not matter".to_owned()),
            success: Ok(()),
            running: Ok(()),
            failure: Ok(()),
            queued: Ok(())
        };

        let actual = check_build_status(&pull_request(), &build, &stub_repo);
        assert_eq!(Ok((BuildState::Finished, BuildStatus::Failure)), actual);
    }

    #[test]
    fn check_build_status_returns_correct_state_and_status_for_queued_builds() {
        let build = build_queuing();

        let stub_repo = StubRepository {
            pr_list: Err("This does not matter".to_owned()),
            success: Ok(()),
            running: Ok(()),
            failure: Ok(()),
            queued: Ok(())
        };

        let actual = check_build_status(&pull_request(), &build, &stub_repo);
        assert_eq!(Ok((BuildState::Queued, BuildStatus::Unknown)), actual);
    }

    #[test]
    fn check_build_status_returns_correct_state_and_status_for_running_builds() {
        let build = build_running();

        let stub_repo = StubRepository {
            pr_list: Err("This does not matter".to_owned()),
            success: Ok(()),
            running: Ok(()),
            failure: Ok(()),
            queued: Ok(())
        };

        let actual = check_build_status(&pull_request(), &build, &stub_repo);
        assert_eq!(Ok((BuildState::Running, BuildStatus::Success)), actual);
    }
}
