use actix_web::dev::HttpServiceFactory;
use actix_web::{http, web, HttpMessage, HttpRequest, HttpResponse};
use failure::Fallible;
use std::str::FromStr;
use weft::WeftRenderable;

use super::*;
use crate::WithTemplate;

const PREFIX: &str = "/subjects";
const COOKIE_NAME: &str = "subject_id";

#[derive(Debug, Clone)]
pub struct Resource<I> {
    inner: I,
}

impl<S: Clone + Storage + 'static> Resource<Subjects<S>> {
    pub fn new(idgen: IdGen, store: S) -> Fallible<Self> {
        let inner = Subjects::new(idgen, store);
        Ok(Resource::from_inner(inner))
    }
}

impl<I: Clone + 'static> Resource<I> {
    pub fn from_inner(inner: I) -> Self {
        Resource { inner }
    }
}

#[derive(Debug, WeftRenderable)]
#[template(path = "src/subjects/subject.html")]
struct SubjectView {
    subject_id: Id<Subject>,
}

impl<I: Clone + 'static> Resource<I>
where
    I: GenService<CreateSubject, Resp = Id<Subject>>,
{
    pub fn configure(&self, cfg: &mut web::ServiceConfig) {
        cfg.service(web::scope(PREFIX).service(self.create_subject()));
    }
}

impl<I: Clone + 'static> Resource<I>
where
    I: GenService<CreateSubject, Resp = Id<Subject>>,
{
    fn create_subject(&self) -> impl HttpServiceFactory + 'static {
        let me = self.clone();
        let handler = move |req: HttpRequest| -> Result<_, actix_web::Error> {
            let subject_id = if let Some(id) = req
                .cookie(COOKIE_NAME)
                .and_then(|c| Id::from_str(c.value()).ok())
            {
                id
            } else {
                let mut inner = me.inner.clone();
                inner.call(CreateSubject)?
            };

            let view = SubjectView { subject_id };
            let html = weft::render_to_string(&WithTemplate { value: view })?;

            Ok(HttpResponse::Ok()
                .cookie(
                    http::Cookie::build(COOKIE_NAME, subject_id.to_string())
                        .http_only(true)
                        .finish(),
                )
                .body(html))
        };
        web::resource("").route(web::get().to(handler))
    }
}
