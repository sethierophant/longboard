//! Routes for creating new threads and new posts.

use std::fmt::Display;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::string::ToString;

use argon2::hash_encoded;

use chrono::offset::Utc;

use image::ImageFormat;

use mime::Mime;

use mime_guess::get_mime_extensions;

use multipart::server::save::{SavedData, SavedField};
use multipart::server::{Entries, Multipart};

use rand::{thread_rng, Rng};

use rocket::http::hyper::header::Location;
use rocket::http::{ContentType, Status};
use rocket::response::Redirect;
use rocket::{data, Outcome};
use rocket::{post, uri, Data, Request, Responder};

use crate::models::*;
use crate::parse::PostBody;
use crate::routes::NotBlocked;
use crate::{config::Conf, Error, Result};

/// This is a workaround for Rocket's URI type not supporting fragments (the
/// portion after #).
///
/// Hopefully this will be fixed in a future version of rocket. See also
/// #[842][1], #[998][2].
///
/// [1]: https://github.com/SergioBenitez/Rocket/issues/842
/// [2]: https://github.com/SergioBenitez/Rocket/issues/998
#[derive(Responder)]
#[response(status = 303)]
pub struct FragmentRedirect((), Location);

impl FragmentRedirect {
    pub fn to<U, F>(uri: U, fragment: F) -> FragmentRedirect
    where
        U: Display,
        F: Display,
    {
        FragmentRedirect((), Location(format!("{}#{}", uri, fragment)))
    }
}

/// Data guard for multipart/form-data entries.
#[derive(Debug)]
pub struct MultipartEntries(Entries);

impl data::FromDataSimple for MultipartEntries {
    type Error = Error;

    fn from_data(req: &Request, data: Data) -> data::Outcome<Self, Error> {
        let content_type = req
            .guard::<&ContentType>()
            .expect("expected request to have a content type");
        let conf = req.guard::<Conf>().expect("couldn't load configuration");

        if !content_type.is_form_data() {
            return Outcome::Failure((
                Status::BadRequest,
                Error::FormDataBadContentType,
            ));
        }

        let boundary =
            match content_type.params().find(|(k, _)| *k == "boundary") {
                Some((_, boundary)) => boundary,
                None => {
                    return Outcome::Failure((
                        Status::BadRequest,
                        Error::FormDataBadContentType,
                    ));
                }
            };

        let stream = data.open().take(conf.file_size_limit);

        let entries = match Multipart::with_body(stream, boundary)
            .save()
            .temp()
            .into_entries()
        {
            Some(entries) => entries,
            None => {
                return Outcome::Failure((
                    Status::BadRequest,
                    Error::FormDataCouldntParse,
                ))
            }
        };

        Outcome::Success(MultipartEntries(entries))
    }
}

