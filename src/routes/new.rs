//! Routes for creating new threads and new posts.

use std::fmt::Display;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::string::ToString;

use argon2::hash_encoded;

use chrono::offset::Utc;

use image::error::ImageError;
use image::ImageFormat;

use maplit::{hashmap, hashset};

use mime_guess::get_mime_extensions;

use multipart::server::save::{SavedData, SavedField};
use multipart::server::{Entries, Multipart};

use pulldown_cmark::{html::push_html, Options, Parser};

use regex::{Captures, Regex};

use rocket::http::{hyper::header::Location, ContentType};
use rocket::{post, uri, Data, Responder, State};

use crate::models::*;
use crate::{config::Config, Error, Result};

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

/// Compute an Argon2 hash of a param.
fn hash_param<S>(ident: S) -> Result<String>
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
    let mime_ext = field
        .headers
        .content_type
        .as_ref()
        .and_then(|content_type| {
            get_mime_extensions(&content_type)
                .and_then(|v| v.first())
                .map(|ext| if *ext == "jpe" { "jpg" } else { ext })
        });

    let file_ext = field
        .headers
        .filename
        .as_ref()
        .and_then(|filename| Path::new(filename).extension())
        .and_then(|ext| ext.to_str());

    let epoch = Utc::now().format("%s").to_string();
    let mut num = 0;
    let mut suffix = String::new();

    let mut new_path: PathBuf;

    // Loop until we generate a filename that isn't already taken.
    loop {
        let new_base_name = format!("{}{}", epoch, suffix);
        let mut new_file_name = PathBuf::from(new_base_name);

        if let Some(ext) = mime_ext.or(file_ext) {
            new_file_name.set_extension(ext);
        }

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

    let mut readable = field.data.readable()?;

    io::copy(&mut readable, &mut new_file)?;

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

/// Parse a post's body.
fn parse_body<S>(body: S, conf: &Config, db: &Database) -> Result<String>
where
    S: AsRef<str>,
{
    // TODO: This is definitely not the most efficient way to do this.

    // First pass: replace post references with links and run word filters
    let re = Regex::new(r">>(?P<id>\d+)").unwrap();
    let mut body = re
        .replace_all(body.as_ref(), |captures: &Captures| {
            let id: PostId = captures.name("id").unwrap().as_str().parse().unwrap();

            match db.post_uri(id) {
                Ok(uri) => format!("<a class=\"post-ref\" href=\"{}\">&gt;&gt;{}</a>", uri, id),
                Err(_) => format!("<a>&gt;&gt;{}</a>", id),
            }
        })
        .into_owned();

    for rule in &conf.options.filter_rules {
        body = Regex::new(&rule.pattern)?
            .replace_all(&body, rule.replace_with.as_str())
            .into_owned();
    }

    // Second pass: parse markdown
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);

    let mut html = String::new();
    push_html(&mut html, Parser::new_ext(&body, opts));

    // Third pass: sanitize HTML
    Ok(ammonia::Builder::new()
        .link_rel(Some("noopener noreferrer nofollow"))
        .allowed_classes(hashmap! { "a" => hashset!["post-ref"] })
        .clean(&html)
        .to_string())
}

/// Create a new post and optionally a new file if the post has one. These are
/// models that should be inserted into the database.
///
/// Note that the IDs for the parents of both models still need to be set.
fn create_new_models(
    entries: Entries,
    config: &Config,
    db: &Database,
) -> Result<(NewPost, Option<NewFile>)> {
    let missing_body_err = Error::MissingThreadParam {
        param: "body".into(),
    };

    let body = entries
        .param("body")
        .map(|body| parse_body(body, config, db))
        .transpose()?
        .filter(|body| !body.trim().is_empty())
        .ok_or(missing_body_err)?;

    let author_name = entries
        .param("author")
        .unwrap_or_else(|| config.choose_name())
        .to_string();

    let author_contact = entries.param("contact").map(ToString::to_string);
    let author_ident = entries.param("ident").map(hash_param).transpose()?;
    let delete_hash = entries.param("delete-pass").map(hash_param).transpose()?;

    let new_post = NewPost {
        body,
        author_name,
        author_contact,
        author_ident,
        delete_hash,
        thread: 0,
        board: String::new(),
    };

    let field = entries.field("file").filter(|field| field.data.size() > 0);

    let new_file = if let Some(field) = field {
        let save_path = save_file(field, &config.options.upload_dir)?;
        let save_name = save_path
            .file_name()
            .expect("bad filename for save path")
            .to_string_lossy()
            .into_owned();

        let orig_name = field.headers.filename.clone();

        let thumb_path = create_thumbnail(&save_path)?;
        let thumb_name = thumb_path.and_then(|path| {
            path.file_name()
                .map(|os_str| os_str.to_string_lossy().into_owned())
        });

        let content_type = field.headers.content_type.as_ref().map(ToString::to_string);

        let is_spoiler = entries.param("spoiler").is_some();

        Some(NewFile {
            save_name,
            orig_name,
            thumb_name,
            content_type,
            is_spoiler,
            post: 0,
        })
    } else {
        None
    };

    Ok((new_post, new_file))
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

    let missing_file_err = Error::MissingThreadParam {
        param: "file".into(),
    };

    let entries: Entries = MultipartEntriesExt::parse(content_type, data)?;

    let subject = entries
        .param("subject")
        .ok_or(missing_subject_err)?
        .to_string();

    let models = create_new_models(entries, &config, &db)?;

    if let (mut new_post, Some(mut new_file)) = models {
        let new_thread_id = db.insert_thread(NewThread {
            subject,
            board: board_name.clone(),
        })?;
        new_post.thread = new_thread_id;
        new_post.board = board_name.clone();

        let new_post_id = db.insert_post(new_post)?;
        new_file.post = new_post_id;

        db.insert_file(new_file)?;

        let uri = uri!(crate::routes::thread: board_name, new_thread_id);
        Ok(FragmentRedirect::to(uri, new_post_id))
    } else {
        Err(missing_file_err)
    }
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
    if db.thread(thread_id).is_err() {
        return Err(Error::ThreadNotFound {
            board_name,
            thread_id,
        });
    }

    let entries: Entries = MultipartEntriesExt::parse(content_type, data)?;

    let (mut new_post, new_file) = create_new_models(entries, &config, &db)?;

    new_post.thread = thread_id;
    new_post.board = board_name.clone();
    let new_post_id = db.insert_post(new_post)?;

    if let Some(mut new_file) = new_file {
        new_file.post = new_post_id;
        db.insert_file(new_file)?;
    }

    let uri = uri!(crate::routes::thread: board_name, thread_id);
    Ok(FragmentRedirect::to(uri, new_post_id))
}
