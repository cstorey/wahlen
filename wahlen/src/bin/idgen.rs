use failure::Fallible;

use chrono::{DateTime, SecondsFormat, Utc};
use infra::ids::IdGen;
use infra::untyped_ids::UntypedId;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "idgen", about = "Generate Identifiers")]
enum Commands {
    #[structopt(name = "gen", about = "Generate Identifiers")]
    Generate(Generate),
    #[structopt(name = "decompose", about = "Decompose Identifiers")]
    Decompose(Decompose),
}

#[derive(Debug, StructOpt)]
struct Generate {
    #[structopt(short = "n", long = "count", default_value = "1")]
    count: usize,
}

#[derive(Debug, StructOpt)]
struct Decompose {
    ids: Vec<UntypedId>,
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
        Commands::Decompose(opt) => {
            for id in opt.ids {
                let stamp: DateTime<Utc> = id.timestamp().into();
                let random = id.random();
                println!(
                    "t:{}; r:0x{:0>16x}",
                    stamp.to_rfc3339_opts(SecondsFormat::Nanos, true),
                    random
                );
            }
        }
    }

    Ok(())
}
