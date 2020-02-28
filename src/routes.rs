use std::fmt::Display;
use std::io::Read;
use std::path::PathBuf;

use argon2::hash_encoded;

use chrono::offset::Utc;

use maplit::hashmap;

use multipart::server::{Multipart, Entries};

use rocket::http::ContentType;
use rocket::response::NamedFile;
use rocket::{State, Data, get, post, uri};

use rocket_contrib::templates::Template;

use serde_json::value::{Value, to_value};

use crate::{Error, Result, config::Config};
use crate::models::{ThreadId, Database, Post, NewPost, NewThread};

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
                let thread_id = thread.id;
                Ok(hashmap!{
                    "thread" => to_value(thread)?,
                    "posts" =>
                        to_value(db.posts_in_thread(thread_id)?
                                   .into_iter()
                                   .map(post_to_value)
                                   .collect::<Result<Vec<_>>>()?)?,
                    "thread_href" => {
                        let uri = uri!(thread: &board_name, thread_id);
                        to_value(uri.to_string())?
                    },
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
        "posts" =>
            to_value(db.posts_in_thread(thread_id)?
                       .into_iter()
                       .map(post_to_value)
                       .collect::<Result<Vec<_>>>()?
        )?,
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

use std::sync::Arc;
use std::collections::HashMap;
use multipart::server::SavedField;

type MultipartFields = HashMap<Arc<str>, Vec<SavedField>>;

pub trait MultipartFieldsExt {
    fn find_param<S: AsRef<str>>(&self, name: S) -> Result<Option<String>>;
}

impl MultipartFieldsExt for MultipartFields {
    fn find_param<S: AsRef<str>>(&self, name: S) -> Result<Option<String>> {
        if let Some(field) = self.get(name.as_ref()) {
            let mut buf = String::new();
            let data = &field[0].data;
            data.readable()?.read_to_string(&mut buf)?;

            if buf.trim().is_empty() {
                return Ok(None);
            }

            return Ok(Some(buf));
        }

        Ok(None)
    }
}

fn get_multipart_fields(content_type: &ContentType, data: Data)
    -> Result<MultipartFields>
{
    if !content_type.is_form_data() {
        return Err(Error::FormDataBadContentType);
    }

    let (_, boundary) = content_type.params().find(|&(k, _)| k == "boundary")
                                    .ok_or(Error::FormDataBadContentType)?;

    let Entries { fields, .. } = Multipart::with_body(data.open(), boundary)
        .save().temp().into_entries().ok_or(Error::FormDataCouldntParse)?;

    Ok(fields)
}

#[post("/<board_name>", data = "<data>", rank = 1)]
pub fn new_thread(
    board_name: String,
    content_type: &ContentType,
    data: Data,
    db: State<Database>
) -> Result<FragmentRedirect> {
    let fields = get_multipart_fields(content_type, data)?;

    let subject = fields.find_param("subject")?
        .ok_or(Error::MissingThreadParam { param: "subject".into() })?;

    let new_thread_id = db.insert_thread(NewThread {
        time_stamp: Utc::now(),
        subject,
        board: board_name.clone(),
    })?;

    let body = fields.find_param("body")?
        .ok_or(Error::MissingThreadParam { param: "body".into() })?;
    let author_name = fields.find_param("author")?;
    let author_contact = fields.find_param("contact")?;
    let author_ident = match fields.find_param("ident")? {
        Some(pass) => {
            let conf = argon2::Config::default();
            Some(hash_encoded(pass.as_bytes(), b"longboard", &conf)?)
        },
        None => None,
    };

    let new_post_id = db.insert_post(NewPost {
        time_stamp: Utc::now(),
        body,
        author_name,
        author_contact,
        author_ident,
        thread: new_thread_id,
    })?;

    let uri = uri!(thread: board_name=board_name, thread_id=new_thread_id);
    Ok(FragmentRedirect::to(uri, new_post_id))
}

#[post("/<board_name>/<thread_id>", data = "<data>", rank = 1)]
pub fn new_post(
    board_name: String,
    thread_id: ThreadId,
    content_type: &ContentType,
    data: Data,
    db: State<Database>
) -> Result<FragmentRedirect> {
    let fields = get_multipart_fields(content_type, data)?;

    let body = fields.find_param("body")?
        .ok_or(Error::MissingPostParam { param: "body".into() })?;

    let author_ident = match fields.find_param("ident")? {
        Some(pass) => {
            let conf = argon2::Config::default();
            Some(hash_encoded(pass.as_bytes(), b"longboard", &conf)?)
        },
        None => None,
    };

    let new_post_id = db.insert_post(NewPost {
        time_stamp: Utc::now(),
        body,
        author_name: fields.find_param("author")?,
        author_contact: fields.find_param("contact")?,
        author_ident,
        thread: thread_id,
    })?;

    let uri = uri!(thread: board_name=board_name, thread_id=thread_id);
    Ok(FragmentRedirect::to(uri, new_post_id))
}
