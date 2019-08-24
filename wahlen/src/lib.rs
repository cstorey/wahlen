use actix_web::{web, HttpRequest, Responder};
use failure::Error;
use log::*;
use weft_actix::WeftResponse;
use weft_derive::WeftRenderable;

pub mod config;

#[derive(Debug, WeftRenderable)]
#[template(path = "src/base.html")]
pub struct WithTemplate<C> {
    value: C,
}

#[derive(Clone)]
pub struct Wahlen {}

impl Wahlen {
    pub fn new(config: &config::Config) -> Result<Self, Error> {
        let _ = config.postgres.build()?;

        Ok(Wahlen {})
    }

    pub fn configure(&self, cfg: &mut web::ServiceConfig) {
        cfg.service(web::resource("/").route(web::get().to_async(index)));
    }
}

#[derive(Debug, WeftRenderable)]
#[template(path = "src/index.html")]
struct IndexView;

pub fn index(req: HttpRequest) -> Result<impl Responder, actix_web::Error> {
    info!("handling: {:?}", req);

    Ok(WeftResponse::of(IndexView))
}
