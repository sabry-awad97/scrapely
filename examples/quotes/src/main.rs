use anyhow::Result;
use scrapely::{Item, ItemTrait};

const URL: &str = "https://quotes.toscrape.com/page/1/";

#[derive(Debug, Item)]
#[item(selector = ".quote")]
struct Quote {
    #[field(selector = "span.text", regex = r#"["“](.+)["”]"#)]
    text: String,

    #[field(selector = "small.author")]
    author: String,

    #[field(selector = ".tags .tag")]
    tags: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Fetching quotes from https://quotes.toscrape.com/page/1/...\n");

    // Fetch the HTML from the website
    let response = reqwest::get(URL).await?;
    let html_content = response.text().await?;

    println!("Scraping quotes...\n");

    // Extract all quotes directly from the HTML string
    let quotes = Quote::from_html(&html_content)?;

    println!("Found {} quotes:\n", quotes.len());

    for (i, quote) in quotes.iter().enumerate() {
        println!("Quote #{}:", i + 1);
        println!("  Text: {}", quote.text);
        println!("  Author: {}", quote.author);
        println!("  Tags: {}", quote.tags.join(", "));
        println!();
    }

    Ok(())
}
