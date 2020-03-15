//! Rocket HTTP routes.

use std::path::PathBuf;
use std::string::ToString;

use argon2::verify_encoded;

use rocket::request::{Form, FromForm};
use rocket::response::NamedFile;
use rocket::{get, post, routes, uri, Route, State};

use crate::models::*;
use crate::views::*;
use crate::{config::Config, Error, Result};

pub mod new;
pub mod staff;

/// Get all routes.
pub fn routes() -> Vec<Route> {
    routes![
        crate::routes::home,
        crate::routes::static_file,
        crate::routes::banner,
        crate::routes::style,
        crate::routes::upload,
        crate::routes::board,
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
        crate::routes::staff::log,
    ]
}

/// Serve the home page.
#[get("/", rank = 0)]
pub fn home(config: State<Config>, db: State<Database>) -> Result<HomePage> {
    HomePage::new(&db, &config)
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

/// Serve a board.
#[get("/<board_name>", rank = 2)]
pub fn board(board_name: String, config: State<Config>, db: State<Database>) -> Result<BoardPage> {
    if db.board(&board_name).is_err() {
        return Err(Error::BoardNotFound { board_name });
    }

    BoardPage::new(board_name, &db, &config)
}

/// Serve a thread.
#[get("/<board_name>/<thread_id>", rank = 2)]
pub fn thread(
    board_name: String,
    thread_id: ThreadId,
    config: State<Config>,
    db: State<Database>,
) -> Result<ThreadPage> {
    if db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    ThreadPage::new(board_name, thread_id, &db, &config)
}

/// Serve a post preview.
#[get("/<_board_name>/<_thread_id>/preview/<post_id>", rank = 2)]
pub fn post_preview(
    _board_name: String,
    _thread_id: ThreadId,
    post_id: PostId,
    db: State<Database>,
) -> Result<PostPreview> {
    if db.post(post_id).is_err() {
        return Err(Error::PostNotFound { post_id });
    }

    PostPreview::new(post_id, &db)
}

/// Report a post.
#[get("/<_board_name>/<_thread_id>/report/<post_id>")]
pub fn report(
    _board_name: String,
    _thread_id: ThreadId,
    post_id: PostId,
    db: State<Database>,
) -> Result<ReportPage> {
    if db.post(post_id).is_err() {
        return Err(Error::PostNotFound { post_id });
    }

    Ok(ReportPage {
        post: db.post(post_id)?,
    })
}

/// Form data for reporting a post.
#[derive(FromForm)]
pub struct ReportData {
    reason: String,
}

/// Create a new post report.
#[post("/<_board_name>/<_thread_id>/report/<post_id>", data = "<report_data>")]
pub fn new_report(
    _board_name: String,
    _thread_id: ThreadId,
    post_id: PostId,
    report_data: Form<ReportData>,
    db: State<Database>,
) -> Result<ActionSuccessPage> {
    if db.post(post_id).is_err() {
        return Err(Error::PostNotFound { post_id });
    }

    let thread = db.parent_thread(post_id)?;

    db.insert_report(NewReport {
        reason: report_data.reason.clone(),
        post: post_id,
    })?;

    Ok(ActionSuccessPage {
        msg: format!("Reported post {} successfully.", post_id),
        redirect_uri: uri!(thread: thread.board_name, thread.id).to_string(),
    })
}

/// Serve a form for deleting a post.
#[get("/<_board_name>/<_thread_id>/delete/<post_id>")]
pub fn delete(
    _board_name: String,
    _thread_id: ThreadId,
    post_id: PostId,
    db: State<Database>,
) -> Result<DeletePage> {
    if db.post(post_id).is_err() {
        return Err(Error::PostNotFound { post_id });
    }

    Ok(DeletePage {
        post: db.post(post_id)?,
    })
}

/// Form data for deleting a post.
#[derive(FromForm)]
pub struct DeleteData {
    password: String,
    file_only: Option<String>,
}

/// Delete a post.
#[post("/<_board_name>/<_thread_id>/delete/<post_id>", data = "<delete_data>")]
pub fn handle_delete(
    _board_name: String,
    _thread_id: ThreadId,
    post_id: PostId,
    delete_data: Form<DeleteData>,
    db: State<Database>,
) -> Result<ActionSuccessPage> {
    if db.post(post_id).is_err() {
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
        uri!(board: thread.board_name).to_string()
    } else {
        uri!(thread: thread.board_name, thread.id).to_string()
    };

    Ok(ActionSuccessPage { msg, redirect_uri })
}
