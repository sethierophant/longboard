#![feature(proc_macro_hygiene)]
#![feature(decl_macro)]

#[macro_use]
extern crate diesel;

use derive_more::{Display, From};

use maplit::hashmap;

use rand::{thread_rng, seq::SliceRandom};

use rocket::{Request, response::Responder, http::Status};

use rocket_contrib::templates::Template;

pub mod routes;
pub mod models;
pub mod schema;

#[derive(Debug, Display, From)]
pub enum Error {
    #[display(fmt="Missing param '{}' for new thread.", param)]
    MissingThreadParam { param: String },
    #[display(fmt="Missing param '{}' for new post.", param)]
    MissingPostParam { param: String },
    #[display(fmt="Couldn't parse multipart/form-data")]
    FormDataCouldntParse,
    #[display(fmt="Bad Content-Type for multipart/form-data")]
    FormDataBadContentType,
    #[display(fmt="Error hashing password: {}", _0)]
    #[from]
    HashError(argon2::Error),
    #[display(fmt="Render error in HTML template: {}", _0)]
    #[from]
    RenderError(handlebars::RenderError),
    #[display(fmt="JSON error: {}", _0)]
    #[from]
    JsonError(serde_json::error::Error),
    #[display(fmt="HTML template file error: {}", _0)]
    #[from]
    TemplateError(handlebars::TemplateFileError),
    #[display(fmt="Database connection pool error: {}", _0)]
    #[from]
    R2d2Error(r2d2::Error),
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

impl<'r> Responder<'r> for Error {
    fn respond_to(self, req: &Request) -> rocket::response::Result<'r> {
        match self {
            Error::MissingThreadParam { .. } |
                Error::MissingPostParam { .. } =>
            {
                let data = hashmap!{ "message" => self.to_string() };
                let mut res =
                    Template::render("layout/400", &data).respond_to(req)?;
                res.set_status(Status::BadRequest);

                Ok(res)
            },
            _ => {
                Err(Status::InternalServerError)
            }
        }
    }
}

impl std::error::Error for Error { }

pub type Result<T> = std::result::Result<T, Error>;

pub struct BannerList {
    pub banners: Vec<String>,
}

impl BannerList {
    pub fn new<B: Into<Vec<String>>>(banners: B) -> BannerList {
        let banners = banners.into();

        if banners.is_empty() {
            panic!("banner list cannot be empty");
        }

        BannerList { banners: banners.into() }
    }

    pub fn choose(&self) -> &str {
        let mut rng = thread_rng();
        &self.banners.choose(&mut rng).unwrap()
    }
}
