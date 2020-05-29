//! Parsing for uploaded posts.

use combine::parser::char::*;
use combine::parser::combinator::*;
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
fn inline_spaces<Input>() -> impl Parser<Input, Output = String>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    many(satisfy(|c: char| c.is_whitespace() && c != '\n'))
}

/// Parses in-line text started and ended by `delim`. Characters escaped with a
/// backslash are not counted as the delimiter.
fn inline_delimited<Input, F, P>(
    delim: F,
) -> impl Parser<Input, Output = String>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
    F: Fn() -> P,
    P: Parser<Input>,
    P::Output: Into<
        combine::error::Info<
            <Input as StreamOnce>::Token,
            <Input as StreamOnce>::Range,
            &'static str,
        >,
    >,
{
    let line_char = || satisfy(|c: char| c != '\n' && c != '\\');

    let escaped_line_char = || satisfy(|c: char| c != '\n');

    let inner = many1(choice((
        not_followed_by(attempt(delim())).with(line_char()),
        char('\\').with(escaped_line_char()),
    )));

    delim().with(inner).skip(delim())
}

/// Parse `**strong**` text.
fn strong_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    inline_delimited(|| string("**")).map(LineItem::Strong)
}

/// Parse `*emphasized*` text.
fn emphasis_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    inline_delimited(|| char('*')).map(LineItem::Emphasis)
}

/// Parse `~spoiler~` text.
fn spoiler_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    inline_delimited(|| char('~')).map(LineItem::Spoiler)
}

/// Parse a post ref like `>>123`.
fn post_ref_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    string(">>")
        .with(many1(digit()))
        .map(|s: String| LineItem::PostRef {
            id: s.parse().unwrap(),
            uri: None,
        })
}

/// Parse an HTTP link.
fn link_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    dbg!("link_parser");

    let link_char = || {
        satisfy(|c: char| {
            let special_link_chars = [
                '-', '.', '_', '~', ':', '/', '?', '#', '[', ']', '@', '!',
                '$', '&', '\'', '(', ')', '*', '+', ',', ';', '%', '=',
            ];

            c.is_alphanumeric() || special_link_chars.contains(&c)
        })
    };

    // Most links don't end with special characters, so we omit them from the
    // link, even though technically they're valid URI characters.
    //
    // For example, if a link was at the end of a sentence, this prevents the
    // period for becoming a part of the link.
    let final_link_char = || {
        satisfy(|c: char| {
            let special_link_chars = ['#', '$', '-', '_', '+', '*', '\''];

            c.is_alphanumeric() || special_link_chars.contains(&c)
        })
    };

    attempt(recognize((
        look_ahead(any()).map(|c| dbg!(c)),
        choice((attempt(string("http://")), attempt(string("https://")))),
        many::<String, _, _>(attempt(
            link_char().skip(look_ahead(link_char())),
        )),
        optional(final_link_char()),
    )))
    .map(LineItem::Link)
}

/// Parse `\`code\`` within a line.
fn line_code_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    inline_delimited(|| char('`')).map(LineItem::Code)
}

