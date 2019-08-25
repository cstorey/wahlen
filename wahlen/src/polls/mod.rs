use std::collections::HashMap;

use failure::Fallible;
use serde::{Deserialize, Serialize};

use infra::documents::{DocMeta, HasMeta};
use infra::ids::Entity;
use infra::ids::{Id, IdGen};
use infra::persistence::Storage;

mod tests;

pub struct Polls<S> {
    store: S,
    idgen: IdGen,
}

#[derive(Debug)]
pub struct CreatePoll {
    name: String,
}

#[derive(Debug)]
pub struct RecordVote {
    poll_id: Id<Poll>,
    subject_id: Id<Subject>,
    choice: String,
}
#[derive(Debug)]
pub struct TallyVotes {
    poll_id: Id<Poll>,
}
pub struct VoteSummary {
    tally: HashMap<String, u64>,
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
impl<S: Storage> GenService<RecordVote> for Polls<S> {
    type Resp = ();
    fn call(&mut self, req: RecordVote) -> Fallible<Self::Resp> {
        let mut poll = self
            .store
            .load(&req.poll_id)?
            .ok_or_else(|| failure::err_msg(format!("Missing vote: {}", req.poll_id)))?;

        poll.call(req)?;

        self.store.save(&mut poll)?;

        Ok(())
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

impl<S: Storage> GenService<TallyVotes> for Polls<S> {
    type Resp = VoteSummary;
    fn call(&mut self, req: TallyVotes) -> Fallible<Self::Resp> {
        let TallyVotes { poll_id } = req;
        let mut poll = self
            .store
            .load(&req.poll_id)?
            .ok_or_else(|| failure::err_msg(format!("Missing vote: {}", req.poll_id)))?;

        let tally = poll.call(req)?;

        Ok(tally)
    }
}
impl GenService<TallyVotes> for Poll {
    type Resp = VoteSummary;
    fn call(&mut self, req: TallyVotes) -> Fallible<Self::Resp> {
        let mut tally = HashMap::new();
        for v in self.votes.values().cloned() {
            *tally.entry(v).or_insert(0) += 1;
        }

        Ok(VoteSummary { tally })
    }
}
