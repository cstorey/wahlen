use std::sync::{Arc, Mutex};

use actix_web::dev::HttpServiceFactory;
use actix_web::{web, HttpRequest, HttpResponse};
use failure::Fallible;
use weft::WeftRenderable;
use weft_actix::WeftResponse;

use super::*;
use crate::WithTemplate;

const PREFIX: &str = "/polls";

#[derive(Debug, Clone)]
pub struct PollsResource<I> {
    inner: Arc<Mutex<I>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreatePollForm {
    name: String,
}

#[derive(Debug, WeftRenderable)]
#[template(path = "src/polls/poll.html")]
struct PollView;

impl<S: Clone + Storage + 'static> PollsResource<Polls<S>> {
    pub fn new(idgen: IdGen, store: S) -> Fallible<Self> {
        let inner = Polls::new(idgen, store);
        Ok(PollsResource::from_inner(inner))
    }
}

impl<I: Clone + 'static> PollsResource<I> {
    pub fn from_inner(inner: I) -> Self {
        let inner = Arc::new(Mutex::new(inner));
        PollsResource { inner }
    }
}

impl<I: Clone + 'static> PollsResource<I>
where
    I: GenService<CreatePoll, Resp = Id<Poll>>,
    I: GenService<Identified<TallyVotes>, Resp = VoteSummary>,
{
    pub fn configure(&self, cfg: &mut web::ServiceConfig) {
        let scope = web::scope(PREFIX)
            .service(self.create_poll())
            .service(self.show_poll());

        cfg.service(scope);
    }
}

impl<I: Clone + 'static> PollsResource<I>
where
    I: GenService<CreatePoll, Resp = Id<Poll>>,
{
    fn create_poll(&self) -> impl HttpServiceFactory + 'static {
        let me = self.clone();
        let handler = move |(form, req): (
            web::Form<CreatePollForm>,
            HttpRequest,
        )|
         -> Result<_, actix_web::Error> {
            let mut inner = me.inner.lock().expect("unlock");
            let result: Id<Poll> = inner.call(CreatePoll {
                name: form.name.clone(),
            })?;

            let uri = req.url_for("poll", &[result.untyped().to_string()])?;

            Ok(HttpResponse::SeeOther()
                .header("location", uri.to_string())
                .finish())
        };
        web::resource("").route(web::post().to(handler))
    }
}

impl<I: Clone + 'static> PollsResource<I> {
    fn show_poll(&self) -> impl HttpServiceFactory + 'static {
        let handler = move |_me: web::Data<Self>| -> Result<_, actix_web::Error> {
            Ok(WeftResponse::of(WithTemplate { value: PollView }))
        };

        web::resource("/{poll_id}")
            .name("poll")
            .route(web::get().to(handler))
    }
}

#[cfg(test)]
mod tests {
    use actix_web::dev::Service;
    use actix_web::{test, App};
    use failure::Fallible;
    use infra::untyped_ids::UntypedId;
    use serde_urlencoded;
    use url::Url;

    use super::*;

    #[test]
    fn redirect_on_new() -> Fallible<()> {
        env_logger::try_init().unwrap_or_default();

        #[derive(Clone)]
        struct Stub;
        impl GenService<CreatePoll> for Stub {
            type Resp = Id<Poll>;
            fn call(&mut self, req: CreatePoll) -> Fallible<Self::Resp> {
                let CreatePoll { name } = req;

                Ok(Id::hashed(name))
            }
        }

        let resource = PollsResource::from_inner(Stub);

        let mut app = test::init_service(App::new().configure(|cfg| {
            cfg.service(
                web::scope(PREFIX)
                    .service(resource.create_poll())
                    .service(web::resource("/{poll_id}").name("poll")),
            );
        }));

        let name = "Bob";
        let form = CreatePollForm { name: name.into() };

        let req = test::TestRequest::post()
            .uri(&PREFIX)
            .set_payload(serde_urlencoded::to_string(form)?)
            .header("content-type", "application/x-www-form-urlencoded")
            .to_request();
        println!("{:?}", req);
        let resp = test::block_on(app.call(req)).unwrap();
        println!("→ Resp: {:?}", resp);

        let status = resp.status();
        let locationp = resp.headers().get("Location").map(|l| l.clone());

        let body = test::read_body(resp);
        println!("→ Body: {:?}", String::from_utf8_lossy(&body));

        let location = Url::parse(locationp.expect("location header").to_str()?)?;

        assert_eq!(status, 303);
        assert_eq!(
            location.path(),
            format!("{}/{}", PREFIX, UntypedId::hashed(name))
        );

        Ok(())
    }
}
