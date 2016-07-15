use ::rest;
use hyper;
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

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_camel_case_types)]
pub enum BuildStatus {
    SUCCESS,
    FAILURE,
    UNKNOWN
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
    //
    // "running-info": {
    //   "percentageComplete": 96,
    //   "elapsedSeconds": 851,
    //   "estimatedTotalSeconds": 895,
    //   "currentStageText": "Step 2/3: + bundle exec rubocop -r /usr/lib64/ruby/gems/2.2.0/gems/rubocop-junit-formatter-0.1.3/lib/rubocop/formatter/junit_formatter.rb --format RuboCop::Formatter::JUnitFormatter",
    //   "outdated": false,
    //   "probablyHanging": false
    // },
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
}
