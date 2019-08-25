use failure::Fallible;

use infra::ids::{Id, IdGen};
use infra::persistence::Storage;

mod tests;

pub struct Polls<S> {
    store: S,
    idgen: IdGen,
}

pub struct CreatePoll {
    name: String,
}

pub struct Poll {
    id: Id<Poll>,
    name: String,
}

pub trait GenService<Req> {
    type Resp;
    fn call(&self, req: Req) -> Fallible<Self::Resp>;
}

impl<S> Polls<S> {
    pub fn new(idgen: IdGen, store: S) -> Self {
        Polls { idgen, store }
    }
}

impl<S: Storage> GenService<CreatePoll> for Polls<S> {
    type Resp = Id<Poll>;
    fn call(&self, req: CreatePoll) -> Fallible<Id<Poll>> {
        let CreatePoll { name } = req;
        let id = self.idgen.generate();
        Ok(id)
    }
}
