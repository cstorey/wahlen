#![cfg(test)]
use std::env;
use std::str::FromStr;

use actix_http::HttpService;
use actix_http_test::{TestServer, TestServerRuntime};
use actix_web::middleware::Logger;
use actix_web::App;
use failure::{Fallible, ResultExt};
use sulfur::*;
use sulfur::{chrome, By};

use infra::ids::*;
use wahlen::gen_service::*;
use wahlen::polls::*;
use wahlen::subjects::{CreateSubject, Subject};

struct Driver {
    srv: TestServerRuntime,
    browser: DriverHolder,
}

#[test]
fn canary() -> Fallible<()> {
    env_logger::try_init().unwrap_or_default();
    let mut polls = Driver::new()?;

    let _poll_id = polls.call(CreatePoll {
        name: "Canary Poll".into(),
    })?;

    let _subject_id = polls.call(CreateSubject)?;

    #[cfg(todo)]
    {
        polls.call(Identified(
            poll_id,
            RecordVote {
                subject_id,
                choice: "Banana".into(),
            },
        ))?;

        let results = polls.call(Identified(poll_id, TallyVotes))?;

        assert_eq!(results.tally, hashmap! {"Banana".into() => 1});
    }

    Ok(())
}

#[cfg(never)]
#[test]
fn two_folks_can_vote() -> Fallible<()> {
    let store = pool("two_folks_can_vote")?;
    let idgen = IdGen::new();
    let mut polls = Driver::new(idgen.clone(), store);

    let poll_id = polls.call(CreatePoll {
        name: "Canary Poll".into(),
    })?;

    polls.call(Identified(
        poll_id,
        RecordVote {
            subject_id: idgen.generate(),
            choice: "Banana".into(),
        },
    ))?;
    polls.call(Identified(
        poll_id,
        RecordVote {
            subject_id: idgen.generate(),
            choice: "Chocolate".into(),
        },
    ))?;

    let results = polls.call(Identified(poll_id, TallyVotes))?;

    assert_eq!(
        results.tally,
        hashmap! {"Banana".into() => 1, "Chocolate".into() => 1}
    );

    Ok(())
}

#[cfg(never)]
#[test]
fn two_voting_twice_changes_vote() -> Fallible<()> {
    let store = pool("two_voting_twice_changes_vote")?;
    let idgen = IdGen::new();
    let mut polls = Driver::new(idgen.clone(), store);

    let poll_id = polls.call(CreatePoll {
        name: "Canary Poll".into(),
    })?;

    let subject_id = idgen.generate();

    polls.call(Identified(
        poll_id,
        RecordVote {
            subject_id,
            choice: "Banana".into(),
        },
    ))?;
    polls.call(Identified(
        poll_id,
        RecordVote {
            subject_id,
            choice: "Chocolate".into(),
        },
    ))?;

    let results = polls.call(Identified(poll_id, TallyVotes))?;

    assert_eq!(results.tally, hashmap! {"Chocolate".into() => 1});

    Ok(())
}

impl Driver {
    fn new() -> Fallible<Self> {
        let mut config = wahlen::config::Config::default();
        config.postgres.url = env::var("POSTGRES_URL").context("$POSTGRES_URL")?;
        let app = wahlen::Wahlen::new(&config).expect("new rustbucks");

        let srv = TestServer::new(move || {
            HttpService::new(
                App::new()
                    .wrap(Logger::default())
                    .configure(|cfg| app.configure(cfg)),
            )
        });

        let config = chrome::Config::default();
        let browser = sulfur::chrome::start(&config)?;
        Ok(Driver { srv, browser })
    }
}

impl GenService<CreatePoll> for Driver {
    type Resp = Id<Poll>;
    fn call(&mut self, req: CreatePoll) -> Fallible<Self::Resp> {
        let url = format!("http://{}/", self.srv.addr());
        self.browser.visit(&url)?;

        let meta = self.browser.find_element(&By::css("*[data-page]"))?;
        let page_name = self
            .browser
            .attribute(&meta, "data-page")?
            .ok_or_else(|| failure::err_msg("Expected 'data-page' atttribute"))?;
        assert_eq!(page_name, "top");

        eprintln!("Creating poll");
        let poll_name_elt = self
            .browser
            .find_element(&By::css("input[data-poll-name]"))?;
        self.browser.send_keys(&poll_name_elt, &req.name)?;

        let button = self
            .browser
            .find_element(&By::css("*[data-job='create-poll']"))?;
        self.browser.click(&button)?;
        eprintln!("Clicked button");

        let meta = self.browser.find_element(&By::css("*[data-page]"))?;
        let page_name = self
            .browser
            .attribute(&meta, "data-page")?
            .ok_or_else(|| failure::err_msg("Expected 'data-page' atttribute"))?;
        assert_eq!(page_name, "poll");
        let poll_id = self
            .browser
            .attribute(&meta, "data-poll-id")?
            .ok_or_else(|| failure::err_msg("Expected 'data-page' attribute"))?;

        let poll_name_elt = self.browser.find_element(&By::css("*[data-poll-name]"))?;
        let text = self.browser.text(&poll_name_elt)?;
        assert!(
            text.contains(&req.name),
            "Page name text {:?} should contain passed {:?}",
            text,
            req.name
        );

        Ok(Id::from_str(&poll_id)?)
    }
}

impl GenService<CreateSubject> for Driver {
    type Resp = Id<Subject>;
    fn call(&mut self, _: CreateSubject) -> Fallible<Self::Resp> {
        let url = format!("http://{}/", self.srv.addr());
        self.browser.visit(&url)?;

        let meta = self.browser.find_element(&By::css("*[data-page]"))?;
        let page_name = self
            .browser
            .attribute(&meta, "data-page")?
            .ok_or_else(|| failure::err_msg("Expected 'data-page' atttribute"))?;
        assert_eq!(page_name, "top");

        let button = self
            .browser
            .find_element(&By::css("*[data-job='create-subject']"))?;
        self.browser.click(&button)?;
        eprintln!("Clicked button");

        let meta = self.browser.find_element(&By::css("*[data-page]"))?;
        let page_name = self
            .browser
            .attribute(&meta, "data-page")?
            .ok_or_else(|| failure::err_msg("Expected 'data-page' atttribute"))?;
        assert_eq!(page_name, "subject");

        let subject_id = self
            .browser
            .attribute(&meta, "data-subject-id")?
            .ok_or_else(|| failure::err_msg("Expected 'data-subject-id' attribute"))?;

        Ok(Id::from_str(&subject_id)?)
    }
}

impl<Req> GenService<Identified<Req>> for Driver
where
    Poll: GenService<Req>,
{
    type Resp = <Poll as GenService<Req>>::Resp;
    fn call(&mut self, _req: Identified<Req>) -> Fallible<Self::Resp> {
        unimplemented!()
    }
}
