#![cfg(test)]

use failure::Fallible;
use maplit::*;

use super::*;
use crate::testing::*;

#[test]
fn canary() -> Fallible<()> {
    let store = pool("canary")?;
    let idgen = IdGen::new();
    let polls = Polls::new(idgen.clone(), store);

    let poll_id = polls.call(CreatePoll {
        name: "Canary Poll".into(),
    })?;

    {
        let choice = "Banana".into();
        polls.call(RecordVote { poll_id, choice })?;
    }

    let results = polls.call(TallyVotes { poll_id })?;

    assert_eq!(results.tally, hashmap! {"Banana".into() => 1});

    Ok(())
}
