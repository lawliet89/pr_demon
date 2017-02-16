use std::collections::HashSet;
use fusionner;
use fusionner::merger;
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
            references.insert(pr.from_ref.to_string());
            references.insert(pr.to_ref.to_string());
        }

        info!("Fetching references");
        debug!("{:?}", references);
        let references_slice: Vec<&str> = references.iter().map(|s| &**s).collect();
        map_err!(remote.fetch(&references_slice))?;

        info!("Fetching notes for commits");
        map_err!(merger.fetch_notes())?;

        Ok(())
    }

    fn pre_build_retrieval(&self, pr: ::PullRequest) -> Result<::PullRequest, String> {
        let merger = map_err!(Self::make_merger(&self.repo,
                                                to_option_str(&self.config.notes_namespace),
                                                Some(&pr)))?;
        let merge = {
            let oid = map_err!(git2::Oid::from_str(&pr.from_commit))?;
            let target_oid = map_err!(git2::Oid::from_str(&pr.to_commit))?;
            let reference = &pr.from_ref;
            let target_ref = &pr.to_ref;

            let should_merge = merger.should_merge(oid, target_oid, reference, target_ref);
            info!("Merging {} ({} into {})?: {:?}",
                  reference,
                  oid,
                  target_oid,
                  should_merge);

            match should_merge {
                merger::ShouldMergeResult::Merge(note) => {
                    info!("Performing merge");
                    let merge = map_err!(merger.merge(oid, target_oid, &reference, target_ref))?;

                    let note = match note {
                        None => merger::Note::new_with_merge(merge.clone()),
                        Some(mut note) => {
                            note.append_with_merge(merge.clone());
                            note
                        }
                    };

                    info!("Adding note: {:?}", note);
                    map_err!(merger.add_note(&note, oid))?;
                    merge
                }
                merger::ShouldMergeResult::ExistingMergeInSameTargetReference(note) => {
                    info!("Merge commit is up to date");
                    // Should be safe to unwrap
                    note.merges.get(target_ref).unwrap().clone()
                }
                merger::ShouldMergeResult::ExistingMergeInDifferentTargetReference { mut note,
                                                                                     merges,
                                                                                     proposed_merge } => {
                    info!("Merge found under other target references: {:?}", merges);
                    note.append_with_merge(proposed_merge.clone());
                    info!("Adding note: {:?}", note);
                    map_err!(merger.add_note(&note, oid))?;
                    proposed_merge
                }
            }
        };

        let mut remote = map_err!(self.repo.remote(None))?;
        let notes_reference = merger.notes_reference();
        let refspecs = [&*merge.merge_reference, &*notes_reference];
        info!("Pushing to {:?}", refspecs);
        map_err!(remote.push(&refspecs))?;

        let mut transformed_pr = pr.clone();
        transformed_pr.from_ref = merge.merge_reference.to_string();
        transformed_pr.from_commit = merge.merge_oid.to_string();

        info!("Merge Commit: {}", merge.merge_oid);
        info!("Merge Reference: {}", merge.merge_reference);
        debug!("PR {:?} transformed to {:?}", pr, transformed_pr);
        Ok(transformed_pr)
    }
}
