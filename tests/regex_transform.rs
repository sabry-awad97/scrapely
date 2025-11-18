use scrapely::{Item, ItemTrait};
use scraper::Html;

#[derive(Debug, Item)]
#[item(selector = ".data")]
struct DataWithRegex {
    #[field(selector = ".text", regex = r#""(.+)""#)]
    text: String,
}

#[test]
fn test_regex_extraction() {
    let html = Html::parse_document(
        r#"
        <div class="data">
            <span class="text">"Hello, World!"</span>
        </div>
    "#,
    );

    let data = DataWithRegex::extract(&html.root_element()).expect("Failed to extract");

    // Should extract text without quotes
    assert_eq!(data.text, "Hello, World!");
}

fn uppercase_transform(s: String) -> String {
    s.to_uppercase()
}

#[derive(Debug, Item)]
#[item(selector = ".data")]
struct DataWithTransform {
    #[field(selector = ".text", transform = "uppercase_transform")]
    text: String,
}

#[test]
fn test_transform() {
    let html = Html::parse_document(
        r#"
        <div class="data">
            <span class="text">hello world</span>
        </div>
    "#,
    );

    let data = DataWithTransform::extract(&html.root_element()).expect("Failed to extract");

    assert_eq!(data.text, "HELLO WORLD");
}

#[derive(Debug, Item)]
#[item(selector = ".data")]
struct DataWithBoth {
    #[field(
        selector = ".text",
        regex = r#""(.+)""#,
        transform = "uppercase_transform"
    )]
    text: String,
}

#[test]
fn test_regex_and_transform() {
    let html = Html::parse_document(
        r#"
        <div class="data">
            <span class="text">"hello world"</span>
        </div>
    "#,
    );

    let data = DataWithBoth::extract(&html.root_element()).expect("Failed to extract");

    // Should extract without quotes and then uppercase
    assert_eq!(data.text, "HELLO WORLD");
}
