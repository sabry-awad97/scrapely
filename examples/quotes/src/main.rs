use scrapely::{Item, ItemTrait};
use scraper::Html;

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

fn main() {
    // Sample HTML from quotes.toscrape.com
    let html = Html::parse_document(
        r#"
        <!DOCTYPE html>
        <html>
        <body>
            <div class="quote">
                <span class="text">"The world as we have created it is a process of our thinking. It cannot be changed without changing our thinking."</span>
                <small class="author">Albert Einstein</small>
                <div class="tags">
                    <span class="tag">change</span>
                    <span class="tag">deep-thoughts</span>
                    <span class="tag">thinking</span>
                </div>
            </div>
            
            <div class="quote">
                <span class="text">"It is our choices, Harry, that show what we truly are, far more than our abilities."</span>
                <small class="author">J.K. Rowling</small>
                <div class="tags">
                    <span class="tag">abilities</span>
                    <span class="tag">choices</span>
                </div>
            </div>
            
            <div class="quote">
                <span class="text">"There are only two ways to live your life. One is as though nothing is a miracle. The other is as though everything is a miracle."</span>
                <small class="author">Albert Einstein</small>
                <div class="tags">
                    <span class="tag">inspirational</span>
                    <span class="tag">life</span>
                    <span class="tag">live</span>
                    <span class="tag">miracle</span>
                </div>
            </div>
        </body>
        </html>
    "#,
    );

    println!("Scraping quotes...\n");

    // Extract all quotes from the page using the selector from #[item(selector = ".quote")]
    match Quote::extract_all(&html.root_element()) {
        Ok(quotes) => {
            println!("Found {} quotes:\n", quotes.len());

            for (i, quote) in quotes.iter().enumerate() {
                println!("Quote #{}:", i + 1);
                println!("  Text: {}", quote.text);
                println!("  Author: {}", quote.author);
                println!("  Tags: {}", quote.tags.join(", "));
                println!();
            }
        }
        Err(e) => {
            eprintln!("Error extracting quotes: {}", e);
            std::process::exit(1);
        }
    }
}
