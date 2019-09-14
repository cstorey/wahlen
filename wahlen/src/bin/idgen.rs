use failure::Fallible;

use infra::ids::IdGen;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "idgen", about = "Generate Identifiers")]
struct Generate {
    #[structopt(short = "n", long = "count", default_value = "1")]
    count: usize,
}

fn main() -> Fallible<()> {
    let opt = Generate::from_args();

    let idgen = IdGen::new();
    for _ in 0..opt.count {
        println!("{}", idgen.untyped());
    }

    Ok(())
}
