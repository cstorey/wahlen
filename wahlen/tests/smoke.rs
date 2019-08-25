#![cfg(test)]
use std::env;
use std::str::FromStr;

use actix_http::HttpService;
use actix_http_test::{TestServer, TestServerRuntime};
use actix_web::App;
use failure::{Fallible, ResultExt};
use maplit::*;
use sulfur::*;
use sulfur::{chrome, By};

use infra::ids::*;
use wahlen::polls::*;

struct PollsDriver {
    srv: TestServerRuntime,
    browser: DriverHolder,
}

#[test]
#[ignore]
fn canary() -> Fallible<()> {
    let mut polls = PollsDriver::new()?;

    let poll_id = polls.call(CreatePoll {
        name: "Canary Poll".into(),
    })?;

    let subject_id = unimplemented!();

    polls.call(Identified(
        poll_id,
        RecordVote {
            subject_id,
            choice: "Banana".into(),
        },
    ))?;

    let results = polls.call(Identified(poll_id, TallyVotes))?;

    assert_eq!(results.tally, hashmap! {"Banana".into() => 1});

    Ok(())
}

#[cfg(never)]
#[test]
fn two_folks_can_vote() -> Fallible<()> {
    let store = pool("two_folks_can_vote")?;
    let idgen = IdGen::new();
    let mut polls = PollsDriver::new(idgen.clone(), store);

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
    let mut polls = PollsDriver::new(idgen.clone(), store);

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

impl PollsDriver {
    fn new() -> Fallible<Self> {
        let mut config = wahlen::config::Config::default();
        config.postgres.url = env::var("POSTGRES_URL").context("$POSTGRES_URL")?;
        let app = wahlen::Wahlen::new(&config).expect("new rustbucks");

        let srv = TestServer::new(move || {
            HttpService::new(App::new().configure(|cfg| app.configure(cfg)))
        });

        let config = chrome::Config::default();
        let browser = sulfur::chrome::start(&config)?;
        Ok(PollsDriver { srv, browser })
    }
}

impl GenService<CreatePoll> for PollsDriver {
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

        let button = self
            .browser
            .find_element(&By::css("*[data-job='create-poll']"))?;
        self.browser.click(&button)?;

        let meta = self.browser.find_element(&By::css("*[data-page]"))?;
        let page_name = self
            .browser
            .attribute(&meta, "data-page")?
            .ok_or_else(|| failure::err_msg("Expected 'data-page' atttribute"))?;
        assert_eq!(page_name, "poll");
        let poll_id = self
            .browser
            .attribute(&meta, "data-page")?
            .ok_or_else(|| failure::err_msg("Expected 'data-page' atttribute"))?;

        Ok(Id::from_str(&poll_id)?)
    }
}
impl<Req> GenService<Identified<Req>> for PollsDriver
where
    Poll: GenService<Req>,
{
    type Resp = <Poll as GenService<Req>>::Resp;
    fn call(&mut self, req: Identified<Req>) -> Fallible<Self::Resp> {
        unimplemented!()
    }
}
