//! Routes for user options.

use rocket::http::{hyper::header::Location, Cookie, Status};
use rocket::request::{Form, FromForm, FromRequest, Outcome};
use rocket::{get, post, uri, Request, Response};

use serde::Serialize;

use crate::views::{Context, OptionsPage};
use crate::{Error, Result};

/// Form data for user options.
#[derive(FromForm, Serialize, Clone, Debug)]
pub struct UserOptions {
    pub style: String,
    pub code_highlighting: bool,
}

impl UserOptions {
    fn into_cookies(self) -> Vec<Cookie<'static>> {
        vec![
            Cookie::build("option-style", self.style).path("/").finish(),
            Cookie::build(
                "option-code-highlighting",
                self.code_highlighting.to_string(),
            )
            .path("/")
            .finish(),
        ]
    }
}

impl Default for UserOptions {
    fn default() -> UserOptions {
        UserOptions {
            style: "default".into(),
            code_highlighting: true,
        }
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for UserOptions {
    type Error = Error;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let cookies = request.cookies();

        Outcome::Success(UserOptions {
            style: cookies
                .get("option-style")
                .map(|cookie| cookie.value().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or(UserOptions::default().style),
            code_highlighting: cookies
                .get("option-code-highlighting")
                .map(|cookie| cookie.value().to_string())
                .and_then(|s| s.parse::<bool>().ok())
                .unwrap_or(UserOptions::default().code_highlighting),
        })
    }
}

/// Serve the user options page.
#[get("/options", rank = 0)]
pub fn options(context: Context) -> Result<OptionsPage> {
    OptionsPage::new(&context)
}

/// Update user options.
#[post("/options", rank = 0, data = "<user_options>")]
pub fn update_options<'r>(
    user_options: Form<UserOptions>,
) -> Result<Response<'r>> {
    let mut res = Response::build();

    res.status(Status::SeeOther);
    res.header(Location(uri!(crate::routes::options::options).to_string()));

    for cookie in user_options.into_inner().into_cookies() {
        res.header_adjoin(cookie);
    }

    Ok(res.finalize())
}
