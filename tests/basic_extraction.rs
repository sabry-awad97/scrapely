use scrapely::{Item, ItemTrait};
use scraper::Html;

#[derive(Debug, Item, PartialEq)]
#[item(selector = ".product")]
struct Product {
    #[field(selector = ".name")]
    name: String,

    #[field(selector = ".price")]
    price: f64,

    #[field(selector = ".stock")]
    stock: i32,

    #[field(selector = ".available")]
    available: bool,
}

#[test]
fn test_basic_extraction() {
    let html = Html::parse_document(
        r#"
        <div class="product">
            <span class="name">Widget</span>
            <span class="price">19.99</span>
            <span class="stock">42</span>
            <span class="available">true</span>
        </div>
    "#,
    );

    let product = Product::extract(&html.root_element()).expect("Failed to extract product");

    assert_eq!(product.name, "Widget");
    assert_eq!(product.price, 19.99);
    assert_eq!(product.stock, 42);
    assert_eq!(product.available, true);
}

#[test]
fn test_optional_fields() {
    #[derive(Debug, Item)]
    #[item(selector = ".user")]
    struct User {
        #[field(selector = ".name")]
        name: String,

        #[field(selector = ".email")]
        email: Option<String>,

        #[field(selector = ".phone")]
        phone: Option<String>,
    }

    let html = Html::parse_document(
        r#"
        <div class="user">
            <span class="name">John Doe</span>
            <span class="email">john@example.com</span>
        </div>
    "#,
    );

    let user = User::extract(&html.root_element()).expect("Failed to extract user");

    assert_eq!(user.name, "John Doe");
    assert_eq!(user.email, Some("john@example.com".to_string()));
    assert_eq!(user.phone, None);
}

#[test]
fn test_vec_fields() {
    #[derive(Debug, Item)]
    #[item(selector = ".article")]
    struct Article {
        #[field(selector = ".title")]
        title: String,

        #[field(selector = ".tag")]
        tags: Vec<String>,
    }

    let html = Html::parse_document(
        r#"
        <div class="article">
            <h1 class="title">My Article</h1>
            <span class="tag">rust</span>
            <span class="tag">web</span>
            <span class="tag">scraping</span>
        </div>
    "#,
    );

    let article = Article::extract(&html.root_element()).expect("Failed to extract article");

    assert_eq!(article.title, "My Article");
    assert_eq!(article.tags, vec!["rust", "web", "scraping"]);
}

#[test]
fn test_attribute_extraction() {
    #[derive(Debug, Item)]
    #[item(selector = ".link-container")]
    struct Link {
        #[field(selector = "a", attr = "href")]
        url: String,

        #[field(selector = "a")]
        text: String,
    }

    let html = Html::parse_document(
        r#"
        <div class="link-container">
            <a href="https://example.com">Click here</a>
        </div>
    "#,
    );

    let link = Link::extract(&html.root_element()).expect("Failed to extract link");

    assert_eq!(link.url, "https://example.com");
    assert_eq!(link.text, "Click here");
}

#[test]
fn test_extract_all() {
    let html = Html::parse_document(
        r#"
        <div>
            <div class="product">
                <span class="name">Widget A</span>
                <span class="price">10.00</span>
                <span class="stock">5</span>
                <span class="available">true</span>
            </div>
            <div class="product">
                <span class="name">Widget B</span>
                <span class="price">20.00</span>
                <span class="stock">10</span>
                <span class="available">false</span>
            </div>
            <div class="product">
                <span class="name">Widget C</span>
                <span class="price">30.00</span>
                <span class="stock">15</span>
                <span class="available">true</span>
            </div>
        </div>
    "#,
    );

    let products = Product::extract_all(&html.root_element()).expect("Failed to extract products");

    assert_eq!(products.len(), 3);
    assert_eq!(products[0].name, "Widget A");
    assert_eq!(products[1].name, "Widget B");
    assert_eq!(products[2].name, "Widget C");
}
