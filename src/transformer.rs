use fusionner;

pub struct NoOp {}

pub struct Fusionner {
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

impl Fusionner {
  pub fn new(config: fusionner::RepositoryConfiguration) -> Fusionner {
    Fusionner {
      config: config
    }
  }
}

impl ::PrTransformer for Fusionner {
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
