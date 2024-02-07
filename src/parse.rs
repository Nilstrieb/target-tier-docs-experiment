//! Suboptimal half-markdown parser that's just good-enough for this.

use eyre::{bail, OptionExt, Result, WrapErr};
use std::{fs::DirEntry, path::Path};

#[derive(Debug, PartialEq)]
pub struct ParsedTargetInfoFile {
    pub pattern: String,
    pub tier: Option<String>,
    pub maintainers: Vec<String>,
    pub sections: Vec<(String, String)>,
}

#[derive(serde::Deserialize)]
struct Frontmatter {
    tier: Option<String>,
    #[serde(default)]
    maintainers: Vec<String>,
}

pub fn load_target_infos(directory: &Path) -> Result<Vec<ParsedTargetInfoFile>> {
    let dir = std::fs::read_dir(directory).unwrap();
    let mut infos = Vec::new();

    for entry in dir {
        let entry = entry?;
        infos.push(
            load_single_target_info(&entry)
                .wrap_err_with(|| format!("loading {}", entry.path().display()))?,
        )
    }

    Ok(infos)
}

fn load_single_target_info(entry: &DirEntry) -> Result<ParsedTargetInfoFile> {
    let pattern = entry.file_name();
    let name = pattern
        .to_str()
        .ok_or_eyre("file name is invalid utf8")?
        .strip_suffix(".md")
        .ok_or_eyre("target_info files must end with .md")?;
    let content: String = std::fs::read_to_string(entry.path()).wrap_err("reading content")?;

    parse_file(name, &content)
}

fn parse_file(name: &str, content: &str) -> Result<ParsedTargetInfoFile> {
    let mut frontmatter_splitter = content.split("---\n");

    let frontmatter = frontmatter_splitter
        .nth(1)
        .ok_or_eyre("missing frontmatter")?;

    let frontmatter =
        serde_yaml::from_str::<Frontmatter>(frontmatter).wrap_err("invalid frontmatter")?;

    let body = frontmatter_splitter.next().ok_or_eyre("no body")?;

    let mut sections = Vec::new();

    for line in body.lines() {
        if line.starts_with("#") {
            if let Some(header) = line.strip_prefix("## ") {
                if !crate::SECTIONS.contains(&header) {
                    bail!(
                        "`{header}` is not an allowed section name, must be one of {:?}",
                        super::SECTIONS
                    );
                }
                sections.push((header.to_owned(), String::new()));
            } else {
                bail!("the only allowed headings are `## `");
            }
        } else {
            match sections.last_mut() {
                Some((_, content)) => {
                    content.push_str(line);
                    content.push('\n');
                }
                None if line.trim().is_empty() => {}
                None => bail!("line with content not allowed before the first heading"),
            }
        }
    }

    sections
        .iter_mut()
        .for_each(|section| section.1 = section.1.trim().to_owned());

    Ok(ParsedTargetInfoFile {
        pattern: name.to_owned(),
        maintainers: frontmatter.maintainers,
        tier: frontmatter.tier,
        sections,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_frontmatter() {
        let name = "archlinux-unknown-linux-gnu.md"; // arch linux is an arch, right?
        let content = "";
        assert!(super::parse_file(name, content).is_err());
    }

    #[test]
    fn invalid_section() {
        let name = "6502-nintendo-nes.md";
        let content = "
---
---

## Not A Real Section
";

        assert!(super::parse_file(name, content).is_err());
    }

    #[test]
    fn wrong_header() {
        let name = "x86_64-known-linux-gnu.md";
        let content = "
---
---

# x86_64-known-linux-gnu
";

        assert!(super::parse_file(name, content).is_err());
    }
    
    #[test]
    fn parse_correctly() {
        let name = "cat-unknown-linux-gnu.md";
        let content = r#"
---
tier: "1" # first-class cats
maintainers: ["who maintains the cat?"]
---
## Requirements

This target mostly just meows and doesn't do much.

## Testing

You can pet the cat and it might respond positively.

## Cross compilation

If you're on a dog system, there might be conflicts with the cat, be careful.
But it should be possible.
        "#;

        let info = super::parse_file(name, content).unwrap();

        assert_eq!(info.maintainers, vec!["who maintains the cat?"]);
        assert_eq!(info.pattern, name);
        assert_eq!(info.tier, Some("1".to_owned()));
        assert_eq!(
            info.sections,
            vec![
                (
                    "Requirements".to_owned(),
                    "This target mostly just meows and doesn't do much.".to_owned(),
                ),
                (
                    "Testing".to_owned(),
                    "You can pet the cat and it might respond positively.".to_owned(),
                ),
                (
                    "Cross compilation".to_owned(),
                    "If you're on a dog system, there might be conflicts with the cat, be careful.\nBut it should be possible.".to_owned(),
                ),
            ]
        );
    }
}