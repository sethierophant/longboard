//! Error types.

use std::net::IpAddr;
use std::process::ExitStatus;

use log::{error, warn};

use mime::Mime;

use rocket::http::{hyper::header::Location, Status};
use rocket::response::{Responder, Response};
use rocket::{uri, Request};

use derive_more::{Display, From};

use crate::models::{staff::Role, PostId, ThreadId};
use crate::views::error::*;
use crate::views::Context;

/// Our error type.
#[derive(Debug, Display, From)]
pub enum Error {
    #[display(fmt = "A database operation was called on a mock database")]
    DatabaseIsMock,
    #[display(fmt = "The IP {} was found in the server block list", ip)]
    IpIsBlocked { ip: IpAddr },
    #[display(fmt = "The IP {} was found in {} ({})", ip, dnsbl, result)]
    IpIsBlockedDnsbl {
        dnsbl: String,
        result: IpAddr,
        ip: IpAddr,
    },
    #[display(fmt = "File uploads are not allowed.")]
    FileUploadNotAllowed,
    #[display(fmt = "Banned user {} attempted to access page", user_hash)]
    UserIsBanned { user_hash: String },
    #[display(fmt = "Rate limit exceeded for IP.")]
    UserRateLimitExceeded,
    #[display(fmt = "Rate limit exceeded for post content.")]
    ContentRateLimitExceeded,
    #[display(fmt = "User with ip {} was not found in the database", ip_addr)]
    UserNotFound { ip_addr: IpAddr },
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
    #[display(fmt = "Custom page {} not found", name)]
    CustomPageNotFound { name: String },
    #[display(fmt = "Missing param '{}' for new thread", param)]
    MissingThreadParam { param: String },
    #[display(fmt = "Missing param '{}' for new post", param)]
    MissingPostParam { param: String },
    #[display(fmt = "Couldn't parse multipart/form-data")]
    FormDataCouldntParse,
    #[display(fmt = "Bad content type for multipart/form-data")]
    FormDataBadContentType,
    #[display(fmt = "Uploaded file was missing a content type")]
    UploadMissingContentType,
    #[display(
        fmt = "Unsupported content type {} for uploaded files",
        content_type
    )]
    UploadBadContentType { content_type: Mime },
    #[display(fmt = "Invalid password")]
    DeleteInvalidPassword,
    #[display(fmt = "Deleting files only is not a valid option for threads")]
    CannotDeleteThreadFilesOnly,
    #[display(fmt = "No staff member with username '{}'", staff_name)]
    StaffInvalidUsername { staff_name: String },
    #[display(fmt = "Invaid password for username '{}'", staff_name)]
    StaffInvalidPassword { staff_name: String },
    #[display(fmt = "Missing session cookie")]
    MissingSessionCookie,
    #[display(fmt = "Invalid session cookie")]
    InvalidSessionCookie,
    #[display(fmt = "Session expired")]
    ExpiredSession,
    #[display(fmt = "Report length was more than the max of 250 characters")]
    ReportTooLong,
    #[display(fmt = "Cannot add a post to a locked thread")]
    ThreadLocked,
    #[display(fmt = "Tried to access a staff page without authentication")]
    NotAuthenticated,
    #[display(fmt = "Banner dir is empty")]
    BannerDirEmpty,
    #[display(fmt = "Names file is empty")]
    NamesFileEmpty,
    #[display(fmt = "Path for {} at {} does not exist", name, path)]
    ConfigPathNotFound { name: String, path: String },
    #[display(fmt = "Unknown role: {}", role)]
    UnknownRole { role: String },
    #[display(
        fmt = "Staff member {} does not have the authority of {}",
        staff_name,
        role
    )]
    UnauthorizedRole { staff_name: String, role: Role },
    #[display(fmt = "ffmpeg returned {}: {}", status, stderr)]
    FfmpegError {
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
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
    #[display(fmt = "Database migration error: {}", _0)]
    #[from]
    DatabaseMigrationError(diesel_migrations::RunMigrationsError),
    #[display(fmt = "Couldn't connect to the PostgreSQL database: {}", _0)]
    #[from]
    ConnectionError(diesel::ConnectionError),
    #[display(fmt = "Failed to start: {}", _0)]
    #[from]
    LaunchError(rocket::error::LaunchError),
    #[display(fmt = "Couldn't parse post: {}", _0)]
    #[from]
    ParseError(String),
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
    /// Convert from an I/O error with an added message.
    ///
    /// This is useful e.g. when opening a file; the error messages from the
    /// standard library are usually not helpful at all.
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
            | Error::StaffInvalidPassword { .. }
            | Error::ReportTooLong
            | Error::ThreadLocked
            | Error::FileUploadNotAllowed
            | Error::CannotDeleteThreadFilesOnly => {
                warn!("{}", &self);

                let context = req.guard::<Context>().unwrap();
                let page = BadRequestPage::new(self.to_string(), &context);

                let mut res = page.respond_to(req)?;
                res.set_status(Status::BadRequest);

                Ok(res)
            }

            Error::IpIsBlocked { .. } | Error::IpIsBlockedDnsbl { .. } => {
                // We don't want to show the client the exact error message, as
                // it may contain information that we don't want spammers to
                // know.
                //
                // The error message is still logged to help administrators
                // troubleshoot issues.

                warn!("{}", &self);

                let context = req.guard::<Context>().unwrap();
                let page = SpamDetectedPage::new(
                    "Your IP address was found in a block list.".to_string(),
                    &context,
                );

                let mut res = page.respond_to(req)?;
                res.set_status(Status::Forbidden);

                Ok(res)
            }

            Error::UserRateLimitExceeded | Error::ContentRateLimitExceeded => {
                // Same as for IP being blocked; we don't want to show the exact
                // error message to the client.

                warn!("{}", &self);

                let context = req.guard::<Context>().unwrap();
                let page = SpamDetectedPage::new(
                    "Rate limit exceeded.".to_string(),
                    &context,
                );

                let mut res = page.respond_to(req)?;
                res.set_status(Status::Forbidden);

                Ok(res)
            }

            Error::PostNotFound { .. }
            | Error::BoardNotFound { .. }
            | Error::ThreadNotFound { .. }
            | Error::CustomPageNotFound { .. } => {
                warn!("{}", &self);

                let context = req.guard::<Context>().unwrap();
                let page = NotFoundPage::new(self.to_string(), &context);

                let mut res = page.respond_to(req)?;
                res.set_status(Status::NotFound);

                Ok(res)
            }

            Error::NotAuthenticated => {
                // If the client isn't authenticated, just redirect them to the
                // staff login page.

                let login_uri = uri!(crate::routes::staff::login);

                Ok(Response::build()
                    .status(Status::SeeOther)
                    .header(Location(login_uri.to_string()))
                    .finalize())
            }

            _ => {
                error!("{}", self);

                let context = req.guard::<Context>().unwrap();
                let page =
                    InternalServerErrorPage::new(self.to_string(), &context);

                let mut res = page.respond_to(req)?;
                res.set_status(Status::InternalServerError);

                Ok(res)
            }
        }
    }
}

impl std::error::Error for Error {}

/// Our result type.
pub type Result<T> = std::result::Result<T, Error>;
