use scrapely::Item; // or your actual path to the Item derive

#[derive(Debug, Clone, PartialEq, Eq, Item)]
#[item(selector = ".quote")]
struct QuotesItem {
    #[field(selector = "span.text")]
    text: Option<String>,

    #[field(selector = "small.author")]
    author: Option<String>,
}

fn main() {
    println!("Hello, world!");
}
