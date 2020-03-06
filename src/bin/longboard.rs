#![feature(proc_macro_hygiene)]

use fern::colors::{Color, ColoredLevelConfig};

use rocket::config::{Config as RocketConfig, Environment, LoggingLevel};
use rocket::routes;

use rocket_contrib::templates::Template;

use longboard::{config::Config, models::Database, LogFairing, Result};

fn main_res() -> Result<()> {
    let colors = ColoredLevelConfig::new()
        .debug(Color::Magenta)
        .info(Color::Green)
        .warn(Color::Yellow)
        .error(Color::Red);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{} [{}] {:>5}",
                chrono::Local::now().format("%F %T%.3f"),
                colors.color(record.level()),
                message
            ))
        })
        .filter(|metadata| metadata.target() == "longboard")
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .apply()?;

    let routes = routes![
        longboard::routes::static_file,
        longboard::routes::upload_file,
        longboard::routes::home,
        longboard::routes::board,
        longboard::routes::thread,
        longboard::routes::new_thread,
        longboard::routes::new_post,
    ];

    let default_config_path = if cfg!(debug_assertions) {
        "contrib/dev-config.yaml"
    } else {
        "/etc/longboard/config.yaml"
    };

    let conf = Config::open(default_config_path)?;

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
