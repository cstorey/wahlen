use failure::Fallible;
use serde::{Deserialize, Serialize};

mod resource;
mod tests;

pub use self::resource::Resource;
use crate::gen_service::GenService;
use infra::documents::{DocMeta, HasMeta};
use infra::ids::{Entity, Id, IdGen};
use infra::persistence::Storage;

#[derive(Debug, Clone)]
pub struct Subjects<S> {
    store: S,
    idgen: IdGen,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Subject {
    #[serde(flatten)]
    meta: DocMeta<Subject>,
}

impl Entity for Subject {
    const PREFIX: &'static str = "subject";
}

impl HasMeta for Subject {
    fn meta(&self) -> &DocMeta<Self> {
        &self.meta
    }
    fn meta_mut(&mut self) -> &mut DocMeta<Self> {
        &mut self.meta
    }
}

#[derive(Debug)]
pub struct CreateSubject;

impl<S> Subjects<S> {
    pub fn new(idgen: IdGen, store: S) -> Self {
        Subjects { idgen, store }
    }
}

impl<S: Storage> GenService<CreateSubject> for Subjects<S> {
    type Resp = Id<Subject>;

    fn call(&mut self, _: CreateSubject) -> Fallible<Self::Resp> {
        let meta = DocMeta::new_with_id(self.idgen.generate());
        let mut subject = Subject { meta };
        self.store.save(&mut subject)?;
        Ok(subject.meta.id)
    }
}
