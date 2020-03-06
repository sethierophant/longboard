use std::fmt::Display;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::string::ToString;

use argon2::hash_encoded;

use chrono::offset::Utc;

use image::error::ImageError;
use image::ImageFormat;

use mime_guess::get_mime_extensions;

use multipart::server::save::{SavedData, SavedField};
use multipart::server::{Entries, Multipart};

use rand::{thread_rng, Rng};

use rocket::http::ContentType;
use rocket::response::NamedFile;
use rocket::{get, post, uri, Data, State};

use crate::models::*;
use crate::views::*;
use crate::{config::Config, Error, Result};

/// Serve a static file.
#[get("/static/<file..>", rank = 0)]
pub fn static_file(file: PathBuf, config: State<Config>) -> Result<NamedFile> {
    Ok(NamedFile::open(config.static_dir.join(file))?)
}

/// Serve a user-uploaded file.
#[get("/upload/<file..>", rank = 0)]
pub fn upload_file(file: PathBuf, config: State<Config>) -> Result<NamedFile> {
    Ok(NamedFile::open(config.upload_dir.join(file))?)
}

/// Serve the home page.
#[get("/", rank = 0)]
pub fn home(config: State<Config>, db: State<Database>) -> Result<HomeView> {
    HomeView::new(&db, &config)
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

use rocket::http::hyper::header::Location;
use rocket::Responder;

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

/// Helper functions for entries from a parsed `multipart/form-data`
pub trait MultipartEntriesExt: Sized {
    fn parse(content_type: &ContentType, data: Data) -> Result<Self>;
    fn param<S>(&self, name: S) -> Option<&str>
    where
        S: AsRef<str>;
    fn field<S>(&self, name: S) -> Option<&SavedField>
    where
        S: AsRef<str>;
}

impl MultipartEntriesExt for Entries {
    fn parse(content_type: &ContentType, data: Data) -> Result<Entries> {
        if !content_type.is_form_data() {
            return Err(Error::FormDataBadContentType);
        }

        let (_, boundary) = content_type
            .params()
            .find(|(k, _)| *k == "boundary")
            .ok_or(Error::FormDataBadContentType)?;

        Ok(Multipart::with_body(data.open(), boundary)
            .save()
            .temp()
            .into_entries()
            .ok_or(Error::FormDataCouldntParse)?)
    }

    fn param<S>(&self, name: S) -> Option<&str>
    where
        S: AsRef<str>,
    {
        if let Some(fields) = self.fields.get(name.as_ref()) {
            if let SavedField {
                data: SavedData::Text(text),
                ..
            } = &fields[0]
            {
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
        }

        None
    }

    fn field<S>(&self, name: S) -> Option<&SavedField>
    where
        S: AsRef<str>,
    {
        if let Some(fields) = self.fields.get(name.as_ref()) {
            if fields[0].headers.content_type.is_some() {
                return Some(&fields[0]);
            }
        }

        None
    }
}

/// Compute an Argon2 hash of the ident.
fn hash_ident<S>(ident: S) -> Result<String>
where
    S: AsRef<str>,
{
    let conf = argon2::Config::default();
    Ok(hash_encoded(
        ident.as_ref().as_bytes(),
        b"longboard",
        &conf,
    )?)
}

/// Copy a file from the user's request into the uploads dir. Returns the path
/// the file was saved under.
fn save_file<P>(field: &SavedField, upload_dir: P) -> Result<PathBuf>
where
    P: AsRef<Path>,
{
    let new_base_name = {
        let epoch = Utc::now().format("%s").to_string();
        let suffix = thread_rng().gen_range(1000, 9999).to_string();
        format!("{}-{}", epoch, suffix)
    };

    let mut new_file_name = PathBuf::from(new_base_name);

    let mime_ext = field
        .headers
        .content_type
        .as_ref()
        .and_then(|content_type| {
            get_mime_extensions(&content_type)
                .and_then(|v| v.iter().last())
                .copied()
        });

    let file_ext = field
        .headers
        .filename
        .as_ref()
        .and_then(|filename| Path::new(filename).extension())
        .and_then(|ext| ext.to_str());

    if let Some(ext) = mime_ext.or(file_ext) {
        new_file_name.set_extension(ext);
    }

    let new_path = upload_dir.as_ref().join(&new_file_name);

    {
        let msg = format!("Couldn't create new upload file {}", new_path.display());

        let mut new_file = File::create(&new_path).map_err(|err| Error::from_io_error(err, msg))?;

        let mut readable = field.data.readable()?;

        io::copy(&mut readable, &mut new_file)?;
    }

    Ok(new_path)
}

/// Create a thumbnail from a saved file. If the file is not an image, returns
/// Ok(None).
fn create_thumbnail<P>(source: P) -> Result<Option<PathBuf>>
where
    P: AsRef<Path>,
{
    let source = source.as_ref();

    let format = match ImageFormat::from_path(source) {
        Ok(format) => format,
        Err(ImageError::Decoding(..)) => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let msg = format!("Couldn't open uploaded file {}", source.display());

    let source_file = File::open(source).map_err(|err| Error::from_io_error(err, msg))?;
    let source_reader = BufReader::new(source_file);
    let image = image::load(source_reader, format)?;

    let thumb = image.thumbnail(200, 200);

    let source_stem = source
        .file_stem()
        .expect("bad thumb path")
        .to_str()
        .expect("bad thumb path");
    let thumb_stem = format!("{}-thumb.png", source_stem);
    let thumb_name = Path::new(&thumb_stem).with_extension("png");
    let thumb_path = source.parent().expect("bad thumb path").join(thumb_name);

    thumb.save(&thumb_path)?;

    Ok(Some(thumb_path))
}

/// Handle a request to create a new thread.
#[post("/<board_name>", data = "<data>", rank = 1)]
pub fn new_thread(
    board_name: String,
    content_type: &ContentType,
    data: Data,
    config: State<Config>,
    db: State<Database>,
) -> Result<FragmentRedirect> {
    if db.board(&board_name).is_err() {
        return Err(Error::BoardNotFound { board_name });
    }

    let missing_subject_err = Error::MissingThreadParam {
        param: "subject".into(),
    };

    let missing_body_err = Error::MissingThreadParam {
        param: "body".into(),
    };

    let entries: Entries = MultipartEntriesExt::parse(content_type, data)?;

    let ident = entries.param("ident").map(hash_ident).transpose()?;

    let field = entries.field("file").filter(|field| field.data.size() > 0);
    if let Some(field) = field {
        let orig_name = field.headers.filename.as_deref();

        let content_type = field.headers.content_type.as_ref().map(ToString::to_string);

        let save_path = save_file(field, &config.upload_dir)?;
        let save_name = &save_path.file_name().unwrap().to_str().unwrap();

        let thumb_path = create_thumbnail(&save_path)?;
        let thumb_name = thumb_path
            .as_ref()
            .map(|path| path.file_name().unwrap().to_str().unwrap());

        let new_thread_id = db.insert_thread(NewThread {
            time_stamp: Utc::now(),
            subject: entries.param("subject").ok_or(missing_subject_err)?,
            board: &board_name,
        })?;

        let new_post_id = db.insert_post(NewPost {
            time_stamp: Utc::now(),
            body: entries.param("body").ok_or(missing_body_err)?,
            author_name: entries.param("author"),
            author_contact: entries.param("contact"),
            author_ident: ident.as_deref(),
            thread: new_thread_id,
        })?;

        db.insert_file(NewFile {
            save_name,
            orig_name,
            thumb_name,
            content_type: content_type.as_deref(),
            post: new_post_id,
        })?;

        let uri = uri!(thread: board_name, new_thread_id);
        return Ok(FragmentRedirect::to(uri, new_post_id));
    }

    Err(Error::MissingThreadParam {
        param: "file".into(),
    })
}

/// Handle a request to create a new post.
#[post("/<board_name>/<thread_id>", data = "<data>", rank = 1)]
pub fn new_post(
    board_name: String,
    thread_id: ThreadId,
    content_type: &ContentType,
    data: Data,
    config: State<Config>,
    db: State<Database>,
) -> Result<FragmentRedirect> {
    if db.thread(&board_name, thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    let missing_body_err = Error::MissingThreadParam {
        param: "body".into(),
    };

    let entries: Entries = MultipartEntriesExt::parse(content_type, data)?;

    let ident = entries.param("ident").map(hash_ident).transpose()?;

    let field = entries.field("file").filter(|field| field.data.size() > 0);
    let content_type;
    let save_path;
    let thumb_path;
    let new_file = if let Some(field) = field {
        let orig_name = field.headers.filename.as_deref();

        content_type = field.headers.content_type.as_ref().map(ToString::to_string);

        save_path = save_file(field, &config.upload_dir)?;
        let save_name = &save_path
            .file_name()
            .expect("bad image path")
            .to_str()
            .expect("bad image path");

        thumb_path = create_thumbnail(&save_path)?;
        let thumb_name = thumb_path.as_ref().map(|path| {
            path.file_name()
                .expect("bad image path")
                .to_str()
                .expect("bad image path")
        });

        Some(NewFile {
            save_name,
            orig_name,
            thumb_name,
            content_type: content_type.as_deref(),
            post: 0,
        })
    } else {
        None
    };

    let new_post_id = db.insert_post(NewPost {
        time_stamp: Utc::now(),
        body: entries.param("body").ok_or(missing_body_err)?,
        author_name: entries.param("author"),
        author_contact: entries.param("contact"),
        author_ident: ident.as_deref(),
        thread: thread_id,
    })?;

    if let Some(new_file) = new_file {
        db.insert_file(NewFile {
            post: new_post_id,
            ..new_file
        })?;
    }

    let uri = uri!(thread: board_name, thread_id);
    Ok(FragmentRedirect::to(uri, new_post_id))
}
