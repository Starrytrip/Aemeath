//! Markdown 转纯文本，不执行 MDX；按标题分段，段内保留少量重叠。
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

pub const CHUNK_CHARS: usize = 800;
const OVERLAP: usize = 100;

#[derive(Debug)]
pub struct Chunk {
    pub heading: String,
    pub content: String,
}

pub fn split(markdown: &str) -> Vec<Chunk> {
    let mut sections = Vec::new();
    let mut heading = String::new();
    let mut text = String::new();
    let mut in_heading = false;
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                if !text.trim().is_empty() {
                    sections.push((heading.clone(), std::mem::take(&mut text)));
                }
                heading.clear();
                in_heading = true;
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
            }
            Event::Text(value) | Event::Code(value) => {
                if in_heading {
                    heading.push_str(&value);
                } else {
                    text.push_str(&value);
                }
            }
            Event::SoftBreak
            | Event::HardBreak
            | Event::End(TagEnd::Paragraph)
            | Event::End(TagEnd::CodeBlock)
            | Event::End(TagEnd::Item) => text.push('\n'),
            _ => {}
        }
    }
    if !text.trim().is_empty() {
        sections.push((heading, text));
    }
    let mut result = Vec::new();
    for (heading, content) in sections {
        let chars: Vec<char> = content.trim().chars().collect();
        let mut start = 0;
        while start < chars.len() {
            let end = (start + CHUNK_CHARS).min(chars.len());
            let content: String = chars[start..end].iter().collect();
            if !content.trim().is_empty() {
                result.push(Chunk {
                    heading: heading.clone(),
                    content,
                });
            }
            if end == chars.len() {
                break;
            }
            start = end - OVERLAP;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_chunks_keep_headings_and_code_but_discard_html() {
        let chunks = split(&format!(
            "# 第一章\n\n{}\n\n# 第二章\n\n`cargo check`\n<script>ignored</script>",
            "文".repeat(1800)
        ));
        assert!(chunks
            .iter()
            .all(|c| c.content.chars().count() <= CHUNK_CHARS));
        assert_eq!(chunks[0].heading, "第一章");
        assert_eq!(chunks.last().unwrap().heading, "第二章");
        assert!(chunks.last().unwrap().content.contains("cargo check"));
        assert!(!chunks.last().unwrap().content.contains("ignored"));
        assert_eq!(chunks.iter().filter(|c| c.heading == "第一章").count(), 3);
    }
}
