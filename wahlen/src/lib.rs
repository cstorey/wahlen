use actix_web::{web, HttpRequest, Responder};
use failure::{Error, ResultExt};
use log::*;
use r2d2::Pool;
use weft_actix::WeftResponse;
use weft_derive::WeftRenderable;

use infra::ids::IdGen;
use infra::persistence::DocumentConnectionManager;

pub mod config;
pub mod gen_service;
pub mod polls;
pub mod subjects;
mod testing;

#[derive(Debug, WeftRenderable)]
#[template(path = "src/base.html")]
pub struct WithTemplate<C> {
    value: C,
}

#[derive(Clone)]
pub struct Wahlen {
    polls: polls::PollsResource<polls::Polls<Pool<DocumentConnectionManager>>>,
    subjects: subjects::Resource<subjects::Subjects<Pool<DocumentConnectionManager>>>,
}

impl Wahlen {
    pub fn new(config: &config::Config) -> Result<Self, Error> {
        let store = config.postgres.build()?;

        store.get()?.setup().context("Setup Db")?;
        let idgen = IdGen::new();
        let polls = polls::PollsResource::new(idgen.clone(), store.clone())?;
        let subjects = subjects::Resource::new(idgen, store)?;

        Ok(Wahlen { polls, subjects })
    }

    pub fn configure(&self, cfg: &mut web::ServiceConfig) {
        cfg.service(web::resource("/").route(web::get().to_async(index)));
        self.polls.configure(cfg);
        self.subjects.configure(cfg);
    }
}

#[derive(Debug, WeftRenderable)]
#[template(path = "src/index.html")]
struct IndexView;

pub fn index(req: HttpRequest) -> Result<impl Responder, actix_web::Error> {
    info!("handling: {:?}", req);

    Ok(WeftResponse::of(WithTemplate { value: IndexView }))
}
