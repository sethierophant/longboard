#[macro_use]
extern crate diesel;

use derive_more::{Display, From};

pub mod models;
pub mod schema;

#[derive(Debug, Display, From)]
pub enum Error {
    #[display(fmt="Not found in database: {}", what)]
    NotFoundInDatabase { what: String },
    #[display(fmt="Database error: {}", _0)]
    #[from]
    DatabaseError(diesel::result::Error),
    #[display(fmt="Error connecting to PostgreSQL database: {}", _0)]
    #[from]
    ConnectionError(diesel::ConnectionError),
    #[display(fmt="I/O error: {}", _0)]
    #[from]
    IoError(std::io::Error),
}

impl std::error::Error for Error { }

pub type Result<T> = std::result::Result<T, Error>;
