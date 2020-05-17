#![feature(proc_macro_hygiene)]

use std::fs::OpenOptions;
use std::path::PathBuf;

use clap::{App, Arg};

use fern::colors::{Color, ColoredLevelConfig};

use log::{debug, info, log_enabled};

use longboard::config::{Config, ExtensionConfig, GlobalConfig};
use longboard::{new_instance, Error, Result};

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
            Arg::with_name("extension-dir")
                .short("x")
                .long("extension-dir")
                .value_name("DIR")
                .takes_value(true)
                .help("Directory of extension configs to use"),
        )
        .arg(
            Arg::with_name("log-file")
                .short("l")
                .long("log-file")
                .value_name("FILE")
                .takes_value(true)
                .help("Log file to use (- for stdout)"),
        )
        .arg(
            Arg::with_name("database-uri")
                .short("u")
                .long("database-uri")
                .value_name("URI")
                .takes_value(true)
                .help("URI to use to connect to the database"),
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

    let mut conf_path = GlobalConfig::default_path();
    let mut extension_dir = ExtensionConfig::default_dir();

    if let Some(path) = matches.value_of("config") {
        conf_path = PathBuf::from(path);
        extension_dir = conf_path.parent().expect("bad config path").to_owned();
    }

    if let Some(dir) = matches.value_of("extension-dir") {
        extension_dir = PathBuf::from(dir);
    }

    let mut config = Config::load(&conf_path, extension_dir)?;

    if let Some(path) = matches.value_of("log-file") {
        config.global_config.log_file = match path {
            "-" => None,
            _ => Some(PathBuf::from(path)),
        };
    }

    if let Some(uri) = matches.value_of("database-uri") {
        config.global_config.database_uri = uri.to_string();
    }

    let log_to_file = config.global().log_file.is_some();

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

    match config.global().log_file {
        Some(ref log_path) => {
            let log_file = OpenOptions::new()
                .append(true)
                .create(true)
                .open(log_path)
                .map_err(|cause| Error::IoErrorMsg {
                    cause,
                    msg: format!(
                        "Couldn't open log file at {}",
                        log_path.display()
                    ),
                })?;
            dispatch.chain(log_file).apply()?
        }
        None => dispatch.chain(std::io::stdout()).apply()?,
    };

    info!("Using config file {}", conf_path.display());

    if matches.is_present("debug-config") {
        for line in format!("{:#?}", config).lines() {
            debug!("{}", line);
        }
    }

    Err(Error::from(new_instance(config)?.launch()))
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

        std::process::exit(-1);
    }
}