/// Parse any plain text within a line; anything not covered by the above
/// line-item parsers.
fn line_text_parser<Input>() -> impl Parser<Input, Output = LineItem>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    dbg!("line_text_parser");

    let line_char = || {
        choice((
            // Parse any character other than a backslash literally.
            satisfy(|c: char| c != '\n' && c != '\\'),
            // If a backslash is followed by a character, parse that character
            // literally.
            attempt(char('\\').with(satisfy(|c: char| c != '\n'))),
            // If a backslash is followed by a newline, parse it literally.
            char('\\'),
        ))
    };

    let end = || {
        choice((
            char('*'),
            char('~'),
            char('>'),
            char('`'),
            attempt(string("http://")).map(|_| '\0'),
            attempt(string("https://")).map(|_| '\0'),
            newline(),
        ))
    };

    let not_end = || attempt(not_followed_by(end()).with(line_char()));

    // We parse one character before not_end, because if there is an
    // unmatched end character such as '*' or '~', it is parsed literally.
    //
    // We know that the first character this parser encounters wasn't handled by
    // any other parser because this is the last parser tried.
    line_char()
        .and(many(not_end()))
        .map(|(first, rest): (char, String)| format!("{}{}", first, rest))
        .map(LineItem::Text)
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
        .skip(inline_spaces())
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
        .skip(inline_spaces())
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
        // Needed to differentiate between quotes and post refs.
        .skip(not_followed_by(char('>')))
        .skip(inline_spaces())
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

        // Here we do some preprocessing, so the parser can assume that no lines
        // are empty and all lines end with a newline.

        // Replace web newlines \r\n with normal newlines \n.
        content = Regex::new(r"\r\n")
            .unwrap()
            .replace_all(&content, "\n")
            .into_owned();

        // Remove empty lines.
        content = Regex::new(r"\n+")
            .unwrap()
            .replace_all(&content, "\n")
            .into_owned();

        // Remove an initial newline, if present.
        if content.starts_with('\n') {
            content = String::from(&content[1..]);
        }

        // Add a trailing newline, if not present.
        if !content.ends_with('\n') {
            content.push('\n');
        }

        println!();

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
    fn link_period() -> Result<()> {
        test_parse("Here's a cool site: https://lainchan.org.", "<p>Here's a cool site: <a href=\"https://lainchan.org\" rel=\"nofollow noopener\" target=\"_blank\">https://lainchan.org</a>.</p>")
    }

    #[test]
    fn link_sentence() -> Result<()> {
        test_parse("What do you think of https://lainchan.org? I think it's pretty cool.", "<p>What do you think of <a href=\"https://lainchan.org\" rel=\"nofollow noopener\" target=\"_blank\">https://lainchan.org</a>? I think it's pretty cool.</p>")
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

    #[test]
    fn unmatched_emph() -> Result<()> {
        test_parse("*", "<p>*</p>")
    }

    #[test]
    fn unmatched_code() -> Result<()> {
        test_parse("`", "<p>`</p>")
    }

    #[test]
    fn unmatched_spoiler() -> Result<()> {
        test_parse("~", "<p>~</p>")
    }

    #[test]
    fn escaped_emph() -> Result<()> {
        test_parse(r"*ab\*cd*", "<p><em>ab*cd</em></p>")
    }

    #[test]
    fn escaped_strong() -> Result<()> {
        test_parse(r"**ab\*cd\**ef**", "<p><strong>ab*cd**ef</strong></p>")
    }

    #[test]
    fn escaped_spoiler() -> Result<()> {
        test_parse("~hi\\~~", "<p><span class=\"spoiler\">hi~</span></p>")
    }

    #[test]
    fn escaped_code() -> Result<()> {
        test_parse("`a\\`b`", "<p><code>a`b</code></p>")
    }

    #[test]
    fn fuzz() -> Result<()> {
        use rand::{distributions::Uniform, thread_rng, Rng};

        for _ in 1..50 {
            // Iterate over ASCII values.
            let post_content = thread_rng()
                .sample_iter(Uniform::new_inclusive(0, 127))
                .map(|byte: u8| char::from(byte))
                .filter(|c| c.is_ascii_graphic() || *c == ' ' || *c == '\n')
                .take(100)
                .collect::<String>();

            if let Err(e) = PostBody::parse(&post_content, &[]) {
                println!("Post content: {:?}", post_content);
                return Err(e);
            }
        }

        Ok(())
    }

    #[test]
    #[ignore]
    fn fuzz_more() -> Result<()> {
        use rand::{distributions::Uniform, thread_rng, Rng};

        for _ in 1..1000 {
            // Iterate over visible ASCII values.
            let post_content = thread_rng()
                .sample_iter(Uniform::new_inclusive(0, 127))
                .map(|byte: u8| char::from(byte))
                .filter(|c| c.is_ascii_graphic() || *c == ' ' || *c == '\n')
                .take(100)
                .collect::<String>();

            if let Err(e) = PostBody::parse(&post_content, &[]) {
                println!("Post content: {:?}", post_content);
                return Err(e);
            }
        }

        Ok(())
    }

    #[test]
    #[ignore]
    fn fuzz_ascii() -> Result<()> {
        use rand::{distributions::Uniform, thread_rng, Rng};

        for _ in 1..1000 {
            // Iterate over all ASCII values.
            let post_content = thread_rng()
                .sample_iter(Uniform::new_inclusive(0, 127))
                .map(|byte: u8| char::from(byte))
                .take(100)
                .collect::<String>();

            if let Err(e) = PostBody::parse(&post_content, &[]) {
                println!("Post content: {:?}", post_content);
                return Err(e);
            }
        }

        Ok(())
    }

    #[test]
    #[ignore]
    fn fuzz_utf8() -> Result<()> {
        use rand::{distributions::Standard, thread_rng, Rng};

        for _ in 1..1000 {
            // Iterate over random UTF8.
            let post_content = thread_rng()
                .sample_iter::<char, _>(Standard)
                .take(100)
                .collect::<String>();

            if let Err(e) = PostBody::parse(&post_content, &[]) {
                println!("Post content: {:?}", post_content);
                return Err(e);
            }
        }

        Ok(())
    }
}
