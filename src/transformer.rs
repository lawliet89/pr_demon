use std::collections::HashSet;
use fusionner;
use git2;

static DEFAULT_REFSPEC: &'static str = "refs/pull/*";

macro_rules! map_err {
    ($x:expr) => {
        $x.map_err(|e| format!("{:?}", e))
    }
}

fn to_option_str(opt: &Option<String>) -> Option<&str> {
    opt.as_ref().map(|s| &**s)
}

pub struct NoOp {}
impl ::PrTransformer for NoOp {}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct FusionnerConfiguration {
    pub notes_namespace: Option<String>,
    pub repository: fusionner::RepositoryConfiguration,
    pub push: Option<bool>,
}

pub struct Fusionner<'repo> {
    repo: fusionner::git::Repository<'repo>,
    config: FusionnerConfiguration,
}

impl<'repo> Fusionner<'repo> {
    pub fn new(config: &'repo FusionnerConfiguration) -> Result<Fusionner<'repo>, String> {
        let repo = map_err!(fusionner::git::Repository::<'repo>::clone_or_open(&config.repository))?;

        {
            // One time setup of refspecs
            let merger = Self::make_merger(&repo, to_option_str(&config.notes_namespace), None)?;
            // Add the necessary refspecs
            map_err!(merger.add_note_refspecs())?;
        }
        {
            let remote = map_err!(repo.remote(None))?;
            let refspec = remote.generate_refspec(DEFAULT_REFSPEC, true)?;
            map_err!(remote.add_refspec(&refspec, git2::Direction::Fetch))?;
            map_err!(remote.add_refspec(&refspec, git2::Direction::Push))?;
        }

        Ok(Fusionner {
            repo: repo,
            config: config.clone(),
        })
    }

    fn make_merger<'cb>(repo: &'repo fusionner::git::Repository<'repo>,
                        namespace: Option<&str>,
                        pr: Option<&::PullRequest>)
                        -> Result<fusionner::merger::Merger<'repo, 'cb>, String>
        where 'repo: 'cb
    {
        let namer = match pr {
            Some(pr) => Some(fusionner::merger::MergeReferenceNamer::Custom(Self::make_namer(pr))),
            None => None,
        };

        map_err!(fusionner::merger::Merger::new(repo, None, namespace, namer))
    }

    fn make_namer<'cb>(pr: &::PullRequest) -> Box<fusionner::merger::MergeReferenceNamerCallback<'cb>> {
        let pr_id = pr.id;

        Box::new(move |_reference : _, _target_reference : _, _oid : _, _target_oid : _| {
            format!("refs/pull/{}/merge", pr_id)
        })
    }
}

impl<'repo> ::PrTransformer for Fusionner<'repo> {
    fn prepare(&self, prs: &Vec<::PullRequest>) -> Result<(), String> {
        let mut remote = map_err!(self.repo.remote(None))?;
        let mut merger = map_err!(Self::make_merger(&self.repo,
                                                    to_option_str(&self.config.notes_namespace),
                                                    None))?;

        let mut references = HashSet::<String>::new();

        info!("Gathering references and commits from PRs to fetch from remote");
        for pr in prs {
            references.insert(format!("+{}", pr.from_ref.to_string()));
            references.insert(format!("+{}", pr.to_ref.to_string()));
        }

        info!("Fetching references");
        debug!("{:?}", references);
        let references_slice: Vec<&str> = references.iter().map(|s| &**s).collect();
        map_err!(remote.fetch(&references_slice))?;

        info!("Fetching notes for commits");
        map_err!(merger.fetch_notes())?;

        Ok(())
    }

    /// Transform PR with commits into merge commit
    fn pre_build_retrieval(&self, pr: ::PullRequest) -> Result<::PullRequest, String> {
        let mut merger = map_err!(Self::make_merger(&self.repo,
                                                    to_option_str(&self.config.notes_namespace),
                                                    Some(&pr)))?;

        let oid = map_err!(git2::Oid::from_str(&pr.from_commit))?;
        let target_oid = map_err!(git2::Oid::from_str(&pr.to_commit))?;
        let reference = &pr.from_ref;
        let target_ref = &pr.to_ref;
        let push = self.config.push != Some(false);
        let merge = map_err!(merger.check_and_merge(oid, target_oid, reference, target_ref, push))?;

        let mut transformed_pr = pr.clone();
        transformed_pr.from_ref = merge.merge_reference.to_string();
        transformed_pr.from_commit = merge.merge_oid.to_string();

        info!("Merge Commit: {}", merge.merge_oid);
        info!("Merge Reference: {}", merge.merge_reference);
        debug!("PR {:?} transformed to {:?}", pr, transformed_pr);
        Ok(transformed_pr)
    }

