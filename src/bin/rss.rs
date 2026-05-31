use std::{
    collections::HashSet,
    env::{self, args},
    error::Error,
    fs::File,
    process::ExitCode,
};

use chrono::{DateTime, Utc};
use feed_rs::{model::Link, parser};
use itertools::Itertools;
use ringfairy::website::Website;
use serde::Serialize;

use crate::discord::{Author, Embed, Image, Message};

#[derive(Debug, Serialize)]
struct Post {
    blog_title: Option<String>,
    blog_url: Option<String>,
    url: String,
    title: String,
    description: Option<String>,
    image_url: Option<String>,
    tags: Vec<String>,
    timestamp: DateTime<Utc>,
}

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let discord_webhook_url = env::var("DISCORD_WEBHOOK")
        .map_err(|err| format!("environment variable DISCORD_WEBHOOK is missing: {err:?}"))?;
    if discord_webhook_url.is_empty() {
        Err("environment variable DISCORD_WEBHOOK must be nonempty")?;
    }
    if !discord_webhook_url.starts_with("http") {
        Err("DISCORD_WEBHOOK must be a URL")?;
    }

    let seen_urls_str = {
        let mut args = args();
        let executable_name = args.next();
        args.next().ok_or_else(|| {
            format!(
                "usage: {} <urls>\n<urls> may be an empty string",
                executable_name
                    .as_ref()
                    .map(String::as_ref)
                    .unwrap_or("rss")
            )
        })?
    };
    let seen_urls = seen_urls_str.lines().collect::<HashSet<_>>();

    let client = reqwest::blocking::Client::new();

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
            eprintln!("Fetching {feed_url}");
            parser::parse(
                client
                    .get(feed_url)
                    .send()
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
        .collect_vec();

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
                    image_url: entry
                        .media
                        .iter()
                        .flat_map(|media| media.content.iter())
                        .filter_map(|content| content.url.as_ref().map(|url| url.as_str()))
                        .next()
                        .map(String::from),
                    tags: entry
                        .categories
                        .iter()
                        .map(|category| category.term.clone())
                        .collect(),
                    timestamp: entry.published.or(entry.updated)?,
                })
            })
        })
        .filter(|post| !seen_urls.contains(post.url.as_str()))
        .collect_vec();
    posts.sort_by_key(|post| post.timestamp);

    let has_new_posts = !posts.is_empty();
    for post in &posts {
        println!("{}", post.url);
    }

    for embed_group in &posts
        .into_iter()
        .map(|post| Embed {
            title: Some(post.title),
            description: post.description,
            url: Some(post.url),
            timestamp: Some(post.timestamp.to_rfc3339()),
            color: Some(0xee5396),
            image: post.image_url.map(|url| Image { url }),
            author: post.blog_title.map(|title| Author {
                name: title,
                url: post.blog_url,
            }),
        })
        .chunks(10)
    {
        let embeds = embed_group.collect_vec();
        eprintln!("Sending {} post(s) to Discord webhook", embeds.len());
        client
            .post(&discord_webhook_url)
            .json(&create_message(embeds))
            .send()?
            .error_for_status()?;
    }

    let exit_code = if has_new_posts {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    };
    eprintln!("{exit_code:?}");
    Ok(exit_code)
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
        pub image: Option<Image>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub author: Option<Author>,
    }

    #[derive(Serialize, Debug)]
    pub struct Image {
        pub url: String,
    }

    #[derive(Serialize, Debug)]
    pub struct Author {
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
    }
}
