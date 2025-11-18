use scrapely::{Item, ItemTrait};
use scraper::Html;

#[derive(Debug, Item, PartialEq)]
#[item(selector = ".comment")]
struct Comment {
    #[field(selector = ".author")]
    author: String,

    #[field(selector = ".text")]
    text: String,

    #[field(selector = ".likes")]
    likes: i32,
}

#[derive(Debug, Item)]
#[item(selector = ".post")]
struct Post {
    #[field(selector = ".title")]
    title: String,

    #[field(selector = ".content")]
    content: String,

    #[field(selector = ".comment")]
    comments: Vec<Comment>,
}

#[test]
fn test_nested_items() {
    let html = Html::parse_document(
        r#"
        <div class="post">
            <h1 class="title">My Blog Post</h1>
            <p class="content">This is the content of my post.</p>
            <div class="comment">
                <span class="author">Alice</span>
                <span class="text">Great post!</span>
                <span class="likes">5</span>
            </div>
            <div class="comment">
                <span class="author">Bob</span>
                <span class="text">Thanks for sharing.</span>
                <span class="likes">3</span>
            </div>
        </div>
    "#,
    );

    let post = Post::extract(&html.root_element()).expect("Failed to extract post");

    assert_eq!(post.title, "My Blog Post");
    assert_eq!(post.content, "This is the content of my post.");
    assert_eq!(post.comments.len(), 2);

    assert_eq!(post.comments[0].author, "Alice");
    assert_eq!(post.comments[0].text, "Great post!");
    assert_eq!(post.comments[0].likes, 5);

    assert_eq!(post.comments[1].author, "Bob");
    assert_eq!(post.comments[1].text, "Thanks for sharing.");
    assert_eq!(post.comments[1].likes, 3);
}

#[test]
fn test_optional_nested_item() {
    #[derive(Debug, Item)]
    #[item(selector = ".author")]
    struct Author {
        #[field(selector = ".name")]
        name: String,

        #[field(selector = ".bio")]
        bio: String,
    }

    #[derive(Debug, Item)]
    #[item(selector = ".article")]
    struct Article {
        #[field(selector = ".title")]
        title: String,

        #[field(selector = ".author")]
        author: Option<Author>,
    }

    // Test with author present
    let html_with_author = Html::parse_document(
        r#"
        <div class="article">
            <h1 class="title">Article Title</h1>
            <div class="author">
                <span class="name">John Doe</span>
                <span class="bio">Software developer</span>
            </div>
        </div>
    "#,
    );

    let article =
        Article::extract(&html_with_author.root_element()).expect("Failed to extract article");

    assert_eq!(article.title, "Article Title");
    assert!(article.author.is_some());
    assert_eq!(article.author.as_ref().unwrap().name, "John Doe");

    // Test without author
    let html_without_author = Html::parse_document(
        r#"
        <div class="article">
            <h1 class="title">Article Title</h1>
        </div>
    "#,
    );

    let article =
        Article::extract(&html_without_author.root_element()).expect("Failed to extract article");

    assert_eq!(article.title, "Article Title");
    assert!(article.author.is_none());
}

#[test]
fn test_deeply_nested_items() {
    #[derive(Debug, Item, PartialEq)]
    #[item(selector = ".reply")]
    struct Reply {
        #[field(selector = ".author")]
        author: String,

        #[field(selector = ".text")]
        text: String,
    }

    #[derive(Debug, Item)]
    #[item(selector = ".comment")]
    struct CommentWithReplies {
        #[field(selector = ".author")]
        author: String,

        #[field(selector = ".text")]
        text: String,

        #[field(selector = ".reply")]
        replies: Vec<Reply>,
    }

    #[derive(Debug, Item)]
    #[item(selector = ".thread")]
    struct Thread {
        #[field(selector = ".title")]
        title: String,

        #[field(selector = ".comment")]
        comments: Vec<CommentWithReplies>,
    }

    let html = Html::parse_document(
        r#"
        <div class="thread">
            <h1 class="title">Discussion Thread</h1>
            <div class="comment">
                <span class="author">Alice</span>
                <span class="text">First comment</span>
                <div class="reply">
                    <span class="author">Bob</span>
                    <span class="text">Reply to Alice</span>
                </div>
                <div class="reply">
                    <span class="author">Charlie</span>
                    <span class="text">Another reply</span>
                </div>
            </div>
            <div class="comment">
                <span class="author">Dave</span>
                <span class="text">Second comment</span>
            </div>
        </div>
    "#,
    );

    let thread = Thread::extract(&html.root_element()).expect("Failed to extract thread");

    assert_eq!(thread.title, "Discussion Thread");
    assert_eq!(thread.comments.len(), 2);

    assert_eq!(thread.comments[0].author, "Alice");
    assert_eq!(thread.comments[0].replies.len(), 2);
    assert_eq!(thread.comments[0].replies[0].author, "Bob");

    assert_eq!(thread.comments[1].author, "Dave");
    assert_eq!(thread.comments[1].replies.len(), 0);
}
