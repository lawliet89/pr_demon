#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_camel_case_types)]
pub enum BuildState {
    queued,
    finished
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct Build {
    pub id: i32,
    pub buildTypeId: String,
    pub status: Option<String>,
    pub state: BuildState, // queued, finished,
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
