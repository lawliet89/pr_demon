use std::vec::Vec;
use std::option::Option;

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct PagedApi<T> {
    size: i32,
    limit: i32,
    isLastPage: bool,
    values: Vec<T>,
    start: i32
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct PullRequest {
    id: i32,
    version: i32,
    title: String,
    description: String,
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
    participants: Vec<PullRequestParticipant>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct GitReference {
    id: String,
    repository: Repository
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Repository {
    slug: String,
    name: Option<String>,
    project: Project
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Project {
    key: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct PullRequestParticipant {
    user: User,
    role: String,
    approved: bool
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct User {
    name: String,
    emailAddress: String,
    id: i32,
    displayName: String,
    active: bool,
    slug: String
    // type: String
}
