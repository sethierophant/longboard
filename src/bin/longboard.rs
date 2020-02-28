#![feature(proc_macro_hygiene)]

use rocket::{routes, uri};
use rocket::config::{Config, Environment};

use rocket_contrib::templates::Template;

use longboard::{Result, BannerList};
use longboard::models::Database;

fn main() -> Result<()> {
    let banners = BannerList::new(vec![
        uri!(longboard::routes::static_file: "/banners/1.png").to_string(),
        uri!(longboard::routes::static_file: "/banners/2.png").to_string(),
        uri!(longboard::routes::static_file: "/banners/3.png").to_string(),
        uri!(longboard::routes::static_file: "/banners/4.png").to_string(),
    ]);

    let routes = routes![
        longboard::routes::static_file,
        longboard::routes::home,
        longboard::routes::board,
        longboard::routes::thread,
        longboard::routes::new_thread,
        longboard::routes::new_post
    ];

    let conf = Config::build(Environment::Development)
        .address("0.0.0.0")
        .port(8000)
        .extra("template_dir", "res/templates")
        .finalize().unwrap();

    rocket::custom(conf)
        .mount("/", routes)
        .manage(Database::open()?)
        .manage(banners)
        .attach(Template::fairing())
        .launch();

    Ok(())
}
