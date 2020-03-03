#![feature(proc_macro_hygiene)]

use std::path::PathBuf;

use rocket::config::{Config as RocketConfig, Environment};
use rocket::routes;

use rocket_contrib::templates::Template;

use longboard::config::{Banner, Config};
use longboard::{models::Database, Result};

fn main() -> Result<()> {
    let routes = routes![
        longboard::routes::static_file,
        longboard::routes::upload_file,
        longboard::routes::home,
        longboard::routes::board,
        longboard::routes::thread,
        longboard::routes::new_thread,
        longboard::routes::new_post
    ];

    let rocket_conf = RocketConfig::build(Environment::Development)
        .address("0.0.0.0")
        .port(8000)
        .extra("template_dir", "res/templates")
        .finalize()
        .unwrap();

    let app_conf = Config {
        static_dir: PathBuf::from("res/static"),
        upload_dir: PathBuf::from("uploads"),
        banners: vec![
            Banner {
                name: "1.png".into(),
            },
            Banner {
                name: "2.png".into(),
            },
            Banner {
                name: "3.png".into(),
            },
            Banner {
                name: "4.png".into(),
            },
        ],
    };

    rocket::custom(rocket_conf)
        .mount("/", routes)
        .manage(Database::open()?)
        .manage(app_conf)
        .attach(Template::fairing())
        .launch();

    Ok(())
}
