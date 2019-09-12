use failure::Fallible;

use infra::ids::IdGen;

fn main() -> Fallible<()> {
    let idgen = IdGen::new();
    let id = idgen.untyped();
    println!("{}", id);

    Ok(())
}