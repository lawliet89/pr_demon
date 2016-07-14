use std::collections::BTreeMap;
use std::vec::Vec;
use std::option::Option;

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
