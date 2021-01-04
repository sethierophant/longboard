//! Routes for staff member pages and actions.

use argon2::verify_encoded;

use chrono::{Duration, Utc};

use rand::{distributions::Alphanumeric, thread_rng, Rng};

use rocket::http::{hyper::header::Location, Cookie, Cookies, RawStr, Status};
use rocket::request::{
    Form, FromForm, FromFormValue, FromRequest, Outcome, Request,
};
use rocket::response::Response;
use rocket::{get, post, uri};

use crate::models::*;
use crate::views::staff::*;
use crate::views::{ActionSuccessPage, Context};
use crate::{Error, Result};

impl<'a, 'r> FromRequest<'a, 'r> for Session {
    type Error = Error;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let db = request
            .guard::<PooledConnection>()
            .expect("expected database to be initialized");
        let cookies = request.cookies();

        let err = Err((Status::Forbidden, Error::MissingSessionCookie));
        let session_id = cookies.get("session").ok_or(err)?.value();

        let err = Err((Status::Forbidden, Error::InvalidSessionCookie));
        let session = db.session(session_id).map_err(|_| err)?;

        Outcome::Success(session)
    }
}

/// Serve the login page for staff members.
#[get("/staff/login")]
pub fn login(context: Context, _user: User) -> Result<LoginPage> {
    LoginPage::new(&context)
}

/// Login form data.
#[derive(FromForm)]
pub struct LoginData {
    pub user: String,
    pub pass: String,
}

/// Login as a staff member.
#[post("/staff/login", data = "<login_data>")]
pub fn handle_login<'r>(
    login_data: Form<LoginData>,
    db: PooledConnection,
) -> Result<Response<'r>> {
    let staff = db.staff(&login_data.user)?;

    if !verify_encoded(&staff.password_hash, login_data.pass.as_bytes())? {
        // To reduce the effectiveness of brute-forcing passwords.
        std::thread::sleep(std::time::Duration::from_secs(4));

        return Err(Error::StaffInvalidPassword {
            staff_name: login_data.user.clone(),
        });
    }

    let id: String = thread_rng()
        .sample_iter(Alphanumeric)
        .map(char::from)
        .take(42)
        .collect();

    let expires = Utc::now() + Duration::weeks(1);

    let session_cookie = Cookie::build("session", id.clone())
        .path("/")
        .http_only(true)
        .finish();

    db.insert_session(Session { id, expires, staff })?;

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
    db: PooledConnection,
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
pub fn overview(
    context: Context,
    session: Option<Session>,
) -> Result<OverviewPage> {
    if session.is_none() {
        return Err(Error::NotAuthenticated);
    }

    OverviewPage::new(&context)
}

/// Serve the history for staff actions.
#[get("/staff/history")]
pub fn history(
    context: Context,
    session: Option<Session>,
) -> Result<HistoryPage> {
    if session.is_none() {
        return Err(Error::NotAuthenticated);
    }

    HistoryPage::new(&context)
}

/// Form data for closing a report.
#[derive(FromForm)]
pub struct CloseReportData {
    pub id: ReportId,
    pub reason: String,
}

/// Close a report.
#[post("/staff/close-report", data = "<close_data>")]
pub fn close_report(
    close_data: Form<CloseReportData>,
    db: PooledConnection,
    context: Context,
    session: Session,
) -> Result<ActionSuccessPage> {
    let CloseReportData { id, reason } = close_data.into_inner();

    db.delete_report(id)?;

    db.insert_staff_action(NewStaffAction {
        done_by: session.staff.name,
        action: format!("Closed report {}", id),
        reason,
    })?;

    let msg = format!("Closed report {} successfully.", id);
    Ok(ActionSuccessPage::new(
        msg,
        uri!(overview).to_string(),
        &context,
    )?)
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
    db: PooledConnection,
    context: Context,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let CreateBoardData { name, description } = create_data.into_inner();

    let msg = format!("Created board \"{}\" successfully.", name);

    db.insert_board(Board { name, description })?;

    Ok(ActionSuccessPage::new(
        msg,
        uri!(overview).to_string(),
        &context,
    )?)
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
    db: PooledConnection,
    context: Context,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let EditBoardData { name, description } = edit_data.into_inner();

    let msg = format!("Edited board \"{}\" successfully.", name);

    db.update_board(name, description)?;

    Ok(ActionSuccessPage::new(
        msg,
        uri!(overview).to_string(),
        &context,
    )?)
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
    db: PooledConnection,
    context: Context,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let DeleteBoardData { name } = delete_data.into_inner();

    let msg = format!("Deleted board \"{}\" successfully.", name);

    db.delete_board(name)?;

    Ok(ActionSuccessPage::new(
        msg,
        uri!(overview).to_string(),
        &context,
    )?)
}

