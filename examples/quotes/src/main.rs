use anyhow::Result;
use scrapely::{Item, ItemTrait};

#[derive(Debug, Item)]
#[item(selector = ".quote")]
struct Quote {
    #[field(selector = "span.text")]
    text: String,

    #[field(selector = "small.author")]
    author: String,

    #[field(selector = ".tags .tag")]
    tags: Vec<String>,
}

fn main() -> Result<()> {
    println!("Fetching quotes from https://quotes.toscrape.com/page/1/...\n");

    // Fetch the HTML from the website
    let response = reqwest::blocking::get("https://quotes.toscrape.com/page/1/")?;
    let html_content = response.text()?;

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