    /// Reverse transform PR with merge commit into original commits
    fn pre_build_status_posting(&self, pr: ::PullRequest, _build: &::BuildDetails) -> Result<::PullRequest, String> {
        let merge_oid = map_err!(git2::Oid::from_str(&pr.from_commit))?;
        let target_oid = map_err!(git2::Oid::from_str(&pr.to_commit))?;
        let merge_commit = map_err!(self.repo.repository.find_commit(merge_oid))?;

        let pr_oid: Vec<git2::Oid> = merge_commit.parent_ids().filter(|oid| *oid != target_oid).collect();

        if pr_oid.len() != 1 {
            return Err(format!("Exactly one non-target OID was not found: {:?}", pr_oid));
        }
        let pr_oid = pr_oid[0];

        let mut transformed_pr = pr.clone();
        transformed_pr.from_commit = format!("{}", pr_oid);

        // FIXME: There is no good way to get back the original PR reference. How should we do this?

        info!("PR Commit: {}", transformed_pr.from_commit);
        debug!("Transformed PR {:?} reversed to {:?}", pr, transformed_pr);
        Ok(transformed_pr)
    }
}

#[cfg(test)]
mod tests {
    extern crate rand;
    extern crate tempdir;
    extern crate url;

    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    use fusionner;
    use git2;
    use self::tempdir::TempDir;
    use self::url::Url;
    use self::rand::Rng;

    use transformer;
    use PrTransformer;

    macro_rules! not_err {
        ($e:expr) => (match $e {
            Ok(e) => e,
            Err(e) => panic!("{} failed with {}", stringify!($e), e),
        })
    }

    macro_rules! not_none {
        ($e:expr) => (match $e {
            Some(e) => e,
            None => panic!("{} failed with None", stringify!($e)),
        })
    }

    fn raw_repo_init() -> (TempDir, git2::Repository) {
        let td = TempDir::new("test").unwrap();
        let repo = git2::Repository::init(td.path()).unwrap();
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "name").unwrap();
            config.set_str("user.email", "email").unwrap();
            let mut index = repo.index().unwrap();
            let id = index.write_tree().unwrap();

            let tree = repo.find_tree(id).unwrap();
            let sig = repo.signature().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap();

