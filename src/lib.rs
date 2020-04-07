//! An imageboard engine.

#![feature(proc_macro_hygiene)]
#![feature(decl_macro)]
#![feature(never_type)]

#[macro_use]
extern crate diesel;

use std::fmt::Write;
use std::string::ToString;

use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::StatusClass;
use rocket::{Request, Response, Rocket};

use log::{info, warn};

pub mod config;
pub mod error;
pub mod models;
pub mod routes;
pub mod views;

pub use config::Config;
pub use error::{Error, Result};
pub use models::Database;

/// Auto-generated by diesel.
pub mod schema;

/// A rocket fairing for logging about requests.
pub struct LogFairing;

impl Fairing for LogFairing {
    fn info(&self) -> Info {
        Info {
            name: "Logging Fairing",
            kind: Kind::Launch | Kind::Response,
        }
    }

    fn on_launch(&self, rocket: &Rocket) {
        let conf = rocket.config();

        info!("Starting on {}:{}", conf.address, conf.port);
    }

    fn on_response(&self, request: &Request, response: &mut Response) {
        let mut msg = String::new();

        match request.client_ip() {
            Some(ip) => write!(msg, "[{}]", ip).unwrap(),
            None => write!(msg, "[Unknown]").unwrap(),
        }

        write!(msg, " {}", request.method()).unwrap();
        write!(msg, " {}", request.uri().to_string()).unwrap();
        write!(msg, " {}", response.status()).unwrap();

        if let Some(content_type) = response.content_type() {
            write!(msg, " ({})", content_type).unwrap();
        }

        if let Some(referer) = request.headers().get_one("Referer") {
            write!(msg, " Referer \"{}\"", referer).unwrap();
        }

        if let Some(user_agent) = request.headers().get_one("User-Agent") {
            write!(msg, " User-Agent \"{}\"", user_agent).unwrap();
        }

        if let StatusClass::ClientError | StatusClass::ServerError =
            response.status().class()
        {
            warn!("{}", msg);
        } else {
            info!("{}", msg);
        }
    }
}

pub mod sql_types {
    //! Re-exports from `models::sql_types`.
    pub use crate::models::staff::sql_types::Role;
}
