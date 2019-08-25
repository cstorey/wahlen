#![cfg(test)]

use failure::Fallible;
use maplit::*;

use super::*;
use crate::testing::*;

#[test]
fn canary() -> Fallible<()> {
    let store = pool("canary")?;
    let idgen = IdGen::new();
    let mut polls = Polls::new(idgen.clone(), store);

    let poll_id = polls.call(CreatePoll {
        name: "Canary Poll".into(),
    })?;

    polls.call(RecordVote {
        poll_id,
        subject_id: idgen.generate(),
        choice: "Banana".into(),
    })?;

    let results = polls.call(TallyVotes { poll_id })?;

    assert_eq!(results.tally, hashmap! {"Banana".into() => 1});

    Ok(())
}

#[test]
fn two_folks_can_vote() -> Fallible<()> {
    let store = pool("two_folks_can_vote")?;
    let idgen = IdGen::new();
    let mut polls = Polls::new(idgen.clone(), store);

    let poll_id = polls.call(CreatePoll {
        name: "Canary Poll".into(),
    })?;

    polls.call(RecordVote {
        poll_id,
        subject_id: idgen.generate(),
        choice: "Banana".into(),
    })?;
    polls.call(RecordVote {
        poll_id,
        subject_id: idgen.generate(),
        choice: "Chocolate".into(),
    })?;

    let results = polls.call(TallyVotes { poll_id })?;

    assert_eq!(
        results.tally,
        hashmap! {"Banana".into() => 1, "Chocolate".into() => 1}
    );

    Ok(())
}

#[test]
fn two_voting_twice_changes_vote() -> Fallible<()> {
    let store = pool("two_voting_twice_changes_vote")?;
    let idgen = IdGen::new();
    let mut polls = Polls::new(idgen.clone(), store);

    let poll_id = polls.call(CreatePoll {
        name: "Canary Poll".into(),
    })?;

    let subject_id = idgen.generate();

    polls.call(RecordVote {
        poll_id,
        subject_id,
        choice: "Banana".into(),
    })?;
    polls.call(RecordVote {
        poll_id,
        subject_id,
        choice: "Chocolate".into(),
    })?;

    let results = polls.call(TallyVotes { poll_id })?;

    assert_eq!(results.tally, hashmap! {"Chocolate".into() => 1});

    Ok(())
}
