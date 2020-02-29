use std::collections::HashMap;
use std::fmt::Display;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use argon2::hash_encoded;

use chrono::offset::Utc;

use image::ImageFormat;
use image::error::ImageError;

use maplit::hashmap;

use mime_guess::get_mime_extensions;

use multipart::server::{Multipart, Entries};
use multipart::server::save::{SavedData, SavedField, SaveDir};

use rand::{thread_rng, Rng};

use rocket::http::ContentType;
use rocket::response::NamedFile;
use rocket::{State, Data, get, post, uri};

use rocket_contrib::templates::Template;

use serde_json::value::{Value, to_value};

use crate::{Error, Result, config::Config};
use crate::models::{
    ThreadId,
    Database,
    Post,
    NewPost,
    NewThread,
    NewFile,
};

/// Convert a post into a 'Value' which can be rendered by a template.
fn post_to_value(post: Post) -> Result<Value> {
    let author = match post.author_name.as_ref() {
        Some(s) => s.clone(),
        None => "Anonymous".to_string(),
    };
    let date = post.time_stamp.format("%F %R");

    let hash = post.author_ident.as_ref().map(|ident| {
        ident.split('$').last().expect("malformed argon2 hash").to_string()
    });

    let mut value = to_value(post)?;
    let obj = value.as_object_mut()
        .expect("unexpected value type for post serialization");
    obj.insert("time_stamp".to_string(), to_value(date.to_string())?);
    obj.insert("author_name".to_string(), to_value(author)?);
    obj.insert("author_ident".to_string(), to_value(hash)?);

    Ok(value)
}

#[get("/static/<file..>", rank = 0)]
pub fn static_file(file: PathBuf, config: State<Config>) -> Result<NamedFile> {
    Ok(NamedFile::open(config.static_dir.join(file))?)
}

#[get("/upload/<file..>", rank = 0)]
pub fn upload_file(file: PathBuf, config: State<Config>) -> Result<NamedFile> {
    Ok(NamedFile::open(config.upload_dir.join(file))?)
}

#[get("/", rank = 0)]
pub fn home(db: State<Database>) -> Result<Template> {
    let data = hashmap!{
        "all_boards" => to_value(db.all_boards()?)?,
        "version" => to_value(env!("CARGO_PKG_VERSION"))?,
        "stats" => to_value(hashmap!{
            "num_threads" => to_value(db.num_threads()?)?,
            "num_posts" => to_value(db.num_posts()?)?,
        })?,
    };

    Ok(Template::render("layout/home", &data))
}

