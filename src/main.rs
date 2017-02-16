extern crate chrono;
extern crate cron;
extern crate docopt;
extern crate fern;
extern crate fusionner;
extern crate hyper;
#[macro_use]
extern crate log;
extern crate reqwest;
extern crate rustc_serialize;
extern crate time;
extern crate url;

mod bitbucket;
mod fanout;
mod transformer;
mod json_dictionary;
mod rest;
mod teamcity;

use std::fs::File;
use std::io::{self, Read};
use std::iter;
use std::boxed::Box;
use std::thread;

use chrono::*;
use cron::CronSchedule;
use docopt::Docopt;
use rustc_serialize::json;

use fanout::{Fanout, Message, OpCode};

const USAGE: &'static str = "
pr_demon

Usage:
  pr_demon [options] <configuration-file>
  pr_demon -h | --help

Use with a <configuration-file> to specify a path to configuration. Use `-` to read from stdin

Options:
  -h --help                 Show this screen.
  --log-level=<log-level>   The default log level is `info`. Can be set to `trace`, `debug`, `info`, `warn`, or `error`
";

#[derive(RustcDecodable, Debug)]
struct Args {
    arg_configuration_file: String,
    flag_log_level: Option<String>,
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Config {
    // TODO: Rename fields
    teamcity: teamcity::TeamcityCredentials,
    bitbucket: bitbucket::BitbucketCredentials,
    fusionner: Option<fusionner::RepositoryConfiguration>,
    run_interval: Interval,
    stdout_broadcast: Option<bool>,
    post_build: bool,
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
enum Interval {
    Cron { expression: String },
    Fixed { interval: u64 },
}

pub trait UsernameAndPassword {
    fn username(&self) -> &String;
    fn password(&self) -> &String;
}

#[derive(RustcEncodable, RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct PullRequest {
    pub id: i32,
    pub web_url: String,
    pub from_ref: String,
    pub from_commit: String,
    pub title: String,
    pub author: User,
}

impl PullRequest {
    fn branch_name(&self) -> String {
        let git_ref = &self.from_ref;
        git_ref.split('/').skip(2).collect::<Vec<_>>().join("/")
    }
}

#[derive(RustcEncodable, RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct User {
    pub name: String,
    pub email: String,
}

pub trait Repository {
    fn get_pr_list(&self) -> Result<Vec<PullRequest>, String>;
    fn build_queued(&self, pr: &PullRequest, build: &BuildDetails) -> Result<(), String>;
    fn build_running(&self, pr: &PullRequest, build: &BuildDetails) -> Result<(), String>;
    fn build_success(&self, pr: &PullRequest, build: &BuildDetails) -> Result<(), String>;
    fn build_failure(&self, pr: &PullRequest, build: &BuildDetails) -> Result<(), String>;
    fn post_build(&self, pr: &PullRequest, build: &BuildDetails) -> Result<(), String>;
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Build {
    pub id: i32,
}

#[derive(RustcEncodable, RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub enum BuildState {
    Queued,
    Finished,
    Running,
}

#[derive(RustcEncodable, RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub enum BuildStatus {
    Success,
    Failure,
    Unknown,
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct BuildDetails {
    pub id: i32,
    pub build_id: String,
    pub web_url: String,
    pub commit: Option<String>,
    pub state: BuildState,
    pub status: BuildStatus,
    pub status_text: Option<String>,
}

pub trait ContinuousIntegrator {
    fn get_build_list(&self, pr: &PullRequest) -> Result<Vec<Build>, String>;
    fn get_build(&self, build_id: i32) -> Result<BuildDetails, String>;
    fn queue_build(&self, pr: &PullRequest) -> Result<BuildDetails, String>;
}

