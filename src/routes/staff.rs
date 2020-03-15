//! Routes for staff member actions.

use argon2::verify_encoded;

use chrono::{Duration, Utc};

use rand::{distributions::Alphanumeric, thread_rng, Rng};

use rocket::http::{hyper::header::Location, Cookie, Cookies, Status};
use rocket::request::{Form, FromForm, FromRequest, Outcome, Request};
use rocket::response::Response;
use rocket::{get, post, uri, State};

use crate::models::*;
use crate::views::staff::*;
use crate::{Error, Result};

impl<'a, 'r> FromRequest<'a, 'r> for Session {
    type Error = Error;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let db = request
            .guard::<State<Database>>()
            .expect("Expected Database to be in managed state");
        let cookies = request.cookies();

        let err = Err((Status::Forbidden, Error::MissingSessionCookie));
        let session_id = cookies.get("session").ok_or(err)?.value();

        let err = Err((Status::Forbidden, Error::InvalidSessionCookie));
        let session = db.session(session_id).map_err(|_| err)?;

        Outcome::Success(session)
    }
}

#[get("/staff/login")]
pub fn login() -> Result<LoginPage> {
    LoginPage::new()
}

#[derive(FromForm)]
pub struct LoginData {
    user: String,
    pass: String,
}

#[post("/staff/login", data = "<login_data>")]
pub fn handle_login<'r>(login_data: Form<LoginData>, db: State<Database>) -> Result<Response<'r>> {
    let user = db.staff(&login_data.user)?;

    if !verify_encoded(&user.password_hash, login_data.pass.as_bytes())? {
        return Err(Error::StaffInvalidPassword {
            user_name: login_data.user.clone(),
        });
    }

    let id: String = thread_rng().sample_iter(Alphanumeric).take(42).collect();

    let expires = Utc::now() + Duration::weeks(1);

    let session_cookie = Cookie::build("session", id.clone()).finish();

    db.insert_session(Session {
        id,
        expires,
        staff_name: user.name,
    })?;

    Ok(Response::build()
        .status(Status::SeeOther)
        .header(Location(uri!(crate::routes::staff::overview).to_string()))
        .header(session_cookie)
        .finalize())
}

#[get("/staff/logout")]
pub fn logout<'r>(
    session: Session,
    mut cookies: Cookies,
    db: State<Database>,
) -> Result<Response<'r>> {
    if let Some(cookie) = cookies.get("session").cloned() {
        cookies.remove(cookie)
    }

    db.delete_session(session.id)?;

    Ok(Response::build()
        .status(Status::SeeOther)
        .header(Location(uri!(crate::routes::staff::login).to_string()))
        .finalize())
}

#[get("/staff")]
pub fn overview(session: Session, db: State<Database>) -> Result<OverviewPage> {
    OverviewPage::new(session.staff_name, &db)
}

#[get("/staff/log")]
pub fn log(_session: Session) -> Result<LogPage> {
    LogPage::new()
}
