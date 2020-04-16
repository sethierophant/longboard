//! Rocket HTTP routes.

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::string::ToString;

use argon2::verify_encoded;

use pulldown_cmark::{html, Parser};

use rocket::http::{hyper::header::Location, Cookie, Status};
use rocket::request::{Form, FromForm, FromRequest, Outcome, Request};
use rocket::response::{NamedFile, Response};
use rocket::{get, post, routes, uri, Route, State};

use rocket_contrib::templates::Template;

use serde_json::value::{to_value, Value as JsonValue};

use crate::models::*;
use crate::views::*;
use crate::{config::Config, Error, Result};

pub mod new;
pub mod staff;

#[derive(FromForm)]
pub struct UserOptions {
    pub style: String,
}

impl Default for UserOptions {
    fn default() -> UserOptions {
        UserOptions {
            style: "default".into(),
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
                .unwrap_or(UserOptions::default().style),
        })
    }
}

/// Get all routes.
pub fn routes() -> Vec<Route> {
    routes![
        crate::routes::home,
        crate::routes::options,
        crate::routes::update_options,
        crate::routes::static_file,
        crate::routes::banner,
        crate::routes::style,
        crate::routes::upload,
        crate::routes::custom_page,
        crate::routes::board,
        crate::routes::board_catalog,
        crate::routes::thread,
        crate::routes::post_preview,
        crate::routes::new::new_thread,
        crate::routes::new::new_post,
        crate::routes::report,
        crate::routes::new_report,
        crate::routes::delete,
        crate::routes::handle_delete,
        crate::routes::staff::login,
        crate::routes::staff::handle_login,
        crate::routes::staff::logout,
        crate::routes::staff::overview,
        crate::routes::staff::history,
        crate::routes::staff::close_report,
        crate::routes::staff::create_board,
        crate::routes::staff::edit_board,
        crate::routes::staff::delete_board,
        crate::routes::staff::ban_user,
        crate::routes::staff::unban_user,
        crate::routes::staff::add_note,
        crate::routes::staff::remove_note,
        crate::routes::staff::delete_posts_for_user,
        crate::routes::staff::staff_delete,
        crate::routes::staff::pin,
        crate::routes::staff::unpin,
        crate::routes::staff::lock,
        crate::routes::staff::unlock,
    ]
}

/// Serve a static file.
#[get("/file/<file..>", rank = 1)]
pub fn static_file(file: PathBuf, config: State<Config>) -> Result<NamedFile> {
    Ok(NamedFile::open(config.options.resource_dir.join(file))?)
}

/// Serve a stylesheet.
#[get("/file/style/<file..>", rank = 0)]
pub fn style(file: PathBuf, config: State<Config>) -> Result<NamedFile> {
    Ok(NamedFile::open(
        config.options.resource_dir.join("style").join(file),
    )?)
}

/// Serve a script.
#[get("/file/script/<file..>", rank = 0)]
pub fn script(file: PathBuf, config: State<Config>) -> Result<NamedFile> {
    Ok(NamedFile::open(
        config.options.resource_dir.join("script").join(file),
    )?)
}

/// Serve a banner.
#[get("/file/banner/<file..>", rank = 0)]
pub fn banner(file: PathBuf, config: State<Config>) -> Result<NamedFile> {
    Ok(NamedFile::open(
        config.options.resource_dir.join("banners").join(file),
    )?)
}

/// Serve a user-uploaded file.
#[get("/file/upload/<file..>", rank = 0)]
pub fn upload(file: PathBuf, config: State<Config>) -> Result<NamedFile> {
    Ok(NamedFile::open(config.options.upload_dir.join(file))?)
}

/// Serve the home page.
#[get("/", rank = 0)]
pub fn home(
    config: State<Config>,
    db: State<Database>,
    options: UserOptions,
) -> Result<HomePage> {
    HomePage::new(&db, &config, &options)
}

