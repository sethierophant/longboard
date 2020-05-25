//! Parsing for uploaded posts.

use combine::parser::char::*;
use combine::parser::repeat::*;
use combine::stream::position;
use combine::*;

use horrorshow::html;
use horrorshow::prelude::*;

use regex::Regex;

use crate::config::FilterRule;
use crate::models::*;
use crate::{Error, Result};

// The reason that the parser is split into many small top-level parser is that
// it greatly reduces compile times with combine 4.x.
//
// Defining all of these parsers as closures in the same function inflates
// compile times to 30 minutes+ on my machine, while these top-level functions
// can be compiled in about 1 minute.

/// Parse whitespace that isn't a newline.
fn line_spaces<Input>() -> impl Parser<Input, Output = String>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    many(satisfy(|c: char| c.is_whitespace() && c != '\n'))
}

/// Parse `**strong**` text.
fn strong_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    let p = not_followed_by(attempt(string("**"))).with(any());

    string("**")
        .with(many1(p))
        .skip(string("**"))
        .map(LineItem::Strong)
}

/// Parse `*emphasized*` text.
fn emphasis_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    let p = not_followed_by(char('*')).with(any());

    char('*')
        .with(many1(p))
        .skip(char('*'))
        .map(LineItem::Emphasis)
}

/// Parse `~spoiler~` text.
fn spoiler_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    let p = not_followed_by(char('~')).with(any());

    char('~')
        .with(many1(p))
        .skip(char('~'))
        .map(LineItem::Spoiler)
}

/// Parse a post ref like `>>123`.
fn post_ref_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    string(">>")
        .with(many1(digit()))
        .map(move |s: String| LineItem::PostRef {
            id: s.parse().unwrap(),
            uri: None,
        })
}

/// Parse a http link.
fn link_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    let link_char = || {
        satisfy(|c: char| {
            let special_link_chars = [
                '-', '.', '_', '~', ':', '/', '?', '#', '[', ']', '@', '!',
                '$', '&', '\'', '(', ')', '*', '+', ',', ';', '%', '=',
            ];

            c.is_alphanumeric() || special_link_chars.contains(&c)
        })
    };

    choice((
        attempt(string("http://")).and(many1(link_char())),
        attempt(string("https://")).and(many1(link_char())),
    ))
    .map(|(schema, rest): (&str, String)| {
        LineItem::Link(format!("{}{}", schema, rest))
    })
}

/// Parse `\`code\`` within a line.
fn line_code_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    let p = not_followed_by(char('`')).with(any());

    char('`').with(many1(p)).skip(char('`')).map(LineItem::Code)
}

/// Parse any plain text within a line; anything not covered by the above
/// line-item parsers.
fn line_text_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    let end = choice((
        char('*'),
        char('~'),
        char('>'),
        char('`'),
        attempt(string("http://")).map(|_| '\0'),
        attempt(string("https://")).map(|_| '\0'),
        newline(),
    ));

    many1(not_followed_by(end).with(any())).map(LineItem::Text)
}

/// Parse all line items.
fn line_items_parser<Input>() -> impl Parser<Input, Output = Vec<LineItem>>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    many1(choice((
        attempt(strong_parser()),
        attempt(emphasis_parser()),
        attempt(spoiler_parser()),
        attempt(post_ref_parser()),
        attempt(link_parser()),
        attempt(line_code_parser()),
        line_text_parser(),
    )))
}

/// Parse a block of code.
fn code_parser<Input>() -> impl Parser<Input, Output = BlockItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    let delim = string("```").skip(newline());
    let p = not_followed_by(attempt(delim)).with(any());

    string("```")
        .skip(line_spaces())
        .with(many(alpha_num()))
        .skip(newline())
        .and(many1(p))
        .skip(string("```"))
        .skip(newline())
        .map(|(lang, contents): (String, String)| {
            if lang.is_empty() {
                BlockItem::Code {
                    language: None,
                    contents,
                }
            } else {
                BlockItem::Code {
                    language: Some(lang),
                    contents,
                }
            }
        })
}

/// Parse a `#Header`.
fn header_parser<Input>() -> impl Parser<Input, Output = BlockItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    char('#')
        .skip(line_spaces())
        .with(line_items_parser())
        .skip(newline())
        .map(BlockItem::Header)
}

/// Parse a `>greentext` blockquote.
fn quote_parser<Input>() -> impl Parser<Input, Output = BlockItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    char('>')
        .skip(line_spaces())
        .with(line_items_parser())
        .skip(newline())
        .map(BlockItem::Quote)
}

/// Parse any kind of block text not covered by the above block item
/// parsers.
fn text_parser<Input>() -> impl Parser<Input, Output = BlockItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    line_items_parser().skip(newline()).map(BlockItem::Text)
}

