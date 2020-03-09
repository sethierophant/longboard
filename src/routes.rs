use std::path::PathBuf;
use std::string::ToString;

use argon2::verify_encoded;

use rocket::request::{Form, FromForm};
use rocket::response::NamedFile;
use rocket::{get, post, uri, State};

use crate::models::*;
use crate::views::*;
use crate::{config::Config, Error, Result};

pub mod create;

/// Serve the home page.
#[get("/", rank = 0)]
pub fn home(config: State<Config>, db: State<Database>) -> Result<HomeView> {
    HomeView::new(&db, &config)
}

/// Serve a static asset.
#[get("/file/static/<file..>", rank = 0)]
pub fn static_file(file: PathBuf, config: State<Config>) -> Result<NamedFile> {
    Ok(NamedFile::open(config.static_dir.join(file))?)
}

/// Serve a user-uploaded file.
#[get("/file/upload/<file..>", rank = 0)]
pub fn upload_file(file: PathBuf, config: State<Config>) -> Result<NamedFile> {
    Ok(NamedFile::open(config.upload_dir.join(file))?)
}

/// Serve a board.
#[get("/<board_name>", rank = 1)]
pub fn board(board_name: String, config: State<Config>, db: State<Database>) -> Result<BoardView> {
    if db.board(&board_name).is_err() {
        return Err(Error::BoardNotFound { board_name });
    }

    BoardView::new(board_name, &db, &config)
}

/// Serve a thread.
#[get("/<board_name>/<thread_id>", rank = 1)]
pub fn thread(
    board_name: String,
    thread_id: ThreadId,
    config: State<Config>,
    db: State<Database>,
) -> Result<ThreadView> {
    if db.thread(&board_name, thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    ThreadView::new(board_name, thread_id, &db, &config)
}

#[get("/action/post/report/<post_id>")]
pub fn report(post_id: PostId, db: State<Database>) -> Result<ReportView> {
    if db.post(post_id).is_err() {
        return Err(Error::PostNotFound { post_id });
    }

    ReportView::new(post_id, &db)
}

#[derive(FromForm)]
pub struct ReportData {
    reason: String,
}

#[post("/action/post/report/<post_id>", data = "<report_data>")]
pub fn new_report(
    post_id: PostId,
    report_data: Form<ReportData>,
    db: State<Database>,
) -> Result<ActionSuccessView> {
    if db.post(post_id).is_err() {
        return Err(Error::PostNotFound { post_id });
    }

    let thread = db.parent_thread(post_id)?;

    db.insert_report(NewReport {
        reason: report_data.reason.clone(),
        post: post_id,
    })?;

    Ok(ActionSuccessView {
        msg: format!("Reported post {} successfully.", post_id),
        redirect_uri: uri!(thread: thread.board_name, thread.id).to_string(),
    })
}

#[get("/action/post/delete/<post_id>")]
pub fn delete(post_id: PostId, db: State<Database>) -> Result<DeleteView> {
    if db.post(post_id).is_err() {
        return Err(Error::PostNotFound { post_id });
    }

    DeleteView::new(post_id, &db)
}

#[derive(FromForm)]
pub struct DeleteData {
    password: String,
    file_only: Option<String>,
}

#[post("/action/post/delete/<post_id>", data = "<delete_data>")]
pub fn do_delete(
    post_id: PostId,
    delete_data: Form<DeleteData>,
    db: State<Database>,
) -> Result<ActionSuccessView> {
    if db.post(post_id).is_err() {
        return Err(Error::PostNotFound { post_id });
    }

    let post = db.post(post_id)?;
    let thread = db.parent_thread(post_id)?;

    let hash = post.delete_hash.ok_or(Error::PasswordError)?;

    if !verify_encoded(&hash, delete_data.password.as_bytes())? {
        return Err(Error::PasswordError);
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

    Ok(ActionSuccessView { msg, redirect_uri })
}