/// Serve a admin-created page.
#[get("/page/<page_name>", rank = 1)]
pub fn custom_page(
    page_name: String,
    config: State<Config>,
    options: UserOptions,
) -> Result<Template> {
    let page_path = config
        .options
        .resource_dir
        .join("custom-pages")
        .join(format!("{}.md", &page_name.to_lowercase()));

    let page_contents = read_to_string(page_path)?;
    let parser = Parser::new(&page_contents);

    let mut page_html = String::new();
    html::push_html(&mut page_html, parser);

    let mut data = HashMap::new();
    data.insert(
        "page_info".to_string(),
        to_value(PageInfo::new(&page_name, &options))?,
    );
    data.insert(
        "page_footer".to_string(),
        to_value(PageFooter::new(&config))?,
    );
    data.insert("page_name".to_string(), JsonValue::String(page_name));
    data.insert("content".to_string(), JsonValue::String(page_html));

    Ok(Template::render("pages/custom-page", data))
}

/// Serve the user options page.
#[get("/options", rank = 0)]
pub fn options(
    db: State<Database>,
    config: State<Config>,
    options: UserOptions,
) -> Result<OptionsPage> {
    OptionsPage::new(&db, &config, &options)
}

/// Update user options.
#[post("/options", rank = 0, data = "<user_options>")]
pub fn update_options<'r>(
    user_options: Form<UserOptions>,
) -> Result<Response<'r>> {
    let UserOptions { style } = user_options.into_inner();

    let style_cookie = Cookie::build("option-style", style).path("/").finish();

    Ok(Response::build()
        .status(Status::SeeOther)
        .header(Location(uri!(crate::routes::options).to_string()))
        .header(style_cookie)
        .finalize())
}

/// Serve a board.
#[get("/<board_name>?<page>", rank = 2)]
pub fn board(
    board_name: String,
    page: Option<u32>,
    config: State<Config>,
    db: State<Database>,
    options: UserOptions,
    session: Option<Session>,
    _user: User,
) -> Result<BoardPage> {
    if db.board(&board_name).is_err() {
        return Err(Error::BoardNotFound { board_name });
    }

    BoardPage::new(
        board_name,
        page.unwrap_or(1),
        &db,
        &config,
        &options,
        session.and_then(|session| db.staff(&session.staff_name).ok()),
    )
}

/// Serve a board catalog.
#[get("/<board_name>/catalog", rank = 2)]
pub fn board_catalog(
    board_name: String,
    config: State<Config>,
    db: State<Database>,
    options: UserOptions,
    _user: User,
) -> Result<BoardCatalogPage> {
    if let Err(Error::DatabaseError(diesel::result::Error::NotFound)) =
        db.board(&board_name)
    {
        return Err(Error::BoardNotFound { board_name });
    }

    BoardCatalogPage::new(board_name, &db, &config, &options)
}

/// Serve a thread.
#[get("/<board_name>/<thread_id>", rank = 3)]
pub fn thread(
    board_name: String,
    thread_id: ThreadId,
    config: State<Config>,
    db: State<Database>,
    options: UserOptions,
    session: Option<Session>,
    _user: User,
) -> Result<ThreadPage> {
    if db.board(&board_name).is_err() || db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    ThreadPage::new(
        board_name,
        thread_id,
        &db,
        &config,
        &options,
        session.and_then(|session| db.staff(&session.staff_name).ok()),
    )
}

/// Serve a post preview.
#[get("/<board_name>/<thread_id>/preview/<post_id>", rank = 2)]
pub fn post_preview(
    board_name: String,
    thread_id: ThreadId,
    post_id: PostId,
    db: State<Database>,
    _user: User,
) -> Result<PostPreview> {
    if db.board(board_name).is_err()
        || db.thread(thread_id).is_err()
        || db.post(post_id).is_err()
    {
        return Err(Error::PostNotFound { post_id });
    }

    PostPreview::new(post_id, &db)
}