impl MultipartEntries {
    fn param<S>(&self, name: S) -> Option<&str>
    where
        S: AsRef<str>,
    {
        if let Some(fields) = self.0.fields.get(name.as_ref()).as_ref() {
            if let Some(field) = fields.first() {
                if let SavedField {
                    data: SavedData::Text(text),
                    ..
                } = field
                {
                    if !text.trim().is_empty() {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    fn field<S>(&self, name: S) -> Option<&SavedField>
    where
        S: AsRef<str>,
    {
        if let Some(fields) = self.0.fields.get(name.as_ref()) {
            if let Some(field) = fields.first() {
                if field.headers.content_type.is_some() && field.data.size() > 0
                {
                    return Some(field);
                }
            }
        }

        None
    }
}

/// Create a new thread.
fn create_thread(
    board_name: String,
    entries: MultipartEntries,
    conf: Conf,
    db: &PooledConnection,
    user: User,
    session: Option<Session>,
) -> Result<ThreadId> {
    if entries.field("file").is_none() {
        return Err(Error::MissingThreadParam {
            param: "file".into(),
        });
    }

    let subject = entries
        .param("subject")
        .ok_or(Error::MissingThreadParam {
            param: "subject".into(),
        })?
        .to_string();

    let new_thread_id = db.insert_thread(NewThread {
        subject,
        board: board_name.clone(),
        locked: false,
        pinned: false,
    })?;

    create_post(
        board_name.clone(),
        new_thread_id,
        entries,
        conf,
        db,
        user,
        session,
    )?;

    db.trim_board(&board_name, crate::DEFAULT_THREAD_LIMIT)?;

    Ok(new_thread_id)
}

/// Crate a new post.
///
/// If the post has an attatched file, the file is also created.
fn create_post(
    board_name: String,
    thread_id: ThreadId,
    entries: MultipartEntries,
    conf: Conf,
    db: &PooledConnection,
    user: User,
    session: Option<Session>,
) -> Result<PostId> {
    if db.user_rate_limit_exceeded(user.id, *conf.rate_limit_same_user)? {
        return Err(Error::UserRateLimitExceeded);
    }

    if entries.field("file").is_some() && !conf.allow_uploads {
        return Err(Error::FileUploadNotAllowed);
    }

    let body_param = entries
        .param("body")
        .filter(|body| !body.trim().is_empty())
        .ok_or(Error::MissingPostParam {
            param: "body".into(),
        })?;

    let mut body = PostBody::parse(body_param, conf.filter_rules)?;
    body.resolve_refs(&db);

    let body_html = body.into_html();

    let limit = *conf.rate_limit_same_content;
    if db.content_rate_limit_exceeded(&body_html, limit)? {
        return Err(Error::ContentRateLimitExceeded);
    }

    let author_name = if let Some(param) = entries.param("author") {
        param.to_string()
    } else {
        conf.choose_name()?
    };

    // TODO: actually parse if this is an email, domain, ...
    let author_contact = entries.param("contact").map(ToString::to_string);

    let author_ident = match entries.param("ident") {
        Some(ident) => {
            let salt: [u8; 20] = thread_rng().gen();
            let hash = hash_encoded(
                ident.as_bytes(),
                &salt,
                &argon2::Config::default(),
            )
            .expect("could not hash ident with Argon2");

            Some(hash)
        }
        None => {
            if let Some(session) = session {
                if let Some(ident) = entries.param("staff-ident") {
                    if ident == "Anonymous" {
                        None
                    } else {
                        let named_role = format!(
                            "{} ({})",
                            session.staff.name, session.staff.role,
                        );

                        if ident == named_role {
                            Some(ident.to_string())
                        } else {
                            let role: Role = ident.parse()?;

                            if session.staff.is_authorized(role) {
                                Some(ident.to_string())
                            } else {
                                return Err(Error::UnauthorizedRole {
                                    staff_name: session.staff.name,
                                    role,
                                });
                            }
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
    };

    let delete_hash = entries.param("delete-pass").map(|pass| {
        let salt = b"longboard-delete";

        hash_encoded(pass.as_bytes(), salt, &argon2::Config::default())
            .expect("could not hash delete password with Argon2")
    });

    let no_bump = entries.param("no-bump").is_some();

    let new_post_id = db.insert_post(NewPost {
        body: body_html,
        author_name,
        author_contact,
        author_ident,
        delete_hash,
        thread: thread_id,
        board: board_name,
        user_id: user.id,
        no_bump,
    })?;

    if !no_bump {
        db.bump_thread(thread_id)?;
    }

    if entries.field("file").is_some() {
        create_file(new_post_id, entries, conf, db)?;
    };

    Ok(new_post_id)
}

/// Create a new file for a post.
fn create_file(
    post_id: PostId,
    entries: MultipartEntries,
    conf: Conf,
    db: &PooledConnection,
) -> Result<()> {
    let field = entries.field("file").unwrap();

    let content_type = match field.headers.content_type.as_ref() {
        Some(content_type) => content_type,
        None => return Err(Error::UploadMissingContentType),
    };

    // This converts from rocket::http::hyper::mime::Mime (re-export of mime
    // v0.2.6) to mime::Mime (mime v0.3.16).
    let content_type: Mime = content_type.to_string().parse().unwrap();

    if !conf.allow_file_types.contains(&content_type) {
        return Err(Error::UploadBadContentType { content_type });
    }

    let save_path = save_file(field, &content_type, conf.upload_dir)?;
    let save_name = save_path
        .file_name()
        .expect("bad filename for save path")
        .to_string_lossy()
        .into_owned();

    let orig_name = field.headers.filename.clone();

    let thumb_path = create_thumbnail(&save_path, &content_type)?;
    let thumb_name = thumb_path
        .file_name()
        .expect("bad thumb path")
        .to_string_lossy()
        .into_owned();

    let is_spoiler = entries.param("spoiler").is_some();

    db.insert_file(NewFile {
        save_name,
        orig_name,
        thumb_name,
        content_type: content_type.to_string(),
        is_spoiler,
        post: post_id,
    })?;

    Ok(())
}

/// Copy a file from the user's request into the uploads dir. Returns the path
/// the file was saved under.
fn save_file<P>(
    field: &SavedField,
    content_type: &Mime,
    upload_dir: P,
) -> Result<PathBuf>
where
    P: AsRef<Path>,
{
    let mut mime_ext = match get_mime_extensions(&content_type) {
        Some(&[ext, ..]) => ext,
        _ => {
            return Err(Error::UploadBadContentType {
                content_type: content_type.clone(),
            })
        }
    };

    if mime_ext == "jpe" {
        mime_ext = "jpg";
    }

    let epoch = Utc::now().format("%s").to_string();
    let mut num = 0;
    let mut suffix = String::new();

    // Loop until we generate a filename that isn't already taken.
    let mut new_path: PathBuf;
    loop {
        let new_base_name = format!("{}{}", epoch, suffix);

        let mut new_file_name = PathBuf::from(new_base_name);
        new_file_name.set_extension(mime_ext);

        new_path = upload_dir.as_ref().join(new_file_name);

        if !new_path.exists() {
            break;
        }

        num += 1;
        suffix = format!("-{}", num);
    }

    let mut new_file = File::create(&new_path).map_err(|err| {
        Error::from_io_error(
            err,
            format!("Couldn't create new upload file {}", new_path.display()),
        )
    })?;

    let mut file_data = field.data.readable()?;

    io::copy(&mut file_data, &mut new_file)?;

    Ok(new_path)
}

/// Create a thumbnail from a saved file.
fn create_thumbnail<P>(save_path: P, content_type: &Mime) -> Result<PathBuf>
where
    P: AsRef<Path>,
{
    let save_path = save_path.as_ref();

    let save_path_stem = save_path
        .file_stem()
        .expect("bad thumb path")
        .to_str()
        .expect("bad thumb path");
    let thumb_stem = format!("{}-thumb.png", save_path_stem);
    let thumb_name = Path::new(&thumb_stem).with_extension("png");
    let thumb_path =
        save_path.parent().expect("bad thumb path").join(thumb_name);

    match content_type.type_() {
        name if name == "image" => {
            create_image_thumbnail(save_path, &thumb_path)?
        }
        name if name == "video" => {
            create_video_thumbnail(save_path, &thumb_path)?
        }
        _ => {
            return Err(Error::UploadBadContentType {
                content_type: content_type.clone(),
            })
        }
    }

    Ok(thumb_path)
}

fn create_image_thumbnail<P1, P2>(source_path: P1, thumb_path: P2) -> Result<()>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let source_path = source_path.as_ref();
    let thumb_path = thumb_path.as_ref();

    let source_file =
        File::open(source_path).map_err(|cause| Error::IoErrorMsg {
            cause,
            msg: format!(
                "Couldn't open uploaded file {}",
                source_path.display()
            ),
        })?;

    let format = ImageFormat::from_path(source_path)?;

    let image = image::load(BufReader::new(source_file), format)?;

    let thumb = image.thumbnail(200, 200);

    thumb.save(&thumb_path)?;

    Ok(())
}

fn create_video_thumbnail<P1, P2>(source_path: P1, thumb_path: P2) -> Result<()>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let source_path = source_path.as_ref();
    let thumb_path = thumb_path.as_ref();

    let output = Command::new("ffmpeg")
        .arg("-i")
        .arg(source_path)
        .arg("-ss")
        .arg("00:00:00.69")
        .arg("-vframes")
        .arg("1")
        .arg(thumb_path)
        .output()
        .map_err(|cause| Error::IoErrorMsg {
            cause,
            msg: "Error running ffmpeg".into(),
        })?;

    if !output.status.success() {
        return Err(Error::FfmpegError {
            status: output.status,
            stdout: String::from_utf8(output.stdout)
                .expect("bad utf8 from ffmpeg"),
            stderr: String::from_utf8(output.stderr)
                .expect("bad utf8 from ffmpeg"),
        });
    }

    create_image_thumbnail(thumb_path, thumb_path)?;

    Ok(())
}

/// Handle a request to create a new thread.
#[post("/<board_name>", data = "<entries>", rank = 1)]
pub fn new_thread(
    board_name: String,
    entries: MultipartEntries,
    conf: Conf,
    db: PooledConnection,
    user: User,
    session: Option<Session>,
    _not_blocked: NotBlocked,
) -> Result<Redirect> {
    if db.board(&board_name).is_err() {
        return Err(Error::BoardNotFound { board_name });
    }

    let new_thread_id = db.inner.transaction::<_, Error, _>(|| {
        create_thread(board_name.clone(), entries, conf, &db, user, session)
    })?;

    Ok(Redirect::to(uri!(
        crate::routes::thread: board_name,
        new_thread_id
    )))
}

/// Handle a request to create a new post.
#[post("/<board_name>/<thread_id>", data = "<entries>", rank = 1)]
pub fn new_post(
    board_name: String,
    thread_id: ThreadId,
    entries: MultipartEntries,
    conf: Conf,
    db: PooledConnection,
    user: User,
    session: Option<Session>,
    _not_blocked: NotBlocked,
) -> Result<FragmentRedirect> {
    if db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    let new_post_id = db.inner.transaction::<_, Error, _>(|| {
        create_post(
            board_name.clone(),
            thread_id,
            entries,
            conf,
            &db,
            user,
            session,
        )
    })?;

    let uri = uri!(crate::routes::thread: board_name, thread_id);
    Ok(FragmentRedirect::to(uri, new_post_id))
}
