//! Routes for staff member actions.

use argon2::verify_encoded;

use chrono::{Duration, Utc};

use rand::{distributions::Alphanumeric, thread_rng, Rng};

use rocket::http::{hyper::header::Location, Cookie, Cookies, RawStr, Status};
use rocket::request::{
    Form, FromForm, FromFormValue, FromRequest, Outcome, Request,
};
use rocket::response::Response;
use rocket::{get, post, uri, State};

use crate::models::*;
use crate::views::staff::*;
use crate::views::ActionSuccessPage;
use crate::{Error, Result};

impl<'a, 'r> FromRequest<'a, 'r> for Session {
    type Error = Error;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let db = request
            .guard::<State<Database>>()
            .expect("expected database to be initialized");
        let cookies = request.cookies();

        let err = Err((Status::Forbidden, Error::MissingSessionCookie));
        let session_id = cookies.get("session").ok_or(err)?.value();

        let err = Err((Status::Forbidden, Error::InvalidSessionCookie));
        let session = db.session(session_id).map_err(|_| err)?;

        Outcome::Success(session)
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for User {
    type Error = Error;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let db = request
            .guard::<State<Database>>()
            .expect("expected database to be initialized");

        // If we are using a local request (i.e., if we're running a test) then
        // we might not have an IP address. In production, all requests should
        // have an IP address.
        let ip = if cfg!(debug_assertions) {
            request.client_ip().unwrap_or("1.2.3.4".parse().unwrap())
        } else {
            request.client_ip().expect("expected client to have ip")
        };

        match db.user(ip) {
            Ok(user) => {
                if user.is_banned() {
                    Outcome::Failure((
                        Status::Forbidden,
                        Error::UserIsBanned {
                            user_hash: user.hash,
                        },
                    ))
                } else {
                    Outcome::Success(user)
                }
            },
            Err(Error::DatabaseError(diesel::result::Error::NotFound)) => {
                let new_user = NewUser::from_ip(ip);

                let user = db
                    .insert_user(&new_user)
                    .map_err(|err| Err((Status::InternalServerError, err)))?;

                Outcome::Success(user)
            },
            Err(e) => {
                Outcome::Failure((Status::InternalServerError, e))
            },
        }
    }
}

/// Serve the login page for staff members.
#[get("/staff/login")]
pub fn login(_user: User) -> Result<LoginPage> {
    LoginPage::new()
}

/// Login form data.
#[derive(FromForm)]
pub struct LoginData {
    user: String,
    pass: String,
}