/// Report a post.
#[get("/<board_name>/<thread_id>/report/<post_id>")]
pub fn report(
    board_name: String,
    thread_id: ThreadId,
    post_id: PostId,
    db: State<Database>,
    options: UserOptions,
    _user: User,
) -> Result<ReportPage> {
    if db.board(board_name).is_err()
        || db.thread(thread_id).is_err()
        || db.post(post_id).is_err()
    {
        return Err(Error::PostNotFound { post_id });
    }

    ReportPage::new(post_id, &db, &options)
}

/// Form data for reporting a post.
#[derive(FromForm)]
pub struct ReportData {
    reason: String,
}

/// Create a new post report.
#[post("/<board_name>/<thread_id>/report/<post_id>", data = "<report_data>")]
pub fn new_report(
    board_name: String,
    thread_id: ThreadId,
    post_id: PostId,
    report_data: Form<ReportData>,
    db: State<Database>,
    options: UserOptions,
    user: User,
) -> Result<ActionSuccessPage> {
    if db.board(board_name).is_err()
        || db.thread(thread_id).is_err()
        || db.post(post_id).is_err()
    {
        return Err(Error::PostNotFound { post_id });
    }

    let ReportData { reason } = report_data.into_inner();

    if reason.len() > 250 {
        return Err(Error::ReportTooLong);
    }

    let thread = db.parent_thread(post_id)?;

    db.insert_report(NewReport {
        reason,
        post: post_id,
        user_id: user.id,
    })?;

    let msg = format!("Reported post {} successfully.", post_id);
    let uri = uri!(thread: thread.board_name, thread.id).to_string();
    Ok(ActionSuccessPage::new(msg, uri, &options))
}

/// Serve a form for deleting a post.
#[get("/<board_name>/<thread_id>/delete/<post_id>")]
pub fn delete(
    board_name: String,
    thread_id: ThreadId,
    post_id: PostId,
    db: State<Database>,
    options: UserOptions,
    _user: User,
) -> Result<DeletePage> {
    if db.board(board_name).is_err()
        || db.thread(thread_id).is_err()
        || db.post(post_id).is_err()
    {
        return Err(Error::PostNotFound { post_id });
    }

    DeletePage::new(post_id, &db, &options)
}

/// Form data for deleting a post.
#[derive(FromForm)]
pub struct DeleteData {
    password: String,
    file_only: Option<String>,
}

/// Delete a post.
#[post("/<board_name>/<thread_id>/delete/<post_id>", data = "<delete_data>")]
pub fn handle_delete(
    board_name: String,
    thread_id: ThreadId,
    post_id: PostId,
    delete_data: Form<DeleteData>,
    db: State<Database>,
    options: UserOptions,
    _user: User,
) -> Result<ActionSuccessPage> {
    if db.board(board_name).is_err()
        || db.thread(thread_id).is_err()
        || db.post(post_id).is_err()
    {
        return Err(Error::PostNotFound { post_id });
    }

    let post = db.post(post_id)?;
    let thread = db.parent_thread(post_id)?;

    let hash = post.delete_hash.ok_or(Error::DeleteInvalidPassword)?;

    if !verify_encoded(&hash, delete_data.password.as_bytes())? {
        return Err(Error::DeleteInvalidPassword);
    }

    let delete_thread = db.is_first_post(post_id)?;
    let delete_files_only = delete_data.file_only.is_some();

    if delete_thread && delete_files_only {
        return Err(Error::CannotDeleteThreadFilesOnly);
    }

    let msg = if delete_thread {
        db.delete_thread(thread.id)?;
        format!("Deleted thread {} successfully.", post.thread_id)
    } else if delete_files_only {
        db.delete_files_of_post(post_id)?;
        format!("Deleted files from post {} successfully.", post_id)
    } else {
        db.delete_post(post_id)?;
        format!("Deleted post {} successfully.", post_id)
    };

    let redirect_uri = if delete_thread {
        uri!(board: thread.board_name, 1)
    } else {
        uri!(thread: thread.board_name, thread.id)
    };

    Ok(ActionSuccessPage::new(
        msg,
        redirect_uri.to_string(),
        &options,
    ))
}