/// Helper type for the duration a user is banned for.
pub struct BanDuration(pub Duration);

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
    pub id: UserId,
    pub duration: BanDuration,
    pub reason: String,
}

/// Ban a user.
#[post("/staff/ban-user", data = "<ban_data>")]
pub fn ban_user(
    ban_data: Form<BanUserData>,
    db: PooledConnection,
    context: Context,
    session: Session,
) -> Result<ActionSuccessPage> {
    let BanUserData {
        id,
        duration: BanDuration(duration),
        reason,
    } = ban_data.into_inner();

    let msg = format!("Banned user {} successfully.", id);

    db.ban_user(id, duration)?;

    db.insert_staff_action(NewStaffAction {
        done_by: session.staff.name,
        action: format!("Banned user {}", id),
        reason,
    })?;

    Ok(ActionSuccessPage::new(
        msg,
        uri!(overview).to_string(),
        &context,
    )?)
}

/// Form data for unbanning a user.
#[derive(FromForm)]
pub struct UnbanUserData {
    pub id: UserId,
    pub reason: String,
}

/// Unban a user.
#[post("/staff/unban-user", data = "<unban_data>")]
pub fn unban_user(
    unban_data: Form<UnbanUserData>,
    db: PooledConnection,
    context: Context,
    session: Session,
) -> Result<ActionSuccessPage> {
    let UnbanUserData { id, reason } = unban_data.into_inner();

    let msg = format!("Unbanned user {} successfully.", id);

    db.unban_user(id)?;

    db.insert_staff_action(NewStaffAction {
        done_by: session.staff.name,
        action: format!("Unbanned user {}", id),
        reason,
    })?;

    Ok(ActionSuccessPage::new(
        msg,
        uri!(overview).to_string(),
        &context,
    )?)
}

/// Form data for adding a note to a user.
#[derive(FromForm)]
pub struct AddNoteData {
    pub id: UserId,
    pub note: String,
}

/// Add a note to a user.
#[post("/staff/add-note", data = "<note_data>")]
pub fn add_note(
    note_data: Form<AddNoteData>,
    db: PooledConnection,
    context: Context,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let AddNoteData { id, note } = note_data.into_inner();

    db.set_user_note(id, note)?;

    let msg = "Added note successfully.".to_string();
    Ok(ActionSuccessPage::new(
        msg,
        uri!(overview).to_string(),
        &context,
    )?)
}

/// Form data for removing a note from a user.
#[derive(FromForm)]
pub struct RemoveNoteData {
    pub id: UserId,
}

/// Remove a note from a user.
#[post("/staff/remove-note", data = "<note_data>")]
pub fn remove_note(
    note_data: Form<RemoveNoteData>,
    db: PooledConnection,
    context: Context,
    _session: Session,
) -> Result<ActionSuccessPage> {
    let RemoveNoteData { id } = note_data.into_inner();

    db.remove_user_note(id)?;

    let msg = "Removed note successfully.".to_string();
    Ok(ActionSuccessPage::new(
        msg,
        uri!(overview).to_string(),
        &context,
    )?)
}

/// Form data for deleting all of a user's posts.
#[derive(FromForm)]
pub struct DeletePostsForUserData {
    pub id: UserId,
    pub reason: String,
}

/// Delete all of a user's posts.
#[post("/staff/delete-posts-for-user", data = "<delete_data>")]
pub fn delete_posts_for_user(
    delete_data: Form<DeletePostsForUserData>,
    db: PooledConnection,
    context: Context,
    session: Session,
) -> Result<ActionSuccessPage> {
    let DeletePostsForUserData { id, reason } = delete_data.into_inner();

    let count = db.delete_posts_for_user(id)?;

    db.insert_staff_action(NewStaffAction {
        done_by: session.staff.name,
        action: format!("Deleted all posts for user {} ({} total)", id, count),
        reason,
    })?;

    let msg = "Deleted posts successfully.".to_string();
    Ok(ActionSuccessPage::new(
        msg,
        uri!(overview).to_string(),
        &context,
    )?)
}

/// Form data for any request that requires a reason.
#[derive(FromForm)]
pub struct ReasonData {
    pub reason: String,
}

