use ::rest;
use hyper;
use url::percent_encoding::{utf8_percent_encode, QUERY_ENCODE_SET};

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct TeamcityCredentials {
    pub username: String,
    pub password: String,
    pub base_url: String,
    pub build_id: String,
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
    running,
}

impl BuildState {
    fn to_build_state(self) -> ::BuildState {
        match self {
            BuildState::queued => ::BuildState::Queued,
            BuildState::finished => ::BuildState::Finished,
            BuildState::running => ::BuildState::Running,
        }
    }
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_camel_case_types)]
pub enum BuildStatus {
    SUCCESS,
    FAILURE,
    UNKNOWN,
}

impl BuildStatus {
    fn to_build_status(self) -> ::BuildStatus {
        match self {
            BuildStatus::SUCCESS => ::BuildStatus::Success,
            BuildStatus::FAILURE => ::BuildStatus::Failure,
            BuildStatus::UNKNOWN => ::BuildStatus::Unknown,
        }
    }
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct BuildList {
    pub count: i32,
    pub href: String,
    pub build: Option<Vec<BuildListItem>>,
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
    pub webUrl: String,
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
    pub finishDate: Option<String>,
    pub lastChanges: Option<LastChanges>,
    pub changes: Href,
    pub revisions: Revisions,
    pub agent: Option<Agent>,
    pub testOccurrences: Option<TestOccurences>,
    pub artifacts: Href,
    pub relatedIssues: Option<Href>,
    pub properties: Properties,
    pub statistics: Option<Href>,
}

impl Build {
    fn to_build_details(&self) -> ::BuildDetails {
        let commit = match self.revisions.revision {
            None => None,
            // Should not panic because None would have caught a non-existent vector
            Some(ref revisions) => Some(revisions.first().unwrap().version.to_owned()),
        };
        let status = match self.status {
            None => ::BuildStatus::Unknown,
            Some(ref status) => status.clone().to_build_status(),
        };
        ::BuildDetails {
            id: self.id,
            build_id: self.buildTypeId.to_owned(),
            web_url: self.webUrl.to_owned(),
            commit: commit,
            state: self.state.clone().to_build_state(),
            status: status,
            status_text: self.statusText.to_owned(),
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
    pub webUrl: String,
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct LastChanges {
    pub count: i32,
    pub change: Vec<Change>,
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct Change {
    pub id: i32,
    pub version: String,
    pub username: String,
    pub date: String,
    pub href: String,
    pub webUrl: String,
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Href {
    pub href: String,
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Revisions {
    pub count: i32,
    pub revision: Option<Vec<Revision>>,
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Revision {
    pub version: String,
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct Agent {
    pub name: String,
    pub typeId: i32,
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct TestOccurences {
    pub count: i32,
    pub href: String,
    pub passed: Option<i32>,
    pub ignored: Option<i32>,
    pub default: bool,
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Properties {
    pub count: i32,
    pub property: Vec<Property>,
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct Property {
    pub name: String,
    pub value: String,
}

impl ::ContinuousIntegrator for TeamcityCredentials {
    fn get_build_list(&self, branch: &str) -> Result<Vec<::Build>, String> {
        let mut headers = rest::Headers::new();
        headers.add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header();

        let encoded_branch = utf8_percent_encode(branch, QUERY_ENCODE_SET).collect::<String>();
        let query_string = format!("state:any,branch:(name:{})", encoded_branch);
        let url = format!("{}/buildTypes/id:{}/builds?locator={}",
                          self.base_url,
                          self.build_id,
                          query_string);

        match rest::get::<BuildList>(&url, &headers.headers) {
            Ok(build_list) => {
                Ok(match build_list.build {
                    None => vec![],
                    Some(ref builds) => {
                        builds.iter()
                            .map(|ref build| ::Build { id: build.id })
                            .collect()
                    }
                })
            }
            Err(err) => Err(format!("Error getting list of builds {}", err)),
        }
    }

    fn get_build(&self, build_id: i32) -> Result<::BuildDetails, String> {
        let mut headers = rest::Headers::new();
        headers.add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header();

        let url = format!("{}/builds/id:{}", self.base_url, build_id);

        match rest::get::<Build>(&url, &headers.headers) {
            Ok(build) => Ok(build.to_build_details()),
            Err(err) => Err(format!("Error getting build {}", err)),
        }
    }

    fn queue_build(&self, branch: &str) -> Result<::BuildDetails, String> {
        let mut headers = rest::Headers::new();
        headers.add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header()
            .add_content_type_xml_header();

        // FIXME: Format a proper template instead!
        let body = format!("<build branchName=\"{}\">
                          <buildType id=\"{}\"/>
                          <comment><text>Triggered by PR Demon</text></comment>
                        </build>",
                           branch,
                           self.build_id);
        let url = format!("{}/buildQueue", self.base_url);

        match rest::post::<Build>(&url,
                                  &body,
                                  &headers.headers,
                                  &hyper::status::StatusCode::Ok) {
            Ok(build) => Ok(build.to_build_details()),
            Err(err) => Err(format!("Error queuing build {}", err)),
        }
    }
}