pub trait PrTransformer {
    fn pre_build_retrieval(&self, pr: PullRequest) -> Result<PullRequest, String>;
    fn pre_build_scheduling(&self, pr: PullRequest) -> Result<PullRequest, String>;
    fn pre_build_checking(&self, pr: PullRequest, build: &BuildDetails) -> Result<PullRequest, String>;
    fn pre_build_status_posting(&self, pr: PullRequest, build: &BuildDetails) -> Result<PullRequest, String>;
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    let logger_config = configure_logger(&args.flag_log_level);
    if let Err(e) = fern::init_global_logger(logger_config, log::LogLevelFilter::Debug) {
        panic!("Failed to initialize global logger: {}", e);
    }

    let config_json = read_config(&args.arg_configuration_file, io::stdin()).unwrap();
    let config = parse_config(&config_json).unwrap();

    let mut fanout = Fanout::<Message>::new();
    if let Some(true) = config.stdout_broadcast {
        let subscriber = fanout.subscribe();
        thread::spawn(move || for message in subscriber.iter() {
            info!("Fanout broadcast received: {:?} {}",
                  message.opcode,
                  message.payload)
        });
    }

    let bitbucket = bitbucket::Bitbucket::new(&config.bitbucket, &fanout);

    let pr_transformer: Box<PrTransformer> = match config.fusionner {
        Some(ref config) => Box::new(transformer::Fusionner::new(config.clone())),
        None => Box::new(transformer::NoOp {}),
    };

    let mut fixed_interval: Option<std::time::Duration> = None;
    let mut schedule: Option<CronSchedule> = None;

    match config.run_interval {
        Interval::Cron { expression } => schedule = Some(CronSchedule::parse(expression).unwrap()),
        Interval::Fixed { interval } => fixed_interval = Some(std::time::Duration::new(interval, 0)),
    };

    loop {
        match bitbucket.get_pr_list() {
            Err(err) => {
                error!("{}Error getting Pull Requests: {}", prefix(0), err);
            }
            Ok(prs) => {
                info!("{}{} Open Pull Requests Found", prefix(0), prs.len());
                for pr in prs {
                    info!("{}Pull Request #{} ({})", prefix(1), pr.id, pr.web_url);
                    if let Err(handled_pr) = handle_pull_request(pr,
                                                                 &bitbucket,
                                                                 &config.teamcity,
                                                                 &*pr_transformer,
                                                                 &fanout,
                                                                 config.post_build) {
                        error!("{}{}", prefix(2), handled_pr);
                    }
                }
            }
        };

        let sleep_duration = match schedule {
            Some(ref sch) => {
                // TODO: Fix these unwrapping
                (sch.next_utc().unwrap()).signed_duration_since(UTC::now()).to_std().unwrap()
            }
            None => fixed_interval.unwrap(),
        };

        info!("{} Sleeping for {} seconds",
              prefix(0),
              sleep_duration.as_secs());
        std::thread::sleep(sleep_duration);
    }
}

fn read_config<R>(path: &str, reader: R) -> Result<String, String>
    where R: std::io::Read
{
    let mut file: Box<std::io::Read> = match path {
        "-" => Box::new(reader),
        path @ _ => Box::new(File::open(path).map_err(|e| format!("Unable to read file because: {}", e))?),
    };

    let mut json = String::new();
    file.read_to_string(&mut json).map_err(|e| format!("Unable to read config: {}", e))?;
    Ok(json)
}

fn parse_config(json: &str) -> Result<Config, String> {
    json::decode(&json).map_err(|err| format!("Unable to decode JSON value {}", err))
}

