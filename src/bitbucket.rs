use std::collections::BTreeMap;
use std::vec::Vec;
use std::option::Option;

use hyper;
use serde::Serialize;
use serde_json;
use serde_json::map::Map;

use fanout;
use rest;

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct PagedApi<T> {
    size: i32,
    limit: i32,
    isLastPage: bool,
    values: Vec<T>,
    start: i32,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct PullRequest {
    id: i32,
    version: i32,
    title: String,
    description: Option<String>,
    state: String,
    open: bool,
    closed: bool,
    createdDate: i64,
    updatedDate: i64,
    fromRef: GitReference,
    toRef: GitReference,
    locked: bool,
    author: PullRequestParticipant,
    reviewers: Vec<PullRequestParticipant>,
    participants: Vec<PullRequestParticipant>,
    links: BTreeMap<String, Vec<Link>>,
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct Comment {
    id: i32,
    version: i32,
    text: String,
    author: User,
    createdDate: i64,
    updatedDate: i64,
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
struct CommentSubmit {
    text: String,
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
struct CommentEdit {
    text: String,
    version: i32,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct GitReference {
    id: String,
    repository: Repository,
    displayId: String,
    latestCommit: String,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
struct Repository {
    slug: String,
    name: Option<String>,
    project: Project,
    public: bool,
    links: BTreeMap<String, Vec<Link>>,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
struct Project {
    key: String,
    id: i32,
    name: String,
    description: String,
    public: bool,
    links: BTreeMap<String, Vec<Link>>,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
struct PullRequestParticipant {
    user: User,
    role: String,
    approved: bool,
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct User {
    name: String,
    emailAddress: String,
    id: i32,
    displayName: String,
    active: bool,
    slug: String,
    links: BTreeMap<String, Vec<Link>>, // type: String
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
struct Link {
    href: String,
    name: Option<String>,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct Activity {
    id: i32,
    createdDate: i64,
    user: User,
    action: String,
    commentAction: Option<String>,
    comment: Option<Comment>,
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
struct Build {
    state: BuildState,
    key: String,
    name: String,
    url: String,
    description: String,
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_camel_case_types)]
enum BuildState {
    INPROGRESS,
    FAILED,
    SUCCESSFUL,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct BitbucketCredentials {
    pub username: String,
    pub password: String,
    /// Base URL for Bitbucket
    pub base_url: String,
    pub project_slug: String,
    pub repo_slug: String,
}

pub struct Bitbucket {
    pub credentials: BitbucketCredentials,
    broadcaster: fanout::Fanout<fanout::Message>,
}

impl ::UsernameAndPassword for Bitbucket {
    fn username(&self) -> &String {
        &self.credentials.username
    }

    fn password(&self) -> &String {
        &self.credentials.password
    }
}

impl ::Repository for Bitbucket {
    fn get_pr_list(&self) -> Result<Vec<::PullRequest>, String> {
        let mut headers = rest::Headers::new();
        headers
            .add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header();
        let url = format!("{}/rest/api/latest/projects/{}/repos/{}/pull-requests",
                          self.credentials.base_url,
                          self.credentials.project_slug,
                          self.credentials.repo_slug);

        let prs = rest::get::<PagedApi<PullRequest>>(&url, headers.headers)
            .map_err(|err| format!("Error getting list of Pull Requests {}", err))?;
        Ok(prs.values
               .iter()
               .map(|pr| {
            ::PullRequest {
                id: pr.id,
                web_url: pr.links["self"][0].href.to_string(),
                from_ref: pr.fromRef.id.to_string(),
                from_commit: pr.fromRef.latestCommit.to_string(),
                to_ref: pr.toRef.id.to_string(),
                to_commit: pr.toRef.latestCommit.to_string(),
                title: pr.title.to_string(),
                author: ::User {
                    name: pr.author.user.displayName.to_string(),
                    email: pr.author.user.emailAddress.to_string(),
                },
            }
        })
               .collect())
    }

    fn build_queued(&self, pr: &::PullRequest, build: &::BuildDetails) -> Result<(), String> {
        self.update_pr_build_status_comment(pr, build, &BuildState::INPROGRESS)
            .map_err(|err| format!("Error submitting comment: {}", err))?;
        Ok(())
    }

    fn build_running(&self, pr: &::PullRequest, build: &::BuildDetails) -> Result<(), String> {
        self.build_queued(pr, build)
    }

    fn build_success(&self, pr: &::PullRequest, build: &::BuildDetails) -> Result<(), String> {
        self.update_pr_build_status_comment(pr, build, &BuildState::SUCCESSFUL)
            .map_err(|err| format!("Error submitting comment: {}", err))?;
        Ok(())
    }

    fn build_failure(&self, pr: &::PullRequest, build: &::BuildDetails) -> Result<(), String> {
        self.update_pr_build_status_comment(pr, build, &BuildState::FAILED)
            .map_err(|err| format!("Error submitting comment: {}", err))?;
        Ok(())
    }

    fn post_build(&self, pr: &::PullRequest, build: &::BuildDetails) -> Result<(), String> {
        self.post_build_status(pr, build)?;
        Ok(())
    }
}

impl Bitbucket {
    pub fn new(credentials: &BitbucketCredentials, broadcaster: &fanout::Fanout<fanout::Message>) -> Bitbucket {
        Bitbucket {
            credentials: credentials.to_owned(),
            broadcaster: broadcaster.to_owned(),
        }
    }

    fn broadcast<T>(&self, opcode: &str, payload: &T)
        where T: Serialize
    {
        let opcode = fanout::OpCode::Custom { payload: format!("Bitbucket::{}", opcode).to_owned() };
        let message = fanout::Message::new(opcode, payload);
        self.broadcaster.broadcast(&message);
    }

    fn matching_comments(comments: &[Comment], text: &str) -> Option<Comment> {
        comments
            .iter()
            .find(|&comment| comment.text == text)
            .cloned()
    }

    fn matching_comments_substring(comments: &[Comment], substr: &str) -> Option<Comment> {
        comments
            .iter()
            .find(|&comment| comment.text.as_str().contains(substr))
            .cloned()
    }

    fn update_pr_build_status_comment(&self,
                                      pr: &::PullRequest,
                                      build: &::BuildDetails,
                                      state: &BuildState)
                                      -> Result<Comment, String> {
        let text = match *state {
            BuildState::INPROGRESS => make_queued_comment(build, pr, &self.credentials),
            BuildState::FAILED => make_failure_comment(build, pr, &self.credentials),
            BuildState::SUCCESSFUL => make_success_comment(build, pr, &self.credentials),
        };

        let mut event_payload = Map::new();
        event_payload.insert("pr".to_string(),
                             serde_json::to_value(&pr).map_err(|e| e.to_string())?);
        event_payload.insert("build".to_string(),
                             serde_json::to_value(&build).map_err(|e| e.to_string())?);

        let (comment, opcode) = match self.get_comments(pr.id) {
            Ok(ref comments) => {
                match Bitbucket::matching_comments(comments, &text) {
                    Some(comment) => (Ok(comment), "Existing"),
                    None => {
                        // Have to post or edit comment
                        match Bitbucket::matching_comments_substring(comments, &pr.from_commit) {
                            Some(comment) => (self.edit_comment(pr.id, &comment, &text), "Update"),
                            None => (self.post_comment(pr.id, &text), "Post"),
                        }
                    }
                }
            }
            Err(err) => (Err(format!("Error getting list of comments {}", err)), "Error"),
        };

        if let Ok(ref comment) = comment {
            event_payload.insert("comment".to_string(),
                                 serde_json::to_value(&comment)
                                     .map_err(|e| e.to_string())?);
        }

        self.broadcast(&format!("Comment::{}", opcode), &event_payload);
        comment
    }

    fn get_comments(&self, pr_id: i32) -> Result<Vec<Comment>, String> {
        let mut headers = rest::Headers::new();
        headers
            .add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header();
        let url = format!("{}/rest/api/latest/projects/{}/repos/{}/pull-requests/{}/activities?fromType=COMMENT",
                          self.credentials.base_url,
                          self.credentials.project_slug,
                          self.credentials.repo_slug,
                          pr_id);

        let activities = rest::get::<PagedApi<Activity>>(&url, headers.headers)
            .map_err(|err| format!("Error getting comments {}", err))?;

        Ok(activities
               .values
               .iter()
               .filter(|&activity| activity.comment.is_some() && activity.user.name == self.credentials.username)
               .map(|activity| {
                        // won't panic because of filter above
                        activity.comment.as_ref().unwrap().to_owned()
                    })
               .collect())
    }

    fn post_comment(&self, pr_id: i32, text: &str) -> Result<Comment, String> {
        let mut headers = rest::Headers::new();
        headers
            .add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header()
            .add_content_type_json_header();

        let body = serde_json::to_string(&CommentSubmit { text: text.to_owned() })
            .map_err(|e| e.to_string())?;
        let url = format!("{}/rest/api/latest/projects/{}/repos/{}/pull-requests/{}/comments",
                          self.credentials.base_url,
                          self.credentials.project_slug,
                          self.credentials.repo_slug,
                          pr_id);

        Ok(rest::post::<Comment>(&url,
                                 &body,
                                 headers.headers,
                                 &hyper::status::StatusCode::Created)
                   .map_err(|err| format!("Error posting comment {}", err))?
                   .to_owned())
    }

    fn edit_comment(&self, pr_id: i32, comment: &Comment, text: &str) -> Result<Comment, String> {
        let mut headers = rest::Headers::new();
        headers
            .add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header()
            .add_content_type_json_header();

        let body = serde_json::to_string(&CommentEdit {
                                              text: text.to_owned(),
                                              version: comment.version,
                                          })
                .map_err(|e| e.to_string())?;
        let url = format!("{}/rest/api/latest/projects/{}/repos/{}/pull-requests/{}/comments/{}",
                          self.credentials.base_url,
                          self.credentials.project_slug,
                          self.credentials.repo_slug,
                          pr_id,
                          comment.id);

        Ok(rest::put::<Comment>(&url, &body, headers.headers, &hyper::status::StatusCode::Ok)
               .map_err(|err| format!("Error posting comment {}", err))?
               .to_owned())

    }

    fn post_build_status(&self, pr: &::PullRequest, build: &::BuildDetails) -> Result<Build, String> {
        let bitbucket_build = Bitbucket::make_build(build);

        let mut headers = rest::Headers::new();
        headers
            .add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header()
            .add_content_type_json_header();

        let body = serde_json::to_string(&bitbucket_build)
            .map_err(|e| e.to_string())?;
        let url = format!("{}/rest/build-status/1.0/commits/{}",
                          self.credentials.base_url,
                          pr.from_commit);

        let response = rest::post_raw(&url, &body, headers.headers)
            .map_err(|err| format!("Error posting build {}", err))?;
        match response.status() {
            status if status == &hyper::status::StatusCode::NoContent => Ok(bitbucket_build),
            e => Err(e.to_string()),
        }
    }

    fn make_build(build: &::BuildDetails) -> Build {
        let build_status = match build.state {
            ::BuildState::Finished => {
                match build.status {
                    ::BuildStatus::Success => BuildState::SUCCESSFUL,
                    _ => BuildState::FAILED,
                }
            }
            _ => BuildState::INPROGRESS,
        };

        let description = build
            .status_text
            .as_ref()
            .map_or_else(|| "".to_string(), |s| s.to_string());

        Build {
            state: build_status.to_owned(),
            key: build.build_id.to_owned(),
            name: format!("{} — {}", build.build_id, build.branch_name),
            url: build.web_url.to_owned(),
            description: description.to_owned(),
        }
    }
}

fn browse_url(base: &str, project_slug: &str, repo_slug: &str, reference: &str) -> String {
    format!("{}/projects/{}/repos/{}/browse?at={}",
            base,
            project_slug,
            repo_slug,
            reference)
}

fn commit_url(base: &str, project_slug: &str, repo_slug: &str, commit: &str) -> String {
    format!("{}/projects/{}/repos/{}/commits/{}",
            base,
            project_slug,
            repo_slug,
            commit)
}

fn make_queued_comment(build: &::BuildDetails, pr: &::PullRequest, config: &BitbucketCredentials) -> String {
    let reference_url = browse_url(&config.base_url,
                                   &config.project_slug,
                                   &config.repo_slug,
                                   &pr.from_ref);
    let commit_url = commit_url(&config.base_url,
                                &config.project_slug,
                                &config.repo_slug,
                                &pr.from_commit);
    format!("⏳ [Build]({build_url}) for [{reference}]({reference_url}) ([{commit}]({commit_url})) queued",
            build_url = build.web_url,
            reference = pr.from_ref,
            reference_url = reference_url,
            commit = pr.from_commit,
            commit_url = commit_url)
}

fn make_success_comment(build: &::BuildDetails, pr: &::PullRequest, config: &BitbucketCredentials) -> String {
    let reference_url = browse_url(&config.base_url,
                                   &config.project_slug,
                                   &config.repo_slug,
                                   &pr.from_ref);
    let commit_url = commit_url(&config.base_url,
                                &config.project_slug,
                                &config.repo_slug,
                                &pr.from_commit);
    let status_text = build
        .status_text
        .as_ref()
        .map_or_else(|| "".to_string(), |s| s.to_string());

    format!("✔️ [Build]({build_url}) for [{reference}]({reference_url}) ([{commit}]({commit_url})) \
                is **successful**: {build_message}",
            build_url = build.web_url,
            reference = pr.from_ref,
            reference_url = reference_url,
            commit = pr.from_commit,
            commit_url = commit_url,
            build_message = status_text)
}

fn make_failure_comment(build: &::BuildDetails, pr: &::PullRequest, config: &BitbucketCredentials) -> String {
    let reference_url = browse_url(&config.base_url,
                                   &config.project_slug,
                                   &config.repo_slug,
                                   &pr.from_ref);
    let commit_url = commit_url(&config.base_url,
                                &config.project_slug,
                                &config.repo_slug,
                                &pr.from_commit);
    let status_text = build
        .status_text
        .as_ref()
        .map_or_else(|| "".to_string(), |s| s.to_string());

    format!("❌ [Build]({build_url}) for [{reference}]({reference_url}) ([{commit}]({commit_url})) \
                has **failed**: {build_message}",
            build_url = build.web_url,
            reference = pr.from_ref,
            reference_url = reference_url,
            commit = pr.from_commit,
            commit_url = commit_url,
            build_message = status_text)
}
