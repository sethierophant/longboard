#![feature(proc_macro_hygiene)]

use std::fs::OpenOptions;

use clap::{App, Arg};

use fern::colors::{Color, ColoredLevelConfig};

use log::{debug, info};

use longboard::{new_instance, Config, Result};

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
            Arg::with_name("log-all")
                .short("a")
                .long("log-all")
                .help("Show all log messages, this makes the log very messy"),
        )
        .arg(
            Arg::with_name("debug-config")
                .short("d")
                .long("debug-config")
                .help("Dump the configuration to the log on startup"),
        )
        .get_matches();

    let conf = if let Some(path) = matches.value_of("config") {
        Config::new(path)?
    } else {
        Config::new_default()?
    };

    let log_to_file = conf.log_file.is_some();

    let log_all = matches.is_present("log-all");

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
                    message,
                ))
            } else {
                out.finish(format_args!(
                    "{} [{}] {:>5}",
                    chrono::Local::now().format("%F %T%.3f"),
                    colors.color(record.level()),
                    message,
                ))
            };
        })
        .level(log::LevelFilter::Debug)
        .filter(move |metadata| {
            metadata.target().starts_with("longboard") || log_all
        });

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

    if let Some(path) = matches.value_of("config") {
        info!("Using config file {}", path);
    } else {
        info!("Using config file {}", Config::default_path().display());
    };

    if matches.is_present("debug-config") {
        for line in format!("{:#?}", conf).lines() {
            debug!("{}", line);
        }
    }

    Err(Error::from(new_instance(conf)?.launch()))
}

fn main() {
    if let Err(e) = main_res() {
        if log_enabled!(log::Level::Error) {
            log::error!("{}", e);
        } else {
            // If an error occured before the log has been set up, write it to
            // stderr.
            eprintln!("{}", e);
        }
    }
}
