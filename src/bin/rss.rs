use std::{error::Error, fs::File, io::stdout};

use feed_rs::{model::Link, parser};
use ringfairy::website::Website;
use serde::Serialize;
use serde_json::to_writer;

use crate::discord::Message;

#[derive(Debug, Serialize)]
struct Post {
    blog_title: Option<String>,
    blog_url: Option<String>,
    url: String,
    title: String,
    description: Option<String>,
    tags: Vec<String>,
    timestamp: i64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let websites: Vec<Website> = serde_json::from_reader(File::open("websites.json")?)?;
    let feeds = websites
        .iter()
        .filter_map(|website| {
            let display_name = website
                .name
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("(no name)");
            let feed_url = website.atom.as_ref().or(website.rss.as_ref())?;
            parser::parse(
                reqwest::blocking::get(feed_url)
                    .map_err(|err| {
                        eprintln!(
                            "Failed to fetch {}'s feed ({}): {:?}",
                            display_name, feed_url, err
                        );
                    })
                    .ok()?,
            )
            .map_err(|err| {
                eprintln!(
                    "Failed to parse {}'s feed ({}): {:?}",
                    display_name, feed_url, err
                );
            })
            .ok()
        })
        .collect::<Vec<_>>();

    let posts = feeds
        .iter()
        .flat_map(|blog| {
            blog.entries.iter().filter_map(|entry| {
                Some(Post {
                    blog_title: blog.title.as_ref().map(|title| title.content.clone()),
                    blog_url: get_url(&blog.links),
                    url: get_url(&entry.links)?,
                    title: entry.title.as_ref()?.content.clone(),
                    description: entry
                        .summary
                        .as_ref()
                        .map(|summary| summary.content.clone())
                        .filter(|desc| desc.len() <= 500),
                    tags: entry
                        .categories
                        .iter()
                        .map(|category| category.term.clone())
                        .collect(),
                    timestamp: entry.published.or(entry.updated)?.timestamp_millis(),
                })
            })
        })
        .collect::<Vec<_>>();

    to_writer(stdout(), &posts)?;

    Ok(())
}

fn get_url(links: &[Link]) -> Option<String> {
    links
        .iter()
        .find(|link| {
            !link
                .media_type
                .as_ref()
                .is_some_and(|media_type| media_type != "text/html")
        })
        .map(|link| link.href.clone())
}

mod discord {
    use serde::Serialize;

    #[derive(Serialize, Debug)]
    pub struct Message {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub username: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub avatar_url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub content: Option<String>,
        pub embeds: Vec<Embed>,
    }

    #[derive(Serialize, Debug)]
    pub struct Embed {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub timestamp: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub color: Option<u16>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub author: Option<Author>,
    }

    #[derive(Serialize, Debug)]
    pub struct Author {
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
    }
}
