use std::sync::{Arc, Mutex};

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use failure::Fallible;

use super::*;
const PREFIX: &str = "/polls";

#[derive(Debug, Clone)]
pub struct PollsResource<I> {
    inner: Arc<Mutex<I>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreatePollForm {
    name: String,
}

impl<S: Clone + Storage + 'static> PollsResource<Polls<S>> {
    pub fn new(idgen: IdGen, store: S) -> Fallible<Self> {
        let inner = Polls::new(idgen, store);
        Ok(PollsResource::from_inner(inner))
    }
}

impl<I: Clone + 'static> PollsResource<I>
where
    I: GenService<CreatePoll, Resp = Id<Poll>>,
{
    pub fn from_inner(inner: I) -> Self {
        let inner = Arc::new(Mutex::new(inner));
        PollsResource { inner }
    }
    pub fn configure(&self, cfg: &mut web::ServiceConfig) {
        cfg.data(self.clone());
        let scope = web::scope(PREFIX)
            .service({ web::resource("/").route(web::post().to(Self::create_poll)) })
            .service({ web::resource("/{poll_id}").name("poll").route(web::get()) });

        cfg.service(scope);
    }

    fn create_poll(
        me: web::Data<Self>,
        form: web::Form<CreatePollForm>,
        req: HttpRequest,
    ) -> Result<impl Responder, actix_web::Error> {
        let mut inner = me.inner.lock().expect("unlock");
        let result: Id<Poll> = inner.call(CreatePoll {
            name: form.name.clone(),
        })?;

        let uri = req.url_for("poll", &[result.untyped().to_string()])?;

        Ok(HttpResponse::SeeOther()
            .header("location", uri.to_string())
            .finish())
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

        let mut app = test::init_service(App::new().configure(|c| resource.configure(c)));

        let name = "Bob";
        let form = CreatePollForm { name: name.into() };

        let req = test::TestRequest::post()
            .uri(&format!("{}/", PREFIX))
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
