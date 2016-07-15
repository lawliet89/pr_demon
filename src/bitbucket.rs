use std::collections::BTreeMap;
use std::vec::Vec;
use std::option::Option;

use ::rest;
use hyper;
use std::io::Read;
use rustc_serialize::json;
use hyper::client::Client;
use hyper::header::Headers;

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct PagedApi<T> {
    pub size: i32,
    pub limit: i32,
    pub isLastPage: bool,
    pub values: Vec<T>,
    pub start: i32
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct PullRequest {
    pub id: i32,
    pub version: i32,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub open:  bool,
    pub closed: bool,
    pub createdDate: i64,
    pub updatedDate: i64,
    pub fromRef: GitReference,
    pub toRef: GitReference,
    pub locked: bool,
    pub author: PullRequestParticipant,
    pub reviewers: Vec<PullRequestParticipant>,
    pub participants: Vec<PullRequestParticipant>,
    pub links: BTreeMap<String, Vec<Link>>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct Comment {
    pub id: i32,
    pub version: i32,
    pub text: String,
    pub author: User,
    pub createdDate: i64,
    pub updatedDate: i64
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct CommentSubmit {
    pub text: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct GitReference {
    pub id: String,
    pub repository: Repository,
    pub displayId: String,
    pub latestCommit: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Repository {
    pub slug: String,
    pub name: Option<String>,
    pub project: Project,
    pub public: bool,
    pub links: BTreeMap<String, Vec<Link>>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Project {
    pub key: String,
    pub id: i32,
    pub name: String,
    pub description: String,
    pub public: bool,
    pub links: BTreeMap<String, Vec<Link>>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct PullRequestParticipant {
    pub user: User,
    pub role: String,
    pub approved: bool
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct User {
    pub name: String,
    pub emailAddress: String,
    pub id: i32,
    pub displayName: String,
    pub active: bool,
    pub slug: String,
    pub links: BTreeMap<String, Vec<Link>>
    // type: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Link {
    pub href: String,
    pub name: Option<String>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct Activity {
    pub id: i32,
    pub createdDate: i64,
    pub user: User,
    pub action: String,
    pub commentAction: Option<String>,
    pub comment: Option<Comment>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct BitbucketCredentials {
    pub username: String,
    pub password: String,
    pub base_url: String,
    pub project_slug: String,
    pub repo_slug: String
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
        let mut headers = Headers::new();
        rest::add_authorization_header(&mut headers, self as &::UsernameAndPassword);
        let client = Client::new();
        let url = format!("{}/projects/{}/repos/{}/pull-requests",
            self.base_url, self.project_slug, self.repo_slug);
        let mut response = match client.get(&url).headers(headers).send() {
            Ok(x) => x,
            Err(err) => return Err(format!("Unable to get list of PR: {}", err))
        };

        match response.status {
            hyper::status::StatusCode::Ok => (),
            e @ _ => return Err(e.to_string())
        };

        let mut json_string = String::new();
        if let Err(err) = response.read_to_string(&mut json_string) {
            return Err(format!("Unable to get a list of PR: {}", err))
        }

        match json::decode::<PagedApi<PullRequest>>(&json_string) {
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
            Err(err) =>  Err(format!("Error parsing response: {}", err))
        }
    }

    fn get_comments(&self, pr_id: i32) -> Result<Vec<::Comment>, String> {
        let mut headers = Headers::new();
        rest::add_authorization_header(&mut headers, self as &::UsernameAndPassword);

        let client = Client::new();
        let url = format!("{}/projects/{}/repos/{}/pull-requests/{}/activities?fromType=COMMENT",
                self.base_url, self.project_slug, self.repo_slug, pr_id);
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

        match json::decode::<PagedApi<Activity>>(&json_string) {
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
            Err(err) =>  Err(format!("Error parsing response: {} {}", json_string, err))
        }
    }

    fn post_comment(&self, pr_id: i32, text: &str) -> Result<::Comment, String> {
        match self.get_comments(pr_id) {
            Ok(ref comments) => {
                let found_comment = comments.iter().find(|&comment| comment.text == text);
                match found_comment {
                    Some(comment) => return Ok(comment.clone().to_owned()),
                    None => {}
                }
            },
            Err(err) => { println!("Error getting list of comments {}", err); }
        };

        let mut headers = Headers::new();
        rest::add_authorization_header(&mut headers, self as &::UsernameAndPassword);
        rest::add_accept_json_header(&mut headers);
        rest::add_content_type_json_header(&mut headers);

        let client = Client::new();
        let body = json::encode(&CommentSubmit {
            text: text.to_owned()
        }).unwrap();
        let url = format!("{}/projects/{}/repos/{}/pull-requests/{}/comments",
                self.base_url, self.project_slug, self.repo_slug, pr_id);
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
            return Err(format!("Unable to submit comment: {}", err))
        }

        match json::decode::<Comment>(&json_string) {
            Ok(comment) => {
                Ok(
                    ::Comment {
                        id: comment.id,
                        text: comment.text.to_owned()
                    }
                )
            },
            Err(err) =>  Err(format!("Error parsing response: {} {}", json_string, err))
        }
    }
}
