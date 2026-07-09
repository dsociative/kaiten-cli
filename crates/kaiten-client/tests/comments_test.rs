use kaiten_client::KaitenClient;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const COMMENTS_LIST: &str = include_str!("fixtures/comments_list.json");
const COMMENT_ADD: &str = include_str!("fixtures/comment_add.json");

#[tokio::test]
async fn list_parses_comments_with_author() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/67089469/comments"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(COMMENTS_LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let comments = client.comments().list(67089469).await.unwrap();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, 85523991);
    assert_eq!(comments[0].text, "test comment");
    assert_eq!(comments[0].edited, Some(false));
    assert_eq!(comments[0].author_id, Some(1068514));
    let author = comments[0].author.as_ref().unwrap();
    assert_eq!(author.email.as_deref(), Some("user@example.com"));
}

#[tokio::test]
async fn add_posts_text_body_and_parses_comment_without_author() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/comments"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "text": "test comment" })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(COMMENT_ADD, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let comment = client.comments().add(67089469, "test comment").await.unwrap();

    assert_eq!(comment.id, 85523991);
    assert_eq!(comment.text, "test comment");
    assert!(comment.author.is_none());
}
