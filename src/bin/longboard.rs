#![feature(proc_macro_hygiene)]

use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use clap::{App, Arg};

use fern::colors::{Color, ColoredLevelConfig};

use log::debug;

use rocket::config::{Config as RocketConfig, Environment, LoggingLevel};
use rocket::routes;

use rocket_contrib::templates::Template;

use longboard::{config::Config, models::Database, LogFairing, Result};

fn main_res() -> Result<()> {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .takes_value(true)
                .help("Config file to use"),
        )
        .arg(
            Arg::with_name("gen-config")
                .long("gen-config")
                .value_name("FILE")
                .takes_value(true)
                .default_value("-")
                .help("Generate a new config file"),
        )
        .get_matches();

    if matches.occurrences_of("gen-config") == 1 {
        let gen_path = matches.value_of("gen-config").unwrap();

        if gen_path == "-" {
            Config::generate(std::io::stdout())?;
        } else {
            Config::generate(File::create(gen_path)?)?;
        }

        return Ok(());
    }

    let conf_path = matches
        .value_of("config")
        .map(PathBuf::from)
        .unwrap_or_else(Config::default_path);
    let conf = Config::open(&conf_path)?;

    let log_to_file = conf.log_file.is_some();

    let dispatch = fern::Dispatch::new()
        .format(move |out, message, record| {
            let colors = ColoredLevelConfig::new()
                .debug(Color::Magenta)
                .info(Color::Green)
                .warn(Color::Yellow)
                .error(Color::Red);

            if log_to_file {
                out.finish(format_args!(
                    "{} [{}] {:>5}",
                    chrono::Local::now().format("%F %T%.3f"),
                    record.level(),
                    message
                ))
            } else {
                out.finish(format_args!(
                    "{} [{}] {:>5}",
                    chrono::Local::now().format("%F %T%.3f"),
                    colors.color(record.level()),
                    message
                ))
            };
        })
        .level(log::LevelFilter::Debug)
        .filter(|metadata| metadata.target().starts_with("longboard"));

    match conf.log_file {
        Some(ref log_path) => {
            let log_file = OpenOptions::new()
                .append(true)
                .create(true)
                .open(log_path)?;
            dispatch.chain(log_file).apply()?
        }
        None => dispatch.chain(std::io::stdout()).apply()?,
    };

    debug!("Using config file {}", conf_path.display());

    let routes = routes![
        longboard::routes::home,
        longboard::routes::static_file,
        longboard::routes::upload_file,
        longboard::routes::report,
        longboard::routes::new_report,
        longboard::routes::board,
        longboard::routes::thread,
        longboard::routes::new_thread,
        longboard::routes::new_post,
    ];

    let rocket_conf = RocketConfig::build(Environment::Development)
        .address(&conf.address)
        .port(conf.port)
        .log_level(LoggingLevel::Off)
        .extra("template_dir", conf.template_dir.display().to_string())
        .finalize()
        .unwrap();

    rocket::custom(rocket_conf)
        .mount("/", routes)
        .manage(Database::open(&conf.database_url)?)
        .manage(conf)
        .attach(Template::fairing())
        .attach(LogFairing)
        .launch();

    Ok(())
}

fn main() {
    if let Err(e) = main_res() {
        eprintln!("{}", e);
    }
}