fn post_body_parser<Input>() -> impl Parser<Input, Output = PostBody>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    many1(choice((
        attempt(code_parser()),
        attempt(header_parser()),
        attempt(quote_parser()),
        text_parser(),
    )))
    .skip(eof())
    .map(PostBody)
}

/// A parsed post body which can be rendered into HTML.
pub struct PostBody(Vec<BlockItem>);

impl PostBody {
    /// Parse the post body.
    pub fn parse<S>(content: S, rules: &[FilterRule]) -> Result<PostBody>
    where
        S: Into<String>,
    {
        let mut content: String = content.into();

        for rule in rules {
            content = rule
                .pattern
                .replace_all(&content, rule.replace_with.as_str())
                .into_owned();
        }

        content = Regex::new(r"(\r\n)+")
            .unwrap()
            .replace_all(&content, "\n")
            .into_owned();

        if !content.ends_with('\n') {
            content.push('\n');
        }

        let (output, _input) = post_body_parser()
            .easy_parse(position::Stream::new(content.as_str()))
            .map_err(|err| Error::ParseError(err.to_string()))?;

        Ok(output)
    }

    /// Resolve post references. This adds an URI to the post reference if the
    /// post in question exists.
    pub fn resolve_refs<C>(&mut self, db: &Connection<C>)
    where
        C: InnerConnection,
    {
        for block_item in self.0.iter_mut() {
            match block_item {
                BlockItem::Header(items)
                | BlockItem::Quote(items)
                | BlockItem::Text(items) => {
                    for line_item in items.iter_mut() {
                        if let LineItem::PostRef { id, uri } = line_item {
                            *uri = db.post_uri(*id).ok();
                        }
                    }
                }

                _ => (),
            }
        }
    }

    pub fn into_html(self) -> String {
        format!("{}", html! { : &self })
    }
}

impl Render for PostBody {
    fn render(&self, tmpl: &mut TemplateBuffer) {
        tmpl << html! {
            @ for item in &self.0 {
                : item
            }
        }
    }
}

impl RenderMut for PostBody {
    fn render_mut(&mut self, tmpl: &mut TemplateBuffer) {
        self.render(tmpl)
    }
}

impl RenderOnce for PostBody {
    fn render_once(self, tmpl: &mut TemplateBuffer) {
        self.render(tmpl)
    }
}

/// A block-level item.
#[derive(Debug)]
enum BlockItem {
    Header(Vec<LineItem>),
    Quote(Vec<LineItem>),
    Code {
        contents: String,
        language: Option<String>,
    },
    Text(Vec<LineItem>),
}

impl Render for BlockItem {
    fn render(&self, tmpl: &mut TemplateBuffer) {
        match self {
            BlockItem::Header(items) => {
                tmpl << html! {
                    h3 {
                        @ for item in items {
                            : item
                        }
                    }
                }
            }
            BlockItem::Quote(items) => {
                tmpl << html! {
                    blockquote {
                        p {
                            @ for item in items {
                                : item
                           }
                        }
                    }
                }
            }
            BlockItem::Code { contents, language } => {
                tmpl << html! {
                    pre(class="blockcode") {
                        @ if let Some(language) = language {
                            code(class=format!("language-{}", language)) {
                                : contents
                            }
                        } else {
                            code { : contents }
                        }
                    }
                }
            }
            BlockItem::Text(items) => {
                tmpl << html! {
                    p {
                        @ for item in items {
                            : item
                        }
                    }
                }
            }
        }
    }
}

impl RenderMut for BlockItem {
    fn render_mut(&mut self, tmpl: &mut TemplateBuffer) {
        self.render(tmpl)
    }
}

impl RenderOnce for BlockItem {
    fn render_once(self, tmpl: &mut TemplateBuffer) {
        self.render(tmpl)
    }
}

/// An item within a line.
///
/// Specifically, these items can appear within headers, quotes, and normal text
/// blocks.
#[derive(Debug)]
enum LineItem {
    Strong(String),
    Emphasis(String),
    Spoiler(String),
    PostRef { id: PostId, uri: Option<String> },
    Link(String),
    Code(String),
    Text(String),
}

impl Render for LineItem {
    fn render(&self, tmpl: &mut TemplateBuffer) {
        match self {
            LineItem::Strong(s) => tmpl << html! { strong { : s } },
            LineItem::Emphasis(s) => tmpl << html! { em { : s } },
            LineItem::Spoiler(s) => {
                tmpl << html! {
                    span(class = "spoiler") {
                        : s
                    }
                }
            }
            LineItem::PostRef { id, uri } => {
                if let Some(uri) = uri {
                    tmpl << html! {
                        a(class = "post-ref", href = (uri)) {
                            : id
                        }
                    }
                } else {
                    tmpl << html! { a(class = "post-ref") { : id } }
                }
            }
            LineItem::Link(s) => {
                tmpl << html! {
                    a(href = s, rel = "nofollow noopener", target = "_blank") {
                        : s
                    }
                }
            }
            LineItem::Code(s) => tmpl << html! { code { : s } },
            LineItem::Text(s) => tmpl << html! { : s },
        }
    }
}

