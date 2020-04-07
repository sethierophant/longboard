//! Error types.

use log::{error, warn};

use maplit::hashmap;

use rocket::http::Status;
use rocket::{response::Responder, Request};

use rocket_contrib::templates::Template;

use derive_more::{Display, From};

use crate::models::{PostId, ThreadId};

/// Our error type.
#[derive(Debug, Display, From)]
pub enum Error {
    #[display(fmt = "Banned user {} attempted to access page", user_hash)]
    UserIsBanned { user_hash: String },
    #[display(fmt = "Board '{}' not found", board_name)]
    BoardNotFound { board_name: String },
    #[display(
        fmt = "Thread #{} on board '{}' not found",
        thread_id,
        board_name
    )]
    ThreadNotFound {
        board_name: String,
        thread_id: ThreadId,
    },
    #[display(fmt = "Post #{} not found", post_id)]
    PostNotFound { post_id: PostId },
    #[display(fmt = "Missing param '{}' for new thread", param)]
    MissingThreadParam { param: String },
    #[display(fmt = "Missing param '{}' for new post", param)]
    MissingPostParam { param: String },
    #[display(fmt = "Couldn't parse multipart/form-data")]
    FormDataCouldntParse,
    #[display(fmt = "Bad Content-Type for multipart/form-data")]
    FormDataBadContentType,
    #[display(fmt = "Invalid password")]
    DeleteInvalidPassword,
    #[display(fmt = "Deleting files only is not a valid option for threads")]
    CannotDeleteThreadFilesOnly,
    #[display(fmt = "No staff member with username '{}'", user_name)]
    StaffInvalidUsername { user_name: String },
    #[display(fmt = "Invaid password for username '{}'", user_name)]
    StaffInvalidPassword { user_name: String },
    #[display(fmt = "Missing session cookie")]
    MissingSessionCookie,
    #[display(fmt = "Invalid session cookie")]
    InvalidSessionCookie,
    #[display(fmt = "Session expired")]
    ExpiredSession,
    #[display(fmt = "Couldn't create regex: {}", _0)]
    #[from]
    RegexError(regex::Error),
    #[display(fmt = "Error processing image: {}", _0)]
    #[from]
    ImageError(image::error::ImageError),
    #[display(fmt = "Couldn't hash password: {}", _0)]
    #[from]
    HashError(argon2::Error),
    #[display(fmt = "Couldn't render HTML template: {}", _0)]
    #[from]
    RenderError(handlebars::RenderError),
    #[display(fmt = "JSON error: {}", _0)]
    #[from]
    JsonError(serde_json::error::Error),
    #[display(fmt = "YAML error: {}", _0)]
    #[from]
    YamlError(serde_yaml::Error),
    #[display(fmt = "HTML template file error: {}", _0)]
    #[from]
    TemplateError(handlebars::TemplateFileError),
    #[display(fmt = "Couldn't initialize logging: {}", _0)]
    #[from]
    LogError(log::SetLoggerError),
    #[display(fmt = "Database connection pool error: {}", _0)]
    #[from]
    R2d2Error(r2d2::Error),
    #[display(fmt = "Database error: {}", _0)]
    #[from]
    DatabaseError(diesel::result::Error),
    #[display(fmt = "Couldn't connect to the PostgreSQL database: {}", _0)]
    #[from]
    ConnectionError(diesel::ConnectionError),
    #[display(fmt = "I/O error: {}", _0)]
    #[from]
    IoError(std::io::Error),
    #[display(fmt = "I/O error: {}: {}", msg, cause)]
    IoErrorMsg { cause: std::io::Error, msg: String },
    #[display(fmt = "Error parsing duration: {}", _0)]
    #[from]
    DurationParseError(parse_duration::parse::Error),
    #[display(fmt = "Duration out of range: {}", _0)]
    #[from]
    DurationOutOfRangeError(time::OutOfRangeError),
}

impl Error {
    pub fn from_io_error<S>(cause: std::io::Error, msg: S) -> Error
    where
        S: Into<String>,
    {
        Error::IoErrorMsg {
            cause,
            msg: msg.into(),
        }
    }
}

impl<'r> Responder<'r> for Error {
    fn respond_to(self, req: &Request) -> rocket::response::Result<'r> {
        match self {
            Error::MissingThreadParam { .. }
            | Error::MissingPostParam { .. }
            | Error::ImageError(..)
            | Error::DeleteInvalidPassword
            | Error::StaffInvalidUsername { .. }
            | Error::StaffInvalidPassword { .. } => {
                warn!("{}", &self);

                let template = Template::render(
                    "pages/error/400",
                    hashmap! {
                        "message" => self.to_string()
                    },
                );

                let mut res = template.respond_to(req)?;
                res.set_status(Status::BadRequest);

                Ok(res)
            }

            Error::PostNotFound { .. }
            | Error::BoardNotFound { .. }
            | Error::ThreadNotFound { .. } => {
                warn!("{}", &self);

                let template = Template::render(
                    "pages/error/404",
                    hashmap! {
                        "message" => self.to_string()
                    },
                );

                let mut res = template.respond_to(req)?;
                res.set_status(Status::NotFound);

                Ok(res)
            }

            _ => {
                error!("{}", self);

                let template = Template::render("pages/error/500", ());

                let mut res = template.respond_to(req)?;
                res.set_status(Status::InternalServerError);

                Ok(res)
            }
        }
    }
}

impl std::error::Error for Error {}

/// Our result type.
pub type Result<T> = std::result::Result<T, Error>;