fn get_latest_build(pr: &PullRequest, ci: &ContinuousIntegrator) -> Option<BuildDetails> {
    let branch_name = pr.branch_name();
    let pr_commit = &pr.from_commit;

    info!("{}Branch: {}", prefix(2), branch_name);
    info!("{}Commit: {}", prefix(2), pr_commit);
    info!("{}Finding latest build from branch", prefix(2));

    let latest_build = match ci.get_build_list(&pr) {
        Ok(ref build_list) => {
            if build_list.is_empty() {
                info!("{}Build does not exist -- running build", prefix(2));
                None
            } else {
                let latest_build_id = build_list.first().unwrap().id;
                match ci.get_build(latest_build_id) {
                    Ok(build) => {
                        info!("{}Latest Build Found {}", prefix(2), build.web_url);
                        Some(build)
                    }
                    Err(err) => {
                        error!("{}Unable to retrieve information for build ID {}: {}",
                               prefix(2),
                               latest_build_id,
                               err);
                        None
                    }
                }
            }
        }
        Err(err) => {
            warn!("{}Error fetching builds -- queuing anyway: {}",
                  prefix(2),
                  err);
            None
        }
    };

    match latest_build {
        None => None,
        Some(ref build) => {
            match build.commit {
                Some(ref commit) => {
                    if commit == pr_commit {
                        info!("{}Commit matches -- skipping", prefix(2));
                        Some(build.to_owned())
                    } else {
                        info!("{}Commit does not match with {} -- scheduling build",
                              prefix(2),
                              commit);
                        None
                    }
                }
                None if build.state == BuildState::Queued => {
                    info!("{}Build is queued -- skipping", prefix(2));
                    Some(build.to_owned())
                }
                _ => {
                    warn!("{}Unknown error -- scheduling build", prefix(2));
                    None
                }
            }
        }
    }
}

fn handle_pull_request(pr: PullRequest,
                       repo: &Repository,
                       ci: &ContinuousIntegrator,
                       pr_transformer: &PrTransformer,
                       fanout: &Fanout<Message>,
                       post_build: bool)
                       -> Result<(), String> {
    fanout.broadcast(&Message::new(OpCode::OpenPullRequest, &pr));

    let pr = pr_transformer.pre_build_retrieval(pr)?;

    match get_latest_build(&pr, ci) {
        None => {
            fanout.broadcast(&Message::new(OpCode::BuildNotFound, &pr));
            let pr = pr_transformer.pre_build_scheduling(pr)?;
            schedule_build(&pr, ci, repo).and_then(|build| {
                fanout.broadcast(&Message::new(OpCode::BuildScheduled, &build));
                Ok(())
            })
        }
        Some(build) => {
            fanout.broadcast(&Message::new(OpCode::BuildFound, &build));
            let pr = pr_transformer.pre_build_checking(pr, &build)?;
            check_build_status(&pr, &build, repo).and_then(|(build_state, build_status)| {
                let opcode = match build_state {
                    BuildState::Queued => OpCode::BuildQueued,
                    BuildState::Running => OpCode::BuildRunning,
                    BuildState::Finished => OpCode::BuildFinished { success: build_status == BuildStatus::Success },
                };
                fanout.broadcast(&Message::new(opcode, &build));
                let pr = pr_transformer.pre_build_status_posting(pr, &build)?;
                if post_build {
                    repo.post_build(&pr, &build)?;
                }
                Ok(())
            })
        }
    }
}

fn schedule_build(pr: &PullRequest, ci: &ContinuousIntegrator, repo: &Repository) -> Result<BuildDetails, String> {
    info!("{}Scheduling build", prefix(2));
    let queued_build = ci.queue_build(pr);
    match queued_build {
        Err(err) => {
            error!("{}Error queuing build: {}", prefix(2), err);
            Err(err)
        }
        Ok(queued) => {
            info!("{}Build Queued: {}", prefix(2), queued.web_url);
            repo.build_queued(&pr, &queued).and(Ok(queued))
        }
    }
}

fn check_build_status(pr: &PullRequest,
                      build: &BuildDetails,
                      repo: &Repository)
                      -> Result<(BuildState, BuildStatus), String> {
    info!("{}Build exists: {}", prefix(2), build.web_url);
    match build.state {
        BuildState::Finished => {
            match build.status {
                BuildStatus::Success => {
                    repo.build_success(&pr, &build).and(Ok((BuildState::Finished, BuildStatus::Success)))
                }
                ref status @ _ => repo.build_failure(&pr, &build).and(Ok((BuildState::Finished, status.to_owned()))),
            }
        }
        BuildState::Running => repo.build_running(&pr, &build).and(Ok((BuildState::Running, build.status.to_owned()))),
        BuildState::Queued => repo.build_queued(&pr, &build).and(Ok((BuildState::Queued, build.status.to_owned()))),
    }
}

