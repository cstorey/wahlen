use failure::Fallible;

use infra::ids::IdGen;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "idgen", about = "Generate Identifiers")]
enum Commands {
    #[structopt(name = "gen", about = "Generate Identifiers")]
    Generate(Generate),
}

#[derive(Debug, StructOpt)]
struct Generate {
    #[structopt(short = "n", long = "count", default_value = "1")]
    count: usize,
}

fn main() -> Fallible<()> {
    let cmd = Commands::from_args();

    match cmd {
        Commands::Generate(opt) => {
            let idgen = IdGen::new();
            for _ in 0..opt.count {
                println!("{}", idgen.untyped());
            }
        }
    }

    Ok(())
}