impl RenderMut for LineItem {
    fn render_mut(&mut self, tmpl: &mut TemplateBuffer) {
        self.render(tmpl)
    }
}

impl RenderOnce for LineItem {
    fn render_once(self, tmpl: &mut TemplateBuffer) {
        self.render(tmpl)
    }
}

#[cfg(test)]
mod tests {
    use super::PostBody;
    use crate::Result;

    fn test_parse<S1, S2>(input: S1, expected_output: S2) -> Result<()>
    where
        S1: Into<String> + std::fmt::Debug,
        S2: AsRef<str> + std::fmt::Debug,
    {
        let body = PostBody::parse(input.into(), &[])?;

        assert_eq!(body.into_html(), expected_output.as_ref());

        Ok(())
    }

    #[test]
    fn strong() -> Result<()> {
        test_parse("**supercomputer**", "<p><strong>supercomputer</strong></p>")
    }

    #[test]
    fn empasis() -> Result<()> {
        test_parse("*harm*", "<p><em>harm</em></p>")
    }

    #[test]
    fn spoiler() -> Result<()> {
        test_parse("~ranting~", "<p><span class=\"spoiler\">ranting</span></p>")
    }

    #[test]
    fn post_ref() -> Result<()> {
        test_parse(">>1729", "<p><a class=\"post-ref\">1729</a></p>")
    }

    #[test]
    fn link() -> Result<()> {
        test_parse("https://lainchan.org", "<p><a href=\"https://lainchan.org\" rel=\"nofollow noopener\" target=\"_blank\">https://lainchan.org</a></p>")
    }

    #[test]
    fn header() -> Result<()> {
        test_parse(
            "# The hardships of artistry",
            "<h3>The hardships of artistry</h3>",
        )
    }

    #[test]
    fn quote() -> Result<()> {
        let input = "> When I was ten, I read fairy tales in secret and would have been ashamed if I had been found doing so. Now that I am fifty, I read them openly. When I became a man I put away childish things, including the fear of childishness and the desire to be very grown up.";

        let expected_output = "<blockquote><p>When I was ten, I read fairy tales in secret and would have been ashamed if I had been found doing so. Now that I am fifty, I read them openly. When I became a man I put away childish things, including the fear of childishness and the desire to be very grown up.</p></blockquote>";

        test_parse(input, expected_output)
    }

    #[test]
    fn code() -> Result<()> {
        let input = "```
if(f(a)||f(b)||f(c)
      ||f(d)||...f(x)
      ||f(y)||f(z)
                     ){
      dostuff();
} 
```";

        let expected_output =
            "<pre class=\"blockcode\"><code>if(f(a)||f(b)||f(c)
      ||f(d)||...f(x)
      ||f(y)||f(z)
                     ){
      dostuff();
} 
</code></pre>";

        test_parse(input, expected_output)
    }

    #[test]
    fn code_with_language() -> Result<()> {
        let input = "``` C
if (very_long_function_name(city_1)
  || very_long_function_name(city_2)
  || very_long_function_name(city_3)
  ...
  || very_long_function_name(city_n)) {
    do_stuff();
} 
```";

        let expected_output = "<pre class=\"blockcode\"><code class=\"language-C\">if (very_long_function_name(city_1)
  || very_long_function_name(city_2)
  || very_long_function_name(city_3)
  ...
  || very_long_function_name(city_n)) {
    do_stuff();
} 
</code></pre>";

        test_parse(input, expected_output)
    }

    #[test]
    fn text() -> Result<()> {
        let input = "I have resources aplenty but I want to know more about how the structure of these sites could be characterized, what design philosophies they followed, what software was used to make them, where their image and animated resources typically came from and what programs were used to create these images/animations.";

        let expected_output = "<p>I have resources aplenty but I want to know more about how the structure of these sites could be characterized, what design philosophies they followed, what software was used to make them, where their image and animated resources typically came from and what programs were used to create these images/animations.</p>";

        test_parse(input, expected_output)
    }

    #[test]
    fn text_multiline() -> Result<()> {
        let input = "Anyone know any good guides to writing text-based adventure games or even MUDs?
I'll take a look at OP's project and give it a whirl when I get home from work. Looks pretty cool!";

        let expected_output = "<p>Anyone know any good guides to writing text-based adventure games or even MUDs?</p><p>I'll take a look at OP's project and give it a whirl when I get home from work. Looks pretty cool!</p>";

        test_parse(input, expected_output)
    }
}
