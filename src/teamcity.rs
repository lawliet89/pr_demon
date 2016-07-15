use ::rest;
use hyper;
use std::io::Read;
use rustc_serialize::json;
use hyper::client::Client;
use hyper::header::Headers;
use url::percent_encoding::{utf8_percent_encode, QUERY_ENCODE_SET};

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct TeamcityCredentials {
    pub username: String,
    pub password: String,
    pub base_url: String,
    pub build_id: String
}

impl ::UsernameAndPassword for TeamcityCredentials {
    fn username(&self) -> &String {
        &self.username
    }

    fn password(&self) -> &String {
        &self.password
    }
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_camel_case_types)]
pub enum BuildState {
    queued,
    finished,
    running
}

impl BuildState {
    fn to_build_state(self) -> ::BuildState {
        match self {
            BuildState::queued => ::BuildState::Queued,
            BuildState::finished => ::BuildState::Finished,
            BuildState::running => ::BuildState::Running
        }
    }
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_camel_case_types)]
pub enum BuildStatus {
    SUCCESS,
    FAILURE,
    UNKNOWN
}

impl BuildStatus {
    fn to_build_status(self) -> ::BuildStatus {
        match self {
            BuildStatus::SUCCESS => ::BuildStatus::Success,
            BuildStatus::FAILURE => ::BuildStatus::Failure,
            BuildStatus::UNKNOWN => ::BuildStatus::Unknown
        }
    }
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct BuildList {
    pub count: i32,
    pub href: String,
    pub build: Option<Vec<BuildListItem>>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct BuildListItem {
    pub id: i32,
    pub buildTypeId: String,
    pub status: Option<BuildStatus>,
    pub state: BuildState,
    pub running: Option<bool>,
    pub percentageComplete: Option<i32>,
    pub branchName: String,
    pub defaultBranch: Option<bool>,
    pub href: String,
    pub webUrl: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct Build {
    pub id: i32,
    pub buildTypeId: String,
    pub status: Option<BuildStatus>,
    pub state: BuildState,
    pub failedToStart: Option<bool>,
    pub branchName: String,
    pub defaultBranch: Option<bool>,
    pub href: String,
    pub webUrl: String,
    pub statusText: Option<String>,
    pub buildType: BuildType,
    pub queuedDate: String,
    pub startDate: Option<String>,
    pub finishDate:  Option<String>,
    pub lastChanges: Option<LastChanges>,
    pub changes: Href,
    pub revisions: Revisions,
    pub agent: Option<Agent>,
    pub testOccurrences: Option<TestOccurences>,
    pub artifacts: Href,
    pub relatedIssues: Option<Href>,
    pub properties: Properties,
    pub statistics: Option<Href>
}

impl Build {
    fn to_build_details(&self) -> ::BuildDetails {
        let commit = match self.revisions.revision {
            None => None,
            // Should not panic because None would have caught a non-existent vector
            Some(ref revisions) => Some(revisions.first().unwrap().version.to_owned())
        };
        let status = match self.status {
            None => ::BuildStatus::Unknown,
            Some(ref status) => status.clone().to_build_status()
        };
        ::BuildDetails {
            id: self.id,
            web_url: self.webUrl.to_owned(),
            commit: commit,
            state: self.state.clone().to_build_state(),
            status: status,
            status_text: self.statusText.to_owned()
        }
    }

}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct BuildType {
    pub id: String,
    pub name: String,
    pub projectName: String,
    pub projectId: String,
    pub href: String,
    pub webUrl: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct LastChanges {
    pub count: i32,
    pub change: Vec<Change>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct Change {
    pub id: i32,
    pub version: String,
    pub username: String,
    pub date: String,
    pub href: String,
    pub webUrl: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Href {
    pub href: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Revisions {
    pub count: i32,
    pub revision: Option<Vec<Revision>>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Revision {
    pub version: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct Agent {
    pub id: i32,
    pub name: String,
    pub typeId: i32,
    pub href: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct TestOccurences {
    pub count: i32,
    pub href: String,
    pub passed: i32,
    pub ignored: i32,
    pub default: bool
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Properties {
    pub count: i32,
    pub property: Vec<Property>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Property {
    pub name: String,
    pub value: String
}

impl ::ContinuousIntegrator for TeamcityCredentials {
    fn get_build_list(&self, branch: &str) -> Result<Vec<::Build>, String> {
        let mut headers = Headers::new();
        rest::add_authorization_header(&mut headers, self as &::UsernameAndPassword);
        rest::add_accept_json_header(&mut headers);

        let client = Client::new();
        let encoded_branch = utf8_percent_encode(branch, QUERY_ENCODE_SET).collect::<String>();
        let query_string = format!("state:any,branch:(name:{})", encoded_branch);
        let url = format!("{}/buildTypes/id:{}/builds?locator={}",
            self.base_url, self.build_id, query_string);
        let mut response = match client.get(&url).headers(headers).send() {
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

        match json::decode::<BuildList>(&json_string) {
            Ok(build_list) => {
                Ok(
                    match build_list.build {
                        None => vec![],
                        Some(ref builds) => {
                            builds.iter().map(|ref build| {
                                ::Build {
                                    id: build.id
                                }
                            }).collect()
                        }
                    }
                )
            },
            Err(err) =>  Err(format!("Error parsing response: {} {}", json_string, err))
        }
    }

    fn get_build(&self, build_id: i32) -> Result<::BuildDetails, String> {
        let mut headers = Headers::new();
        rest::add_authorization_header(&mut headers, self as &::UsernameAndPassword);
        rest::add_accept_json_header(&mut headers);

        let client = Client::new();
        let url = format!("{}/builds/id:{}", self.base_url, build_id);
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

        match json::decode::<Build>(&json_string) {
            Ok(build) => Ok(build.to_build_details()),
            Err(err) => Err(format!("Error parsing response: {} {}", json_string, err))
        }
    }

    fn queue_build(&self, branch: &str) -> Result<::BuildDetails, String> {
        let mut headers = Headers::new();
        rest::add_authorization_header(&mut headers, self as &::UsernameAndPassword);
        rest::add_accept_json_header(&mut headers);
        rest::add_content_type_xml_header(&mut headers);

        let client = Client::new();
        // FIXME: Format a proper template instead!
        let body = format!("<build branchName=\"{}\">
                          <buildType id=\"{}\"/>
                          <comment><text>Triggered by PR Demon</text></comment>
                        </build>", branch, self.build_id);
        let url = format!("{}/buildQueue", self.base_url);
        let mut response = match client
                .post(&url)
                .body(&body)
                .headers(headers).send() {
            Ok(response) => response,
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

        match json::decode::<Build>(&json_string) {
            Ok(build) => Ok(build.to_build_details()),
            Err(err) => Err(format!("Error parsing response: {} {}", json_string, err))
        }
    }
}
