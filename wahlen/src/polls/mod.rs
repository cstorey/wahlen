use std::collections::HashMap;

use failure::Fallible;
use serde::{Deserialize, Serialize};

use infra::documents::{DocMeta, HasMeta};
use infra::ids::Entity;
use infra::ids::{Id, IdGen};
use infra::persistence::Storage;

mod resource;
mod tests;
pub use resource::PollsResource;

#[derive(Debug, Clone)]
pub struct Polls<S> {
    store: S,
    idgen: IdGen,
}

#[derive(Debug)]
pub struct CreatePoll {
    pub name: String,
}

#[derive(Debug)]
pub struct Identified<Req>(pub Id<Poll>, pub Req);

#[derive(Debug)]
pub struct RecordVote {
    pub subject_id: Id<Subject>,
    pub choice: String,
}
#[derive(Debug)]
pub struct TallyVotes;

pub struct VoteSummary {
    pub tally: HashMap<String, u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Poll {
    #[serde(flatten)]
    meta: DocMeta<Poll>,
    name: String,
    votes: HashMap<Id<Subject>, String>,
}

impl Entity for Poll {
    const PREFIX: &'static str = "poll";
}

impl HasMeta for Poll {
    fn meta(&self) -> &DocMeta<Self> {
        &self.meta
    }
    fn meta_mut(&mut self) -> &mut DocMeta<Self> {
        &mut self.meta
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Subject;

impl Entity for Subject {
    const PREFIX: &'static str = "subject";
}

pub trait GenService<Req> {
    type Resp;
    fn call(&mut self, req: Req) -> Fallible<Self::Resp>;
}

impl<S> Polls<S> {
    pub fn new(idgen: IdGen, store: S) -> Self {
        Polls { idgen, store }
    }
}

impl<S: Storage> GenService<CreatePoll> for Polls<S> {
    type Resp = Id<Poll>;
    fn call(&mut self, req: CreatePoll) -> Fallible<Self::Resp> {
        let CreatePoll { name } = req;
        let meta = DocMeta::new_with_id(self.idgen.generate());
        let votes = HashMap::new();
        let mut poll = Poll { meta, name, votes };
        self.store.save(&mut poll)?;
        Ok(poll.meta.id)
    }
}
impl<S: Storage, Req> GenService<Identified<Req>> for Polls<S>
where
    Poll: GenService<Req>,
{
    type Resp = <Poll as GenService<Req>>::Resp;
    fn call(&mut self, req: Identified<Req>) -> Fallible<Self::Resp> {
        let Identified(poll_id, inner) = req;
        let mut poll = self
            .store
            .load(&poll_id)?
            .ok_or_else(|| failure::err_msg(format!("Missing vote: {}", poll_id)))?;

        let resp = poll.call(inner)?;

        self.store.save(&mut poll)?;

        Ok(resp)
    }
}

impl GenService<RecordVote> for Poll {
    type Resp = ();
    fn call(&mut self, req: RecordVote) -> Fallible<Self::Resp> {
        let RecordVote {
            subject_id, choice, ..
        } = req;

        self.votes
            .entry(subject_id)
            .and_modify(|v| *v = choice.clone())
            .or_insert_with(|| choice.clone());

        Ok(())
    }
}

impl GenService<TallyVotes> for Poll {
    type Resp = VoteSummary;
    fn call(&mut self, _: TallyVotes) -> Fallible<Self::Resp> {
        let mut tally = HashMap::new();
        for v in self.votes.values().cloned() {
            *tally.entry(v).or_insert(0) += 1;
        }

        Ok(VoteSummary { tally })
    }
}
