use actix;

use failure;

#[macro_use]
extern crate log;
use serde::Deserialize;
use structopt;
use toml;
use wahlen;

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use actix_web::{middleware::Logger, App, HttpServer};
use failure::ResultExt;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "serve", about = "Serve wahlen.")]
struct Opt {
    /// Input file
    #[structopt(parse(from_os_str))]
    config: PathBuf,
}

#[derive(Deserialize, Debug)]
struct Config {
    #[serde(flatten)]
    wahlen: wahlen::config::Config,
    listener: Listener,
    env_logger: wahlen::config::EnvLogger,
}

#[derive(Deserialize, Debug)]
struct Listener {
    addr: std::net::SocketAddr,
}

fn main() -> Result<(), failure::Error> {
    let opt = Opt::from_args();

    let mut config_buf = String::new();
    File::open(&opt.config)?.read_to_string(&mut config_buf)?;
    let config: Config = toml::from_str(&config_buf)?;

    config.env_logger.builder().init();

    let sys = actix::System::new("wahlen-app");
    let rb = wahlen::Wahlen::new(&config.wahlen)?;
    let factory = move || {
        App::new()
            .wrap(Logger::default())
            .configure(|cfg| rb.configure(cfg))
            .service(actix_files::Files::new("/", "wahlen/static/"))
    };
    let srv = HttpServer::new(factory)
        .bind(&config.listener.addr)
        .context("bind")?;
    info!("Listening on: {:?}", srv.addrs());
    srv.start();
    sys.run()?;
    Ok(())
}
