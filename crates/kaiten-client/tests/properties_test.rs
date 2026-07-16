use kaiten_client::{KaitenClient, UpdateCard};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_hits_company_custom_properties() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/company/custom-properties"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"[{"id": 612634, "name": "Priority", "type": "select", "archived": false}]"#,
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let props = client.properties().list().await.unwrap();

    assert_eq!(props.len(), 1);
    assert_eq!(props[0].id, 612_634);
    assert_eq!(props[0].name, "Priority");
    assert_eq!(props[0].property_type, "select");
}

#[tokio::test]
async fn select_values_hits_property_scoped_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/company/custom-properties/612634/select-values"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"[
                {"id": 18929916, "value": "High", "color": 4, "sort_order": 1},
                {"id": 18929917, "value": "Low", "sort_order": 2}
            ]"#,
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let values = client.properties().select_values(612_634).await.unwrap();

    assert_eq!(values.len(), 2);
    assert_eq!(values[0].value, "High");
    assert_eq!(values[1].id, 18_929_917);
}

#[tokio::test]
async fn update_card_sends_properties_object_verbatim() {
    let server = MockServer::start().await;
    // select values go as an ARRAY of option ids (live-verified format)
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(body_json(serde_json::json!({
            "properties": { "id_612634": [18_929_916] }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"id": 67089469, "title": "x", "properties": {"id_612634": [18929916]}}"#,
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let req = UpdateCard {
        properties: Some(serde_json::json!({ "id_612634": [18_929_916] })),
        ..Default::default()
    };
    let card = client.cards().update(67_089_469, &req).await.unwrap();
    assert_eq!(card.properties.unwrap()["id_612634"][0], 18_929_916);
}
