use combine::parser::char::*;
use combine::parser::repeat::*;
use combine::stream::position;
use combine::*;

use horrorshow::html;
use horrorshow::prelude::*;

use regex::Regex;

use crate::config::FilterRule;
use crate::models::{Database, PostId};
use crate::{Error, Result};

parser! {
    fn line_spaces[Input]()(Input) -> String
    where [Input: Stream<Token = char>]
    {
        many(satisfy(|c: char| c.is_whitespace() && c != '\n'))
    }
}

parser! {
    fn strong_parser[Input]()(Input) -> LineItem
    where [Input: Stream<Token = char>]
    {
        let p = not_followed_by(attempt(string("**"))).with(any());

        string("**")
            .with(many1(p))
            .skip(string("**"))
            .map(LineItem::Strong)
    }
}

parser! {
    fn emphasis_parser[Input]()(Input) -> LineItem
    where [Input: Stream<Token = char>]
    {
        let p = not_followed_by(char('*')).with(any());

        char('*').with(many1(p)).skip(char('*')).map(LineItem::Emphasis)
    }
}

parser! {
    fn spoiler_parser[Input]()(Input) -> LineItem
    where [Input: Stream<Token = char>]
    {
        let p = not_followed_by(char('~')).with(any());

        char('~').with(many1(p)).skip(char('~')).map(LineItem::Spoiler)
    }
}

parser! {
    fn post_ref_parser['a, Input](db: &'a Database)(Input) -> LineItem
    where [Input: Stream<Token = char>]
    {
        string(">>").with(many1(digit())).map(|s: String| {
            let id: PostId = s.parse().unwrap();

            LineItem::PostRef {
                id,
                uri: db.post_uri(id).ok(),
            }
        })
    }
}

parser! {
    fn line_code_parser[Input]()(Input) -> LineItem
    where [Input: Stream<Token = char>]
    {
        let p = not_followed_by(char('`')).with(any());

        char('`')
            .with(many1(p))
            .skip(char('`'))
            .map(LineItem::Code)
    }
}

parser! {
    fn line_text_parser[Input]()(Input) -> LineItem
    where [Input: Stream<Token = char>]
    {
        let end = choice((
            char('*'),
            char('~'),
            char('>'),
            char('`'),
            newline(),
        ));

        many1(not_followed_by(end).with(any())).map(LineItem::Text)
    }
}

parser! {
    fn line_items_parser['a, Input](db: &'a Database)(Input)
        -> Vec<LineItem>
    where [Input: Stream<Token = char>]
    {
        many1(choice((
            attempt(strong_parser()),
            attempt(emphasis_parser()),
            attempt(spoiler_parser()),
            attempt(post_ref_parser(db)),
            attempt(line_code_parser()),
            line_text_parser(),
        )))
    }
}

parser! {
    fn header_parser['a, Input](db: &'a Database)(Input) -> BlockItem
    where [Input: Stream<Token = char>]
    {
        char('#')
            .skip(line_spaces())
            .with(line_items_parser(db))
            .skip(newline())
            .map(BlockItem::Header)
    }
}

parser! {
    fn quote_parser['a, Input](db: &'a Database)(Input) -> BlockItem
    where [Input: Stream<Token = char>]
    {
        char('>')
            .skip(line_spaces())
            .with(line_items_parser(db))
            .skip(newline())
            .map(BlockItem::Quote)
    }
}

parser! {
    fn text_parser['a, Input](db: &'a Database)(Input) -> BlockItem
    where [Input: Stream<Token = char>]
    {
        line_items_parser(db).skip(newline()).map(BlockItem::Text)
    }
}

parser! {
    fn code_parser[Input]()(Input) -> BlockItem
    where [Input: Stream<Token = char>]
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
                    BlockItem::Code { language: None, contents }
                } else {
                    BlockItem::Code { language: Some(lang), contents }
                }
            })
    }
}

parser! {
    fn post_body_parser['a, Input](db: &'a Database)(Input) -> PostBody
    where [Input: Stream<Token = char>]
    {
        many1(choice((
            attempt(code_parser()),
            attempt(header_parser(db)),
            attempt(quote_parser(db)),
            text_parser(db),
        )))
        .skip(eof())
        .map(PostBody)
    }
}

pub struct PostBody(Vec<BlockItem>);

impl PostBody {
    pub fn parse<S>(
        content: S,
        rules: &[FilterRule],
        db: &Database,
    ) -> Result<PostBody>
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

        log::debug!("Parser input: {:?}", &content);

        let (output, _input) = post_body_parser(db)
            .easy_parse(position::Stream::new(content.as_str()))
            .map_err(|err| Error::ParseError(err.to_string()))?;

        Ok(output)
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

#[derive(Debug)]
enum LineItem {
    Strong(String),
    Emphasis(String),
    Spoiler(String),
    PostRef { id: PostId, uri: Option<String> },
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
    use crate::{models::Database, Result};

    fn test_parse<S1, S2>(input: S1, expected_output: S2) -> Result<()>
    where
        S1: Into<String> + std::fmt::Debug,
        S2: AsRef<str> + std::fmt::Debug,
    {
        let body = PostBody::parse(input.into(), &[], &Database::mock())?;

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
