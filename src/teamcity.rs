use rest;
use hyper;

macro_rules! build_request_template {
    () => ("
<build branchName=\"{branch_name}\">
	<buildType id=\"{build_id}\" />
	<lastChanges>
		<change locator=\"version:{commit}\" />
	</lastChanges>
	<comment>
		<text>Triggered by PR Demon for #{pr_id} {pr_url}</text>
	</comment>
</build>
")
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
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

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_camel_case_types)]
pub enum BuildState {
    queued,
    finished,
    running,
}

impl BuildState {
    fn to_build_state(&self) -> ::BuildState {
        match *self {
            BuildState::queued => ::BuildState::Queued,
            BuildState::finished => ::BuildState::Finished,
            BuildState::running => ::BuildState::Running,
        }
    }
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_camel_case_types)]
pub enum BuildStatus {
    SUCCESS,
    FAILURE,
    UNKNOWN,
}

impl BuildStatus {
    fn to_build_status(&self) -> ::BuildStatus {
        match *self {
            BuildStatus::SUCCESS => ::BuildStatus::Success,
            BuildStatus::FAILURE => ::BuildStatus::Failure,
            BuildStatus::UNKNOWN => ::BuildStatus::Unknown,
        }
    }
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct BuildList {
    pub count: i32,
    pub href: String,
    pub build: Option<Vec<BuildListItem>>,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
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

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
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
            branch_name: self.branchName.to_string(),
            state: self.state.clone().to_build_state(),
            status: status,
            status_text: self.statusText.to_owned(),
        }
    }
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct BuildType {
    pub id: String,
    pub name: String,
    pub projectName: String,
    pub projectId: String,
    pub href: String,
    pub webUrl: String,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct LastChanges {
    pub count: i32,
    pub change: Vec<Change>,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct Change {
    pub id: i32,
    pub version: String,
    pub username: String,
    pub date: String,
    pub href: String,
    pub webUrl: String,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct Href {
    pub href: String,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct Revisions {
    pub count: i32,
    pub revision: Option<Vec<Revision>>,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct Revision {
    pub version: String,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct Agent {
    pub name: String,
    pub typeId: i32,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct TestOccurences {
    pub count: i32,
    pub href: String,
    pub passed: Option<i32>,
    pub ignored: Option<i32>,
    pub default: bool,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct Properties {
    pub count: i32,
    pub property: Vec<Property>,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct Property {
    pub name: String,
    pub value: String,
}

impl ::ContinuousIntegrator for TeamcityCredentials {
    fn get_build_list(&self, pr: &::PullRequest) -> Result<Vec<::Build>, String> {
        let mut headers = rest::Headers::new();
        headers
            .add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header();

        let locator = format!("defaultFilter:false,state:any,canceled:false,revision:({})",
                              pr.from_commit);
        let url = format!("{}/buildTypes/id:{}/builds?locator={}",
                          self.base_url,
                          self.build_id,
                          locator);

        let build_list = rest::get::<BuildList>(&url, headers.headers)
            .map_err(|err| format!("Error getting list of builds {}", err))?;
        Ok(match build_list.build {
               None => vec![],
               Some(ref builds) => {
                   builds
                       .iter()
                       .map(|build| ::Build { id: build.id })
                       .collect()
               }
           })
    }

    fn get_build(&self, build_id: i32) -> Result<::BuildDetails, String> {
        let mut headers = rest::Headers::new();
        headers
            .add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header();

        let url = format!("{}/builds/id:{}", self.base_url, build_id);

        let build = rest::get::<Build>(&url, headers.headers)
            .map_err(|err| format!("Error getting build {}", err))?;
        Ok(build.to_build_details())
    }

    fn queue_build(&self, pr: &::PullRequest) -> Result<::BuildDetails, String> {
        let mut headers = rest::Headers::new();
        headers
            .add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header()
            .add_content_type_xml_header();

        let logical_branch_name = format!("pull/{}/merge", pr.id);
        let body = format!(build_request_template!(),
                           branch_name = logical_branch_name,
                           build_id = self.build_id,
                           commit = pr.from_commit,
                           pr_id = pr.id,
                           pr_url = pr.web_url);
        let url = format!("{}/buildQueue", self.base_url);

        let build = rest::post::<Build>(&url, &body, headers.headers, &hyper::status::StatusCode::Ok)
            .map_err(|err| format!("Error queuing build {}", err))?;
        Ok(build.to_build_details())
    }

    fn refresh_vcs(&self) -> Result<(), String> {
        let mut headers = rest::Headers::new();
        headers
            .add_authorization_header(self as &::UsernameAndPassword)
            .add_accept_json_header();

        let url = format!("{}/vcs-root-instances/checkingForChangesQueue?locator=buildType(id:{})",
                          self.base_url,
                          self.build_id);

        let response = rest::post_raw(&url, "", headers.headers)
            .map_err(|err| format!("Error requesting for VCS fetch {}", err))?;
        match response.status() {
            status if status == &hyper::status::StatusCode::Ok => Ok(()),
            e => Err(e.to_string()),
        }
    }
}