fn prefix(x: usize) -> String {
    format!("{} ", iter::repeat("    ").take(x).collect::<String>())
}

fn to_option_str(opt: &Option<String>) -> Option<&str> {
    opt.as_ref().map(|s| &**s)
}

// TODO: Support logging to file/stderr/etc.
fn configure_logger<'a>(log_level: &Option<String>) -> fern::DispatchConfig<'a> {
    let log_level = resolve_log_level(log_level)
        .or_else(|| {
            panic!("Unknown log level `{}``", log_level.as_ref().unwrap());
        })
        .unwrap();

    fern::DispatchConfig {
        format: Box::new(|msg: &str, level: &log::LogLevel, _location: &log::LogLocation| {
            format!("[{}][{}] {}",
                    time::now().strftime("%FT%T%z").unwrap(),
                    level,
                    msg)
        }),
        output: vec![fern::OutputConfig::stdout()],
        level: log_level,
    }
}

fn resolve_log_level(log_level: &Option<String>) -> Option<log::LogLevelFilter> {
    match to_option_str(log_level) {
        Some("trace") => Some(log::LogLevelFilter::Trace),
        Some("debug") => Some(log::LogLevelFilter::Debug),
        Some("warn") => Some(log::LogLevelFilter::Warn),
        Some("error") => Some(log::LogLevelFilter::Error),
        None | Some("info") => Some(log::LogLevelFilter::Info),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{bitbucket, teamcity, Config, Interval, PullRequest, ContinuousIntegrator, Build};
    use super::{BuildDetails, BuildStatus, BuildState, Repository, User};
    use super::{read_config, parse_config, get_latest_build, schedule_build};
    use super::check_build_status;
    use std::fs::File;
    use std::io::{Read, Cursor};

    struct StubBuild {
        build_list: Result<Vec<Build>, String>,
        build: Result<BuildDetails, String>,
        queued: Result<BuildDetails, String>,
    }

    impl ContinuousIntegrator for StubBuild {
        fn get_build_list(&self, _: &PullRequest) -> Result<Vec<Build>, String> {
            self.build_list.clone().to_owned()
        }

        fn get_build(&self, _: i32) -> Result<BuildDetails, String> {
            self.build.clone().to_owned()
        }

        fn queue_build(&self, _: &PullRequest) -> Result<BuildDetails, String> {
            self.queued.clone().to_owned()
        }
    }

    struct StubRepository {
        pr_list: Result<Vec<PullRequest>, String>,
        queued: Result<(), String>,
        running: Result<(), String>,
        success: Result<(), String>,
        failure: Result<(), String>,
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
        fn post_build(&self, _pr: &PullRequest, _build: &BuildDetails) -> Result<(), String> {
            Ok(())
        }
    }

    fn pull_request() -> PullRequest {
        PullRequest {
            id: 111,
            web_url: "http://www.foobar.com/pr/111".to_owned(),
            from_ref: "refs/heads/branch_name".to_owned(),
            from_commit: "363c1dfda4cdf5a01c2d210e49942c8c8e7e898b".to_owned(),
            title: "A very important PR".to_owned(),
            author: User {
                name: "Aaron Xiao Ming".to_owned(),
                email: "aaron@xiao.ming".to_owned(),
            },
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
            status_text: Some("Build passed with flying colours".to_owned()),
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
            status_text: None,
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
            status_text: None,
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
            status_text: Some("Build failed with walking monochrome".to_owned()),
        }
    }

    #[test]
    fn it_reads_from_config_file() {
        let mut expected = String::new();
        if let Err(err) = File::open("tests/fixtures/config.json")
            .unwrap()
            .read_to_string(&mut expected) {
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
            },
            teamcity: teamcity::TeamcityCredentials {
                username: "username".to_owned(),
                password: "password".to_owned(),
                build_id: "foobar".to_owned(),
                base_url: "https://www.foobar.com/rest".to_owned(),
            },
            fusionner: None,
            run_interval: Interval::Fixed { interval: 999u64 },
            stdout_broadcast: Some(false),
            post_build: false,
        };

        let json_string = read_config("tests/fixtures/config.json", Cursor::new("")).unwrap();
        let actual = parse_config(&json_string).unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn get_latest_build_returns_latest_buiild_successfully() {
        let expected = &build_success();
        let stub_build = StubBuild {
            build_list: Ok(vec![Build { id: 213232321 }, Build { id: 21323232 }]),
            build: Ok(expected.to_owned()),
            queued: Err("This does not matter".to_owned()),
        };

        let actual = get_latest_build(&pull_request(), &stub_build).unwrap();
        assert_eq!(expected, &actual);
    }

    #[test]
    fn get_latest_build_returns_none_if_no_builds_found() {
        let stub_build = StubBuild {
            build_list: Ok(vec![]),
            build: Err("ignored".to_owned()),
            queued: Err("This does not matter".to_owned()),
        };
        let actual = get_latest_build(&pull_request(), &stub_build);
        assert_eq!(None, actual);
    }

    #[test]
    fn get_latest_build_returns_none_if_commit_mismatches() {
        let mut build = build_success();
        build.commit = Some("foobar".to_owned());

        let stub_build = StubBuild {
            build_list: Ok(vec![Build { id: 213232321 }, Build { id: 21323232 }]),
            build: Ok(build.to_owned()),
            queued: Err("This does not matter".to_owned()),
        };

        let actual = get_latest_build(&pull_request(), &stub_build);
        assert_eq!(None, actual);
    }

    #[test]
    fn get_latest_build_returns_build_if_build_queued() {
        let expected = &build_queuing();
        let stub_build = StubBuild {
            build_list: Ok(vec![Build { id: 213232321 }, Build { id: 21323232 }]),
            build: Ok(expected.to_owned()),
            queued: Err("This does not matter".to_owned()),
        };

        let actual = get_latest_build(&pull_request(), &stub_build).unwrap();
        assert_eq!(expected, &actual);
    }

    #[test]
    fn get_latest_build_returns_none_for_error_fetching_build_list() {
        let stub_build = StubBuild {
            build_list: Err("foobar".to_owned()),
            build: Err("This does not matter".to_owned()),
            queued: Err("This does not matter".to_owned()),
        };

        let actual = get_latest_build(&pull_request(), &stub_build);
        assert_eq!(None, actual);
    }

    #[test]
    fn get_latest_build_returns_none_for_error_fetching_build() {
        let stub_build = StubBuild {
            build_list: Ok(vec![Build { id: 213232321 }, Build { id: 21323232 }]),
            build: Err("foobar".to_owned()),
            queued: Err("This does not matter".to_owned()),
        };

        let actual = get_latest_build(&pull_request(), &stub_build);
        assert_eq!(None, actual);
    }

    #[test]
    fn get_latest_build_returns_none_for_pathlogical_errors() {
        let mut build = build_success();
        build.commit = None;

        let stub_build = StubBuild {
            build_list: Ok(vec![Build { id: 213232321 }, Build { id: 21323232 }]),
            build: Ok(build.to_owned()),
            queued: Err("This does not matter".to_owned()),
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
            queued: Ok(build.to_owned()),
        };

        let stub_repo = StubRepository {
            pr_list: Err("This does not matter".to_owned()),
            success: Ok(()),
            running: Ok(()),
            failure: Ok(()),
            queued: Ok(()),
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
            queued: Ok(()),
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
            queued: Ok(()),
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
            queued: Ok(()),
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
            queued: Ok(()),
        };

        let actual = check_build_status(&pull_request(), &build, &stub_repo);
        assert_eq!(Ok((BuildState::Running, BuildStatus::Success)), actual);
    }
}
