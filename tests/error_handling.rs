use scrapely::{ExtractionError, Item, ItemTrait};
use scraper::Html;

#[derive(Debug, Item)]
#[item(selector = ".product")]
struct Product {
    #[field(selector = ".name")]
    name: String,

    #[field(selector = ".price")]
    price: f64,
}

#[test]
fn test_missing_required_field() {
    let html = Html::parse_document(
        r#"
        <div class="product">
            <span class="name">Widget</span>
            <!-- price is missing -->
        </div>
    "#,
    );

    let result = Product::extract(&html.root_element());

    assert!(result.is_err());
    match result.unwrap_err() {
        ExtractionError::MissingField { field, selector } => {
            assert_eq!(field, "price");
            assert_eq!(selector, ".price");
        }
        _ => panic!("Expected MissingField error"),
    }
}

#[test]
fn test_parse_error() {
    let html = Html::parse_document(
        r#"
        <div class="product">
            <span class="name">Widget</span>
            <span class="price">not a number</span>
        </div>
    "#,
    );

    let result = Product::extract(&html.root_element());

    assert!(result.is_err());
    match result.unwrap_err() {
        ExtractionError::ParseError { field, text, .. } => {
            assert_eq!(field, "price");
            assert_eq!(text, "not a number");
        }
        _ => panic!("Expected ParseError"),
    }
}

#[test]
fn test_missing_element() {
    let html = Html::parse_document(
        r#"
        <div>
            <!-- No product element at all -->
        </div>
    "#,
    );

    let result = Product::extract(&html.root_element());

    assert!(result.is_err());
    match result.unwrap_err() {
        ExtractionError::MissingField { field, selector } => {
            assert_eq!(field, "name");
            assert_eq!(selector, ".name");
        }
        _ => panic!("Expected MissingField error"),
    }
}

#[test]
fn test_optional_field_no_error() {
    #[derive(Debug, Item)]
    #[item(selector = ".user")]
    struct User {
        #[field(selector = ".name")]
        name: String,

        #[field(selector = ".email")]
        email: Option<String>,
    }

    let html = Html::parse_document(
        r#"
        <div class="user">
            <span class="name">John</span>
            <!-- email is missing but it's optional -->
        </div>
    "#,
    );

    let result = User::extract(&html.root_element());

    assert!(result.is_ok());
    let user = result.unwrap();
    assert_eq!(user.name, "John");
    assert_eq!(user.email, None);
}

#[test]
fn test_optional_field_parse_failure() {
    #[derive(Debug, Item)]
    #[item(selector = ".data")]
    struct Data {
        #[field(selector = ".value")]
        value: Option<i32>,
    }

    let html = Html::parse_document(
        r#"
        <div class="data">
            <span class="value">not a number</span>
        </div>
    "#,
    );

    let result = Data::extract(&html.root_element());

    // Optional fields return None on parse failure, not an error
    assert!(result.is_ok());
    let data = result.unwrap();
    assert_eq!(data.value, None);
}

#[test]
fn test_vec_field_parse_error() {
    #[derive(Debug, Item)]
    #[item(selector = ".numbers")]
    struct Numbers {
        #[field(selector = ".num")]
        values: Vec<i32>,
    }

    let html = Html::parse_document(
        r#"
        <div class="numbers">
            <span class="num">1</span>
            <span class="num">not a number</span>
            <span class="num">3</span>
        </div>
    "#,
    );

    let result = Numbers::extract(&html.root_element());

    // Vec fields should error if any element fails to parse
    assert!(result.is_err());
    match result.unwrap_err() {
        ExtractionError::ParseError { field, text, .. } => {
            assert_eq!(field, "values");
            assert_eq!(text, "not a number");
        }
        _ => panic!("Expected ParseError"),
    }
}

#[test]
fn test_attribute_missing() {
    #[derive(Debug, Item)]
    #[item(selector = ".link-container")]
    struct Link {
        #[field(selector = "a", attr = "href")]
        url: String,
    }

    let html = Html::parse_document(
        r#"
        <div class="link-container">
            <a>Link without href</a>
        </div>
    "#,
    );

    let result = Link::extract(&html.root_element());

    assert!(result.is_err());
    match result.unwrap_err() {
        ExtractionError::MissingField { field, selector } => {
            assert_eq!(field, "url");
            // The selector should indicate it's looking for the href attribute
            assert!(selector.contains("a") && selector.contains("href"));
        }
        _ => panic!("Expected MissingField error"),
    }
}

#[test]
fn test_error_context_includes_field_info() {
    let html = Html::parse_document(
        r#"
        <div class="product">
            <span class="name">Widget</span>
            <span class="price">invalid</span>
        </div>
    "#,
    );

    let result = Product::extract(&html.root_element());

    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_message = error.to_string();

    // Verify error message contains useful context
    assert!(error_message.contains("price"));
    assert!(error_message.contains("invalid"));
}
