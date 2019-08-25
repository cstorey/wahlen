#![cfg(test)]

use failure::Fallible;

use crate::testing::*;
use infra::ids::IdGen;

use super::*;

#[test]
fn should_create_a_subject() -> Fallible<()> {
    let store = pool("should_create_a_subject")?;
    let idgen = IdGen::new();
    let mut polls = Subjects::new(idgen.clone(), store);

    let subject_id = polls.call(CreateSubject)?;

    println!("{}", subject_id);
    Ok(())
}