            repo.remote("origin", &path2url(&td.path())).unwrap();
        }
        (td, repo)
    }

    fn config_init(tempdir: &TempDir) -> fusionner::RepositoryConfiguration {
        let path = tempdir.path();
        fusionner::RepositoryConfiguration {
            uri: path2url(&path),
            checkout_path: path.to_str().unwrap().to_string(),
            fetch_refspecs: vec![],
            push_refspecs: vec![],
            username: Some("foobar".to_string()),
            password: Some("password".to_string()),
            key: Some("/path/to/some.key".to_string()),
            key_passphrase: None,
            signature_name: None,
            signature_email: None,
        }
    }

    fn repo_init<'a>(config: &'a fusionner::RepositoryConfiguration) -> fusionner::git::Repository<'a> {
        fusionner::git::Repository::open(&config).unwrap()
    }

    fn path2url(path: &Path) -> String {
        Url::from_file_path(path).unwrap().to_string()
    }

    fn head_oid(repo: &fusionner::git::Repository) -> git2::Oid {
        let reference = not_err!(repo.repository.head());
        not_none!(reference.target())
    }

    fn add_branch_commit(repo: &fusionner::git::Repository) -> git2::Oid {
        add_branch_commit_with_reference(repo, "refs/heads/branch")
    }

    fn add_branch_commit_with_reference(repo: &fusionner::git::Repository, reference: &str) -> git2::Oid {
        let repo = &repo.repository;
        let mut parent_commit = vec![];

        // Checkout tree if it exists
        let resolved_reference = repo.find_reference(reference);
        if let Ok(resolved_reference) = resolved_reference {
            let resolved_reference = resolved_reference.resolve().unwrap();
            let oid = resolved_reference.target().unwrap();
            let commit = repo.find_commit(oid).unwrap();
            let tree = commit.tree().unwrap();

            let mut checkout_builder = git2::build::CheckoutBuilder::new();
            checkout_builder.force();

            repo.checkout_tree(tree.as_object(), Some(&mut checkout_builder)).unwrap();
            parent_commit.push(commit);
        }

        let mut index = repo.index().unwrap();
        let workdir = repo.workdir().unwrap();
        let random_string = rand::thread_rng()
            .gen_ascii_chars()
            .take(10)
            .collect::<String>();
        let file = workdir.join(&random_string);
        println!("{:?}", file);

        {
            let mut random_file = File::create(&file).unwrap();
            random_file.write_all(random_string.as_bytes()).unwrap();
        }
        // Add file to index
        index.add_path(Path::new(&random_string)).unwrap();

        let id = index.write_tree_to(repo).unwrap();

        let tree = repo.find_tree(id).unwrap();
        let sig = repo.signature().unwrap();

        let parents: Vec<&git2::Commit> = parent_commit.iter().map(|c| c).collect();

        repo.commit(Some(reference), &sig, &sig, "branch", &tree, &parents)
            .unwrap()
    }

    fn make_pr(oid: git2::Oid, target_oid: git2::Oid, reference: &str, target_reference: &str) -> ::PullRequest {
        ::PullRequest {
            id: 1,
            web_url: "https://www.example.com".to_string(),
            from_ref: reference.to_string(),
            from_commit: format!("{}", oid),
            to_ref: target_reference.to_string(),
            to_commit: format!("{}", target_oid),
            title: "Foobar".to_string(),
            author: ::User {
                name: "John Doe".to_string(),
                email: "email@foobar.com".to_string(),
            },
        }
    }

    #[test]
    fn fusionner_pre_build_retrieval_smoke_test() {
        let (td, _raw) = raw_repo_init();
        let config = config_init(&td);
        let repo = repo_init(&config);

        let oid = head_oid(&repo);
        let branch_oid = add_branch_commit(&repo);
        let reference = "refs/heads/branch";
        let target_reference = "refs/heads/master";

        let pr = make_pr(branch_oid, oid, reference, target_reference);

        let transformer_config = transformer::FusionnerConfiguration {
            repository: config.clone(),
            notes_namespace: None,
            push: Some(false),
        };
        let transformer = not_err!(transformer::Fusionner::new(&transformer_config));

        let transformed_pr = not_err!(transformer.pre_build_retrieval(pr));

        assert_eq!("refs/pull/1/merge", transformed_pr.from_ref);
        assert!(transformed_pr.from_commit != format!("{}", branch_oid));
    }

    #[test]
    fn fusionner_pre_build_retrieval_finds_existing_merge() {
        let (td, _raw) = raw_repo_init();
        let config = config_init(&td);
        let repo = repo_init(&config);

        let oid = head_oid(&repo);
        let branch_oid = add_branch_commit(&repo);
        let reference = "refs/heads/branch";
        let target_reference = "refs/heads/master";

        let pr = make_pr(branch_oid, oid, reference, target_reference);
        let transformer_config = transformer::FusionnerConfiguration {
            repository: config.clone(),
            notes_namespace: None,
            push: Some(false),
        };

        let transformer = not_err!(transformer::Fusionner::new(&transformer_config));
        let mut merger = not_err!(transformer::Fusionner::make_merger(&transformer.repo, None, Some(&pr)));
        let merge = not_err!(merger.check_and_merge(branch_oid, oid, reference, target_reference, false));

        let transformed_pr = not_err!(transformer.pre_build_retrieval(pr));

        assert_eq!(merge.merge_oid, transformed_pr.from_commit);
        assert_eq!(merge.merge_reference, transformed_pr.from_ref);
    }

    #[test]
    fn fusionner_pre_build_status_posting_smoke_test() {
        let (td, _raw) = raw_repo_init();
        let config = config_init(&td);
        let repo = repo_init(&config);

        let oid = head_oid(&repo);
        let branch_oid = add_branch_commit(&repo);
        let reference = "refs/heads/branch";
        let target_reference = "refs/heads/master";

        let pr = make_pr(branch_oid, oid, reference, target_reference);
        let transformer_config = transformer::FusionnerConfiguration {
            repository: config.clone(),
            notes_namespace: None,
            push: Some(false),
        };

        let transformer = not_err!(transformer::Fusionner::new(&transformer_config));
        let mut merger = not_err!(transformer::Fusionner::make_merger(&transformer.repo, None, Some(&pr)));
        let _merge = not_err!(merger.check_and_merge(branch_oid, oid, reference, target_reference, false));

        let transformed_pr = not_err!(transformer.pre_build_retrieval(pr));

        let build_details = ::BuildDetails {
            id: 0,
            build_id: "foobar".to_string(),
            web_url: "http://www.example.com".to_string(),
            commit: None,
            branch_name: "foobar".to_string(),
            state: ::BuildState::Finished,
            status: ::BuildStatus::Success,
            status_text: None,
        };
        let reverse_transformed_pr = not_err!(transformer.pre_build_status_posting(transformed_pr, &build_details));

        assert_eq!(format!("{}", branch_oid),
                   reverse_transformed_pr.from_commit);
    }
}
