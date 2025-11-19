use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use futures::StreamExt;
use scrapely::{Crawler, Spider};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct HackerNewsStory {
    id: i32,
    title: String,
    url: Option<String>,
    score: Option<i32>,
    by: Option<String>,
    time: Option<i64>,
}

struct HackerNewsSpider {
    item_index: AtomicUsize,
}

impl HackerNewsSpider {
    fn new() -> Self {
        Self {
            item_index: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Spider for HackerNewsSpider {
    type Item = HackerNewsStory;

    type Error = reqwest::Error;

    fn start_urls(&self) -> Vec<String> {
        vec!["https://hacker-news.firebaseio.com/v0/topstories.json".to_string()]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
        // Make a GET request to Hacker News API
        let top_story_ids: Vec<i32> = reqwest::get(url).await?.json().await?;

        // Take the top 10 story IDs
        let top_10_story_ids = top_story_ids.iter().take(10).cloned();

        let data: Vec<Result<_, Self::Error>> = futures::stream::iter(top_10_story_ids)
            .map(|story_id| async move {
                let story_url = format!(
                    "https://hacker-news.firebaseio.com/v0/item/{}.json",
                    story_id
                );
                let story_data: HackerNewsStory = reqwest::get(&story_url).await?.json().await?;
                Ok(story_data)
            })
            .buffer_unordered(10)
            .collect::<Vec<_>>()
            .await;

        let top_stories = data.into_iter().flatten().collect::<Vec<_>>();

        let next_pages_link = vec![];

        Ok((top_stories, next_pages_link))
    }

    async fn process(&self, story: Self::Item) -> Result<(), Self::Error> {
        let i = self.item_index.load(Ordering::SeqCst);

        println!("{}. {} (ID: {})", i + 1, story.title, story.id);

        if let Some(url) = &story.url {
            println!("   URL: {}", url);
        }

        if let Some(score) = story.score {
            println!("   Score: {}", score);
        }

        if let Some(by) = &story.by {
            println!("   Author: {}", by);
        }

        if let Some(time) = story.time {
            // Convert Unix timestamp to a readable date
            if let Some(time_str) = chrono::DateTime::from_timestamp(time, 0) {
                println!("   Time: {}", time_str);
            }
        }

        self.item_index.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    // Build crawler with custom configuration and observer
    let crawler = Crawler::builder()
        .rate_limit(5.0) // 5 requests per second using token bucket
        .crawling_concurrency(2)
        .processing_concurrency(1)
        .build()
        .expect("Failed to build crawler");

    let spider = Arc::new(HackerNewsSpider::new());

    crawler.crawl(spider).await;

    Ok(())
}
