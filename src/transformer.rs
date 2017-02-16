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

pub struct Fusionner<'repo> {
    repo: fusionner::git::Repository<'repo>,
    config: fusionner::RepositoryConfiguration,
}

impl NoOp {
    pub fn no_op(pr: ::PullRequest) -> Result<::PullRequest, String> {
        Ok(pr)
    }
}

impl ::PrTransformer for NoOp {
    fn pre_build_retrieval(&self, pr: ::PullRequest) -> Result<::PullRequest, String> {
        Self::no_op(pr)
    }

    fn pre_build_scheduling(&self, pr: ::PullRequest) -> Result<::PullRequest, String> {
        Self::no_op(pr)
    }

    fn pre_build_checking(&self, pr: ::PullRequest, _build: &::BuildDetails) -> Result<::PullRequest, String> {
        Self::no_op(pr)
    }

    fn pre_build_status_posting(&self, pr: ::PullRequest, _build: &::BuildDetails) -> Result<::PullRequest, String> {
        Self::no_op(pr)
    }
}

impl<'repo> Fusionner<'repo> {
    pub fn new(config: &'repo fusionner::RepositoryConfiguration) -> Result<Fusionner<'repo>, String> {
        let repo = map_err!(fusionner::git::Repository::<'repo>::clone_or_open(&config))?;

        {
            // One time setup of refspecs
            let mut merger = Self::make_merger(&repo, to_option_str(&config.notes_namespace), None)?;
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
            Some(pr) => Some(fusionner::merger::MergeReferenceNamer::Default),
            None => None,
        };

        map_err!(fusionner::merger::Merger::new(repo, None, namespace, namer))
    }
}

impl<'repo> ::PrTransformer for Fusionner<'repo> {
    fn pre_build_retrieval(&self, pr: ::PullRequest) -> Result<::PullRequest, String> {
        NoOp::no_op(pr)
    }

    fn pre_build_scheduling(&self, pr: ::PullRequest) -> Result<::PullRequest, String> {
        NoOp::no_op(pr)
    }

    fn pre_build_checking(&self, pr: ::PullRequest, _build: &::BuildDetails) -> Result<::PullRequest, String> {
        NoOp::no_op(pr)
    }

    fn pre_build_status_posting(&self, pr: ::PullRequest, _build: &::BuildDetails) -> Result<::PullRequest, String> {
        NoOp::no_op(pr)
    }
}
