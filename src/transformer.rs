use std::collections::HashSet;
use fusionner;

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

pub struct Fusionner<'repo> {
    repo: fusionner::git::Repository<'repo>,
    config: fusionner::RepositoryConfiguration,
}


impl<'repo> Fusionner<'repo> {
    pub fn new(config: &'repo fusionner::RepositoryConfiguration) -> Result<Fusionner<'repo>, String> {
        let repo = map_err!(fusionner::git::Repository::<'repo>::clone_or_open(&config))?;

        {
            // One time setup of refspecs
            let merger = Self::make_merger(&repo, to_option_str(&config.notes_namespace), None)?;
            // Add the necessary refspecs
            map_err!(merger.add_note_refspecs())?;
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
        let mut commits = HashSet::<String>::new();

        info!("Gathering references and commits from PRs to fetch from remote");
        for pr in prs {
            references.insert(pr.from_ref.to_string());
            references.insert(pr.to_ref.to_string());
            commits.insert(pr.from_commit.to_string());
        }

        info!("Fetching references");
        debug!("{:?}", references);
        let references_slice: Vec<&str> = references.iter().map(|s| &s[..]).collect();
        map_err!(remote.fetch(&references_slice))?;

        info!("Fetching notes for commits");
        debug!("{:?}", commits);
        let commits_slice: Vec<&str> = commits.iter().map(|s| &s[..]).collect();
        map_err!(merger.fetch_notes(&commits_slice))?;

        Ok(())
    }

    fn finalize(&self, _prs: &Vec<::PullRequest>) -> Result<(), String> {
        let mut merger = map_err!(Self::make_merger(&self.repo,
                                                    to_option_str(&self.config.notes_namespace),
                                                    None))?;
        map_err!(merger.push())?;
        Ok(())
    }
}