/// Login as a staff member.
#[post("/staff/login", data = "<login_data>")]
pub fn handle_login<'r>(
    login_data: Form<LoginData>,
    db: State<Database>,
) -> Result<Response<'r>> {
    let user = db.staff(&login_data.user)?;

    if !verify_encoded(&user.password_hash, login_data.pass.as_bytes())? {
        return Err(Error::StaffInvalidPassword {
            user_name: login_data.user.clone(),
        });
    }

    let id: String = thread_rng().sample_iter(Alphanumeric).take(42).collect();

    let expires = Utc::now() + Duration::weeks(1);

    let session_cookie = Cookie::build("session", id.clone())
        .path("/")
        .http_only(true)
        .finish();

    db.insert_session(&Session {
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

/// Logout as a staff member.
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

/// Serve the overview for staff actions.
#[get("/staff")]
pub fn overview(session: Session, db: State<Database>) -> Result<OverviewPage> {
    OverviewPage::new(session.staff_name, &db)
}

/// Serve the history for staff actions.
#[get("/staff/history")]
pub fn history(_session: Session, _user: User) -> Result<HistoryPage> {
    HistoryPage::new()
}

/// Form data for closing a report.
#[derive(FromForm)]
pub struct CloseReportData {
    pub id: ReportId,
}

/// Close a report.
#[post("/staff/close-report", data = "<close_data>")]
pub fn close_report(
    close_data: Form<CloseReportData>,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let CloseReportData { id } = close_data.into_inner();

    db.delete_report(id)?;

    let msg = format!("Closed report {} successfully.", id);
    Ok(ActionSuccessPage::new(msg, uri!(overview).to_string()))
}

/// Form data for creating a board.
#[derive(FromForm)]
pub struct CreateBoardData {
    pub name: String,
    pub description: String,
}

/// Create a board.
#[post("/staff/create-board", data = "<create_data>")]
pub fn create_board(
    create_data: Form<CreateBoardData>,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let CreateBoardData { name, description } = create_data.into_inner();

    let msg = format!("Created board \"{}\" successfully.", name);

    db.insert_board(Board { name, description })?;

    Ok(ActionSuccessPage::new(msg, uri!(overview).to_string()))
}

/// Form data for editing a board.
#[derive(FromForm)]
pub struct EditBoardData {
    pub name: String,
    pub description: String,
}

/// Edit a board.
#[post("/staff/edit-board", data = "<edit_data>")]
pub fn edit_board(
    edit_data: Form<EditBoardData>,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let EditBoardData { name, description } = edit_data.into_inner();

    let msg = format!("Edited board \"{}\" successfully.", name);

    db.update_board(name, description)?;

    Ok(ActionSuccessPage::new(msg, uri!(overview).to_string()))
}

/// Form data for deleting a board.
#[derive(FromForm)]
pub struct DeleteBoardData {
    pub name: String,
}

/// Delete a board.
#[post("/staff/delete-board", data = "<delete_data>")]
pub fn delete_board(
    delete_data: Form<DeleteBoardData>,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let DeleteBoardData { name } = delete_data.into_inner();

    let msg = format!("Deleted board \"{}\" successfully.", name);

    db.delete_board(name)?;

    Ok(ActionSuccessPage::new(msg, uri!(overview).to_string()))
}

/// Helper type for the duration a user is banned for.
struct BanDuration(Duration);

impl<'v> FromFormValue<'v> for BanDuration {
    type Error = Error;

    fn from_form_value(
        form_value: &'v RawStr,
    ) -> std::result::Result<BanDuration, Self::Error> {
        let s = String::from_form_value(form_value).unwrap();

        Ok(BanDuration(Duration::from_std(parse_duration::parse(&s)?)?))
    }
}

/// Form data for banning a user.
#[derive(FromForm)]
pub struct BanUserData {
    id: UserId,
    duration: BanDuration,
}

/// Ban a user.
#[post("/staff/ban-user", data = "<ban_data>")]
pub fn ban_user(
    ban_data: Form<BanUserData>,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let BanUserData {
        id,
        duration: BanDuration(duration),
    } = ban_data.into_inner();

    let msg = format!("Banned user {} successfully.", id);

    db.ban_user(id, duration)?;

    Ok(ActionSuccessPage::new(msg, uri!(overview).to_string()))
}

/// Form data for adding a note to a user.
#[derive(FromForm)]
pub struct AddNoteData {
    id: UserId,
    note: String,
}

/// Add a note to a user.
#[post("/staff/add-note", data = "<note_data>")]
pub fn add_note(
    note_data: Form<AddNoteData>,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let AddNoteData { id, note } = note_data.into_inner();

    db.set_user_note(id, note)?;

    let msg = "Added note successfully.".to_string();
    Ok(ActionSuccessPage::new(msg, uri!(overview).to_string()))
}

/// Form data for removing a note from a user.
#[derive(FromForm)]
pub struct RemoveNoteData {
    id: UserId,
}

/// Remove a note from a user.
#[post("/staff/remove-note", data = "<note_data>")]
pub fn remove_note(
    note_data: Form<RemoveNoteData>,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let RemoveNoteData { id } = note_data.into_inner();

    db.remove_user_note(id)?;

    let msg = "Removed note successfully.".to_string();
    Ok(ActionSuccessPage::new(msg, uri!(overview).to_string()))
}

/// Form data for deleting all of a user's posts.
#[derive(FromForm)]
pub struct DeletePostsForUserData {
    id: UserId,
}

/// Delete all of a user's posts.
#[post("/staff/delete-posts-for-user", data = "<delete_data>")]
pub fn delete_posts_for_user(
    delete_data: Form<DeletePostsForUserData>,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let DeletePostsForUserData { id } = delete_data.into_inner();

    db.delete_posts_for_user(id)?;

    let msg = "Deleted posts successfully.".to_string();
    Ok(ActionSuccessPage::new(msg, uri!(overview).to_string()))
}

/// Pin a thread.
#[post("/<board_name>/<thread_id>/pin")]
pub fn pin(
    board_name: String,
    thread_id: ThreadId,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    if db.board(&board_name).is_err() || db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    let uri = uri!(crate::routes::thread: &board_name, thread_id).to_string();

    db.pin_thread(thread_id)?;

    let msg: String = "Pinned post successfully.".into();
    Ok(ActionSuccessPage::new(msg, uri))
}

/// Unpin a thread.
#[post("/<board_name>/<thread_id>/unpin")]
pub fn unpin(
    board_name: String,
    thread_id: ThreadId,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    if db.board(&board_name).is_err() || db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    let uri = uri!(crate::routes::thread: &board_name, thread_id).to_string();

    db.unpin_thread(thread_id)?;

    let msg: String = "Unpinned post successfully.".into();
    Ok(ActionSuccessPage::new(msg, uri))
}

/// Lock a thread.
#[post("/<board_name>/<thread_id>/lock")]
pub fn lock(
    board_name: String,
    thread_id: ThreadId,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    if db.board(&board_name).is_err() || db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    let uri = uri!(crate::routes::thread: &board_name, thread_id).to_string();

    db.lock_thread(thread_id)?;

    let msg: String = "Locked post successfully.".into();
    Ok(ActionSuccessPage::new(msg, uri))
}

/// Unlock a thread.
#[post("/<board_name>/<thread_id>/unlock")]
pub fn unlock(
    board_name: String,
    thread_id: ThreadId,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    if db.board(&board_name).is_err() || db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    let uri = uri!(crate::routes::thread: &board_name, thread_id).to_string();

    db.unlock_thread(thread_id)?;

    let msg: String = "Unlocked post successfully.".into();
    Ok(ActionSuccessPage::new(msg, uri))
}

/// Delete a post without needing a password.
#[post("/<board_name>/<thread_id>/staff-delete/<post_id>")]
pub fn staff_delete(
    board_name: String,
    thread_id: ThreadId,
    post_id: PostId,
    db: State<Database>,
    _session: Session,
) -> Result<ActionSuccessPage> {
    if db.board(&board_name).is_err()
        || db.thread(thread_id).is_err()
        || db.post(post_id).is_err()
    {
        return Err(Error::PostNotFound { post_id });
    }

    let post = db.post(post_id)?;
    let thread = db.parent_thread(post_id)?;

    let delete_thread = db.is_first_post(post_id)?;

    let msg = if delete_thread {
        db.delete_thread(thread.id)?;
        format!("Deleted thread {} successfully.", post.thread_id)
    } else {
        db.delete_post(post_id)?;
        format!("Deleted post {} successfully.", post_id)
    };

    let redirect_uri = if delete_thread {
        uri!(crate::routes::board: thread.board_name)
    } else {
        uri!(crate::routes::thread: thread.board_name, thread.id)
    };

    Ok(ActionSuccessPage::new(msg, redirect_uri.to_string()))
}
