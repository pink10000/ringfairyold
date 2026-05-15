use std::{error::Error, fs::File, io::stdout};

use chrono::{DateTime, Utc};
use feed_rs::{model::Link, parser};
use ringfairy::website::Website;
use serde::Serialize;
use serde_json::to_writer;

use crate::discord::{Author, Embed, Message};

#[derive(Debug, Serialize)]
struct Post {
    blog_title: Option<String>,
    blog_url: Option<String>,
    url: String,
    title: String,
    description: Option<String>,
    tags: Vec<String>,
    timestamp: DateTime<Utc>,
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

    let mut posts = feeds
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
                    timestamp: entry.published.or(entry.updated)?,
                })
            })
        })
        .collect::<Vec<_>>();
    posts.sort_by_key(|post| post.timestamp);

    for embed_group in posts
        .into_iter()
        .map(|post| Embed {
            title: Some(post.title),
            description: post.description,
            url: Some(post.url),
            timestamp: Some(post.timestamp.to_rfc3339()),
            color: Some(0x123456),
            author: post.blog_title.map(|title| Author {
                name: title,
                url: post.blog_url,
            }),
        })
        .collect::<Vec<_>>()
        .chunks(10)
    {
        to_writer(stdout(), &create_message(embed_group.to_vec()))?;
    }

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

fn create_message(embeds: Vec<Embed>) -> Message {
    Message {
        username: None,
        avatar_url: None,
        content: None,
        embeds,
    }
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
        pub color: Option<u32>,
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
