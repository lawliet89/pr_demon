use std::collections::BTreeMap;
use std::vec::Vec;
use std::option::Option;

use ::rest;
use hyper;
use rustc_serialize::json;

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct PagedApi<T> {
    size: i32,
    limit: i32,
    isLastPage: bool,
    values: Vec<T>,
    start: i32
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct PullRequest {
    id: i32,
    version: i32,
    title: String,
    description: Option<String>,
    state: String,
    open:  bool,
    closed: bool,
    createdDate: i64,
    updatedDate: i64,
    fromRef: GitReference,
    toRef: GitReference,
    locked: bool,
    author: PullRequestParticipant,
    reviewers: Vec<PullRequestParticipant>,
    participants: Vec<PullRequestParticipant>,
    links: BTreeMap<String, Vec<Link>>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct Comment {
    id: i32,
    version: i32,
    text: String,
    author: User,
    createdDate: i64,
    updatedDate: i64
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
struct CommentSubmit {
    text: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct GitReference {
    id: String,
    repository: Repository,
    displayId: String,
    latestCommit: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Repository {
    slug: String,
    name: Option<String>,
    project: Project,
    public: bool,
    links: BTreeMap<String, Vec<Link>>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Project {
    key: String,
    id: i32,
    name: String,
    description: String,
    public: bool,
    links: BTreeMap<String, Vec<Link>>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct PullRequestParticipant {
    user: User,
    role: String,
    approved: bool
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct User {
    name: String,
    emailAddress: String,
    id: i32,
    displayName: String,
    active: bool,
    slug: String,
    links: BTreeMap<String, Vec<Link>>
    // type: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Link {
    href: String,
    name: Option<String>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
struct Activity {
    id: i32,
    createdDate: i64,
    user: User,
    action: String,
    commentAction: Option<String>,
    comment: Option<Comment>
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
struct Build {
    state: BuildState,
    key: String,
    name: String,
    url: String,
    description: String
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_camel_case_types)]
enum BuildState{
    INPROGRESS,
    FAILED,
    SUCCESSFUL
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct BitbucketCredentials {
    pub username: String,
    pub password: String,
    pub base_url: String,
    pub project_slug: String,
    pub repo_slug: String
}

impl BitbucketCredentials {
    fn matching_comments(comments: &Vec<::Comment>, text: &str) -> Option<::Comment> {
        let found_comment = comments.iter().find(|&comment| comment.text == text);
        match found_comment {
            Some(comment) => Some(comment.clone().to_owned()),
            None => None
        }
    }
}

impl ::UsernameAndPassword for BitbucketCredentials {
    fn username(&self) -> &String {
        &self.username
    }

    fn password(&self) -> &String {
        &self.password
    }
}

impl ::Repository for BitbucketCredentials {
    fn get_pr_list(&self) -> Result<Vec<::PullRequest>, String> {
        let mut headers = rest::Headers::new();
        headers.add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header();
        let url = format!("{}/api/latest/projects/{}/repos/{}/pull-requests",
            self.base_url, self.project_slug, self.repo_slug);

        match rest::get::<PagedApi<PullRequest>>(&url, &headers.headers) {
            Ok(ref prs) => {
                Ok(prs.values.iter().map( |ref pr| {
                    ::PullRequest {
                        id: pr.id,
                        web_url: pr.links["self"][0].href.to_owned(),
                        from_ref: pr.fromRef.id.to_owned(),
                        from_commit: pr.fromRef.latestCommit.to_owned()
                    }
                }).collect())
            },
            Err(err) =>  Err(format!("Error getting list of Pull Requests {}", err))
        }
    }

    fn build_queued(&self, pr: &::PullRequest, build: &::BuildDetails) -> Result<(), String> {
        let comment = ::make_queued_comment(&build.web_url, &pr.from_commit);
        match self.post_comment(pr.id, &comment) {
            Ok(_) => {},
            Err(err) => return Err(format!("Error submitting comment: {}", err))
        };
        match self.post_build(&build) {
            Ok(_) => Ok(()),
            Err(err) => return Err(format!("Error posting build: {}", err))
        }
    }

    fn build_success(&self, pr: &::PullRequest, build: &::BuildDetails) -> Result<(), String> {
        let comment = ::make_success_comment(&build.web_url, &pr.from_commit);
        match self.post_comment(pr.id, &comment) {
            Ok(_) => {},
            Err(err) => return Err(format!("Error submitting comment: {}", err))
        };

        match self.post_build(&build) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Error posting build: {}", err))
        }
    }

    fn build_failure(&self, pr: &::PullRequest, build: &::BuildDetails) -> Result<(), String> {
        let status_text = match build.status_text {
            None => "".to_owned(),
            Some(ref build_state) => build_state.to_owned()
        };
        let comment = ::make_failure_comment(&build.web_url, &pr.from_commit, &status_text);
        match self.post_comment(pr.id, &comment) {
            Ok(_) => {},
            Err(err) => return Err(format!("Error submitting comment: {}", err))
        };
        match self.post_build(&build) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Error posting build: {}", err))
        }
    }
}

impl BitbucketCredentials {
    fn get_comments(&self, pr_id: i32) -> Result<Vec<::Comment>, String> {
        let mut headers = rest::Headers::new();
        headers.add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header();
        let url = format!("{}/api/latest/projects/{}/repos/{}/pull-requests/{}/activities?fromType=COMMENT",
                self.base_url, self.project_slug, self.repo_slug, pr_id);

        match rest::get::<PagedApi<Activity>>(&url, &headers.headers) {
            Ok(activities) =>{
                Ok(
                    activities.values.iter()
                        .filter(|&activity| activity.comment.is_some())
                        .map(|ref activity| {
                            // won't panic because of filter above
                            let comment = activity.comment.as_ref().unwrap();
                            ::Comment {
                                id: comment.id,
                                text: comment.text.to_owned()
                            }
                        })
                        .collect()
                )
            },
            Err(err) =>  Err(format!("Error getting comments {}", err))
        }
    }

    fn post_comment(&self, pr_id: i32, text: &str) -> Result<::Comment, String> {
        match self.get_comments(pr_id) {
            Ok(ref comments) => {
                match BitbucketCredentials::matching_comments(&comments, &text) {
                    Some(comment) => return Ok(comment),
                    None => {}
                }
            },
            Err(err) => { println!("Error getting list of comments {}", err); }
        };

        let mut headers = rest::Headers::new();
        headers.add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header()
            .add_content_type_json_header();

        let body = json::encode(&CommentSubmit {
            text: text.to_owned()
        }).unwrap();
        let url = format!("{}/api/latest/projects/{}/repos/{}/pull-requests/{}/comments",
                self.base_url, self.project_slug, self.repo_slug, pr_id);

        match rest::post::<Comment>(&url, &body, &headers.headers, &hyper::status::StatusCode::Created) {
            Ok(comment) => {
                Ok(
                    ::Comment {
                        id: comment.id,
                        text: comment.text.to_owned()
                    }
                )
            },
            Err(err) =>  Err(format!("Error posting comment {}", err))
        }
    }


    fn post_build(&self, build: &::BuildDetails) -> Result<Build, String> {
        let bitbucket_build = BitbucketCredentials::make_build(&build);

        let commit = build.commit.clone().unwrap();

        let mut headers = rest::Headers::new();
        headers.add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header()
            .add_content_type_json_header();

        let body = json::encode(&bitbucket_build).unwrap();
        let url = format!("{}/build-status/1.0/commits/{}",
                self.base_url, commit);

        match rest::post_raw(&url, &body, &headers.headers) {
            Ok(response) => {
                match response.status {
                    ref status if status == &hyper::status::StatusCode::NoContent => Ok(bitbucket_build),
                    e @ _ => Err(e.to_string())
                }
            },
            Err(err) =>  Err(format!("Error posting build {}", err))
        }
    }

    fn make_build(build: &::BuildDetails) -> Build {
        let build_status = match build.state {
            ::BuildState::Finished => {
                match build.status {
                    ::BuildStatus::Success => BuildState::SUCCESSFUL,
                    _ => BuildState::FAILED
                }
            },
            _ => BuildState::INPROGRESS
        };

        let description = match build.status_text {
            None => "".to_owned(),
            Some(ref text) => text.to_owned()
        };

        Build {
            state: build_status.to_owned(),
            key: build.build_id.to_owned(),
            name: build.id.to_string(),
            url: build.web_url.to_owned(),
            description: description.to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BitbucketCredentials;
    use super::super::Comment;

    #[test]
    fn matching_comments_returns_matching_comment_text() {
        let expected_text = "Foo Bar Baz";
        let expected_comment = Comment {
            id: 1,
            text: expected_text.to_owned()
        };

        let comments = vec![expected_comment.clone(),
            Comment {
                id: 2,
                text: "Fizz buzz".to_owned()
            },
            Comment {
                id: 3,
                text: "Lorem Ipsum".to_owned()
            }
        ];

        let actual = BitbucketCredentials::matching_comments(&comments, &expected_text);
        assert_eq!(Some(expected_comment), actual);
    }
}