/// Pin a thread.
#[post("/<board_name>/<thread_id>/pin", data = "<reason_data>")]
pub fn pin(
    board_name: String,
    thread_id: ThreadId,
    reason_data: Form<ReasonData>,
    db: PooledConnection,
    context: Context,
    session: Session,
) -> Result<ActionSuccessPage> {
    let ReasonData { reason } = reason_data.into_inner();

    if db.board(&board_name).is_err() || db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    let uri = uri!(crate::routes::thread: &board_name, thread_id).to_string();

    db.pin_thread(thread_id)?;

    db.insert_staff_action(NewStaffAction {
        done_by: session.staff.name,
        action: format!("Pinned thread {}", thread_id),
        reason,
    })?;

    let msg: String = "Pinned post successfully.".into();
    Ok(ActionSuccessPage::new(msg, uri, &context)?)
}

/// Unpin a thread.
#[post("/<board_name>/<thread_id>/unpin", data = "<reason_data>")]
pub fn unpin(
    board_name: String,
    thread_id: ThreadId,
    reason_data: Form<ReasonData>,
    db: PooledConnection,
    context: Context,
    session: Session,
) -> Result<ActionSuccessPage> {
    let ReasonData { reason } = reason_data.into_inner();

    if db.board(&board_name).is_err() || db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    let uri = uri!(crate::routes::thread: &board_name, thread_id).to_string();

    db.unpin_thread(thread_id)?;

    db.insert_staff_action(NewStaffAction {
        done_by: session.staff.name,
        action: format!("Unpinned thread {}", thread_id),
        reason,
    })?;

    let msg: String = "Unpinned post successfully.".into();
    Ok(ActionSuccessPage::new(msg, uri, &context)?)
}

/// Lock a thread.
#[post("/<board_name>/<thread_id>/lock", data = "<reason_data>")]
pub fn lock(
    board_name: String,
    thread_id: ThreadId,
    reason_data: Form<ReasonData>,
    db: PooledConnection,
    context: Context,
    session: Session,
) -> Result<ActionSuccessPage> {
    let ReasonData { reason } = reason_data.into_inner();

    if db.board(&board_name).is_err() || db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    let uri = uri!(crate::routes::thread: &board_name, thread_id).to_string();

    db.lock_thread(thread_id)?;

    db.insert_staff_action(NewStaffAction {
        done_by: session.staff.name,
        action: format!("Locked thread {}", thread_id),
        reason,
    })?;

    let msg: String = "Locked thread successfully.".into();
    Ok(ActionSuccessPage::new(msg, uri, &context)?)
}

/// Unlock a thread.
#[post("/<board_name>/<thread_id>/unlock", data = "<reason_data>")]
pub fn unlock(
    board_name: String,
    thread_id: ThreadId,
    reason_data: Form<ReasonData>,
    db: PooledConnection,
    context: Context,
    session: Session,
) -> Result<ActionSuccessPage> {
    let ReasonData { reason } = reason_data.into_inner();

    if db.board(&board_name).is_err() || db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    let uri = uri!(crate::routes::thread: &board_name, thread_id).to_string();

    db.unlock_thread(thread_id)?;

    db.insert_staff_action(NewStaffAction {
        done_by: session.staff.name,
        action: format!("Unlocked thread {}", thread_id),
        reason,
    })?;

    let msg: String = "Unlocked thread successfully.".into();
    Ok(ActionSuccessPage::new(msg, uri, &context)?)
}

/// Delete a post without needing a password.
#[post(
    "/<board_name>/<thread_id>/staff-delete/<post_id>",
    data = "<reason_data>"
)]
pub fn staff_delete(
    board_name: String,
    thread_id: ThreadId,
    post_id: PostId,
    reason_data: Form<ReasonData>,
    db: PooledConnection,
    context: Context,
    session: Session,
) -> Result<ActionSuccessPage> {
    let ReasonData { reason } = reason_data.into_inner();

    if db.board(&board_name).is_err()
        || db.thread(thread_id).is_err()
        || db.post(post_id).is_err()
    {
        return Err(Error::PostNotFound { post_id });
    }

    let thread = db.parent_thread(post_id)?;

    let delete_thread = db.is_first_post(post_id)?;

    let msg = if delete_thread {
        db.delete_thread(thread.id)?;

        db.insert_staff_action(NewStaffAction {
            done_by: session.staff.name,
            action: format!("Deleted thread {}", thread_id),
            reason,
        })?;

        format!("Deleted thread {} successfully.", thread_id)
    } else {
        db.delete_post(post_id)?;

        db.insert_staff_action(NewStaffAction {
            done_by: session.staff.name,
            action: format!("Deleted post {}", post_id),
            reason,
        })?;

        format!("Deleted post {} successfully.", post_id)
    };

    let redirect_uri = if delete_thread {
        uri!(crate::routes::board: thread.board_name, 1)
    } else {
        uri!(crate::routes::thread: thread.board_name, thread.id)
    };

    Ok(ActionSuccessPage::new(
        msg,
        redirect_uri.to_string(),
        &context,
    )?)
}
