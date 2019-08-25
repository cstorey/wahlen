#![cfg(test)]

use super::*;
use crate::testing::*;

use failure::Fallible;

#[test]
fn canary() -> Fallible<()> {
    let store = pool("canary")?;
    let idgen = IdGen::new();
    let polls = Polls::new(idgen, store);

    let poll = polls.call(CreatePoll {
        name: "Canary Poll".into(),
    })?;

    Ok(())
}
