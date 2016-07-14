use std::collections::BTreeMap;

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Build {
    id: i32,
    buildTypeId: String,
    staus: String,
    state: String,
    branchName: String,
    defaultBranch: bool,
    href: String,
    webUrl: String,
    statusText: String,
    buildType: BuildType,
    queuedDate: String,
    startDate: String,
    finishDate: String,
    triggered: BTreeMap<String, String>,
    lastChanges: LastChanges,
    changes: Href,
    revisions: Revisions,
    agent: Agent,
    testOccurrences: TestOccurences,
    artifacts: Href,
    relatedIssues: Href,
    properties: Properties,
    statistics: Href
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct BuildType {
    id: String,
    name: String,
    projectName: String,
    projectId: String,
    href: String,
    webUrl: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct LastChanges {
    count: i32,
    change: Vec<Change>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Change {
    id: i32,
    version: String,
    username: String,
    date: String,
    href: String,
    webUrl: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Href {
    href: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Revisions {
    count: i32,
    revision: Vec<Revision>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Revision {
    version: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Agent {
    id: i32,
    name: String,
    typeId: i32,
    href: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct TestOccurences {
    count: i32,
    href: String,
    passed: i32,
    ignored: i32,
    default: bool
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Properties {
    count: i32,
    property: Vec<Property>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct Property {
    name: String,
    value: String
}