#[get("/<board_name>", rank = 1)]
pub fn board(
    board_name: String,
    config: State<Config>,
    db: State<Database>
) -> Result<Template> {
    let data = hashmap!{
        "all_boards" => to_value(db.all_boards()?)?,
        "version" => to_value(env!("CARGO_PKG_VERSION"))?,
        "new_thread_form" => to_value(true)?,
        "board" => to_value(db.board(&board_name)?)?,
        "banner_href" => to_value(config.choose_banner().to_string())?,
        "threads" => to_value(db.threads_on_board(&board_name)?
            .into_iter()
            .map(|thread| {
                Ok(hashmap!{
                    "posts" => to_value(db.posts_in_thread(thread.id)?
                        .into_iter()
                        .map(|post| {
                            Ok(hashmap!{
                                "files" => to_value(db.files_in_post(post.id)?)?,
                                "post" => to_value(post_to_value(post)?)?,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?)?,
                    "thread_href" => {
                        let uri = uri!(thread: &board_name, thread.id);
                        to_value(uri.to_string())?
                    },
                    "thread" => to_value(thread)?,
                })
            }).collect::<Result<Vec<_>>>()?)?,
    };

    Ok(Template::render("layout/board", &data))
}

#[get("/<board_name>/<thread_id>", rank = 1)]
pub fn thread(
    board_name: String,
    thread_id: ThreadId,
    config: State<Config>,
    db: State<Database>
) -> Result<Template> {
    let data = hashmap!{
        "all_boards" => to_value(db.all_boards()?)?,
        "version" => to_value(env!("CARGO_PKG_VERSION"))?,
        "board" => to_value(db.board(&board_name)?)?,
        "banner_href" => to_value(config.choose_banner().to_string())?,
        "thread" => to_value(db.thread(&board_name, thread_id)?)?,
        "thread_href" => {
            let uri = uri!(thread: &board_name, thread_id);
            to_value(uri.to_string())?
        },
        "posts" => to_value(db.posts_in_thread(thread_id)?
            .into_iter()
            .map(|post| {
                Ok(hashmap!{
                    "files" => to_value(db.files_in_post(post.id)?)?,
                    "post" => to_value(post_to_value(post)?)?,
                })
            })
            .collect::<Result<Vec<_>>>()?)?,
    };

    Ok(Template::render("layout/thread", &data))
}

use rocket::http::hyper::header::Location;
use rocket::Responder;

/// This is a workaround for Rocket's URI type not supporting fragments (the
/// portion after the #). Hopefully this will be fixed in a future version of
/// rocket.
///
/// https://github.com/SergioBenitez/Rocket/issues/842
/// https://github.com/SergioBenitez/Rocket/issues/998
#[derive(Responder)]
#[response(status=303)]
pub struct FragmentRedirect((), Location);

impl FragmentRedirect {
    pub fn to<U, F>(uri: U, fragment: F) -> FragmentRedirect
        where U: Display, F: Display
    {
        FragmentRedirect((), Location(format!("{}#{}", uri, fragment)))
    }
}

type MultipartFields = HashMap<Arc<str>, Vec<SavedField>>;

pub trait MultipartFieldsExt {
    fn find_param<S: AsRef<str>>(&self, name: S) -> Option<&str>;
    fn find_file<S: AsRef<str>>(&self, name: S) -> Option<&SavedField>;
}

impl MultipartFieldsExt for MultipartFields {
    fn find_param<S: AsRef<str>>(&self, name: S) -> Option<&str> {
        if let Some(fields) = self.get(name.as_ref()) {
            if let SavedField {
                data: SavedData::Text(text),
                ..
            } = &fields[0] {
                if !text.trim().is_empty() {
                    return Some(text)
                }
            }
        }

        None
    }

    fn find_file<S: AsRef<str>>(&self, name: S) -> Option<&SavedField> {
        if let Some(fields) = self.get(name.as_ref()) {
            if fields[0].headers.content_type.is_some() {
                return Some(&fields[0]);
            }
        }

        None
    }
}

/// Parse data as multipart/form-data.
fn get_multipart_fields(content_type: &ContentType, data: Data)
    -> Result<(MultipartFields, SaveDir)>
{
    if !content_type.is_form_data() {
        return Err(Error::FormDataBadContentType);
    }

    let (_, boundary) = content_type
        .params()
        .find(|(k, _)| *k == "boundary")
        .ok_or(Error::FormDataBadContentType)?;

    let Entries { fields, save_dir, .. } =
        Multipart::with_body(data.open(), boundary)
            .save()
            .temp()
            .into_entries()
            .ok_or(Error::FormDataCouldntParse)?;

    Ok((fields, save_dir))
}

/// Compute an Argon2 hash of the ident.
fn hash_ident<S: AsRef<str>>(ident: S) -> Result<String> {
    let conf = argon2::Config::default();
    Ok(hash_encoded(ident.as_ref().as_bytes(), b"longboard", &conf)?)
}

/// Copy a file from the user's request into the uploads dir. Returns the path
/// the file was saved under.
fn save_file<P: AsRef<Path>>(field: &SavedField, upload_dir: P)
    -> Result<PathBuf>
{
    let new_base_name = {
        let epoch = Utc::now().format("%s").to_string();
        let suffix = thread_rng().gen_range(1000, 9999).to_string();
        format!("{}-{}", epoch, suffix)
    };

    let mut new_file_name = PathBuf::from(new_base_name);

    let mime_ext = field.headers.content_type
        .as_ref()
        .and_then(|content_type| {
            get_mime_extensions(&content_type)
                .and_then(|v| v.iter().last()).copied()
        });

    let file_ext = field.headers.filename
        .as_ref()
        .and_then(|filename| Path::new(filename).extension())
        .and_then(|ext| ext.to_str());

    if let Some(ext) = mime_ext.or(file_ext) {
        new_file_name.set_extension(ext);
    }

    let new_path = upload_dir.as_ref().join(&new_file_name);

    {
        let mut new_file = File::create(&new_path)?;
        let mut readable = field.data.readable()?;

        io::copy(&mut readable, &mut new_file)?;
    }

    Ok(new_path)
}

/// Create a thumbnail from a saved file. If the file is not an image, returns
/// Ok(None).
fn create_thumbnail<P: AsRef<Path>>(source: P) -> Result<Option<PathBuf>> {
    let source = source.as_ref();

    let format = match ImageFormat::from_path(source) {
        Ok(format) => format,
        Err(ImageError::Decoding(..)) => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let source_file = BufReader::new(File::open(source)?);
    let image = image::load(source_file, format)?;

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

#[post("/<board_name>", data = "<data>", rank = 1)]
pub fn new_thread(
    board_name: String,
    content_type: &ContentType,
    data: Data,
    config: State<Config>,
    db: State<Database>
) -> Result<FragmentRedirect> {
    let missing_subject_err = Error::MissingThreadParam {
        param: "subject".into()
    };

    let missing_body_err = Error::MissingThreadParam {
        param: "body".into()
    };

    let (fields, _save_dir) = get_multipart_fields(content_type, data)?;

    let new_thread_id = db.insert_thread(NewThread {
        time_stamp: Utc::now(),
        subject: fields.find_param("subject").ok_or(missing_subject_err)?,
        board: &board_name,
    })?;

    let ident = match fields.find_param("ident") {
        Some(param) => Some(hash_ident(param)?),
        None => None,
    };

    let new_post_id = db.insert_post(NewPost {
        time_stamp: Utc::now(),
        body: fields.find_param("body").ok_or(missing_body_err)?,
        author_name: fields.find_param("author"),
        author_contact: fields.find_param("contact"),
        author_ident: ident.as_deref(),
        thread: new_thread_id,
    })?;

    if let Some(field) =  fields.find_file("file") {
        let orig_name = field.headers.filename.as_deref();

        let content_type = field.headers.content_type
            .as_ref()
            .map(|m| m.to_string());

        let save_path = save_file(field, &config.upload_dir)?;
        let save_name = &save_path
            .file_name()
            .expect("bad image path")
            .to_str()
            .expect("bad image path");

        let thumb_path = create_thumbnail(&save_path)?;
        let thumb_name = thumb_path
            .as_ref()
            .map(|path| path.file_name()
                            .expect("bad image path")
                            .to_str()
                            .expect("bad image path"));

        db.insert_file(NewFile {
            save_name,
            orig_name,
            thumb_name,
            content_type: content_type.as_deref(),
            post: new_post_id
        })?;
    }

    let uri = uri!(thread: board_name, new_thread_id);
    Ok(FragmentRedirect::to(uri, new_post_id))
}

#[post("/<board_name>/<thread_id>", data = "<data>", rank = 1)]
pub fn new_post(
    board_name: String,
    thread_id: ThreadId,
    content_type: &ContentType,
    data: Data,
    config: State<Config>,
    db: State<Database>
) -> Result<FragmentRedirect> {
    let missing_body_err = Error::MissingThreadParam {
        param: "body".into()
    };

    let (fields, _save_dir) = get_multipart_fields(content_type, data)?;

    let ident = match fields.find_param("ident") {
        Some(param) => Some(hash_ident(param)?),
        None => None,
    };

    let new_post_id = db.insert_post(NewPost {
        time_stamp: Utc::now(),
        body: fields.find_param("body").ok_or(missing_body_err)?,
        author_name: fields.find_param("author"),
        author_contact: fields.find_param("contact"),
        author_ident: ident.as_deref(),
        thread: thread_id,
    })?;

    if let Some(field) =  fields.find_file("file") {
        let orig_name = field.headers.filename.as_deref();

        let content_type = field.headers.content_type
            .as_ref()
            .map(|m| m.to_string());

        let save_path = save_file(field, &config.upload_dir)?;
        let save_name = &save_path
            .file_name()
            .expect("bad image path")
            .to_str()
            .expect("bad image path");

        let thumb_path = create_thumbnail(&save_path)?;
        let thumb_name = thumb_path
            .as_ref()
            .map(|path| path.file_name()
                            .expect("bad image path")
                            .to_str()
                            .expect("bad image path"));

        db.insert_file(NewFile {
            save_name,
            orig_name,
            thumb_name,
            content_type: content_type.as_deref(),
            post: new_post_id
        })?;
    }

    let uri = uri!(thread: board_name, thread_id);
    Ok(FragmentRedirect::to(uri, new_post_id))
}
