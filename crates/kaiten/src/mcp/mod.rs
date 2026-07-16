mod projections;

use std::sync::Arc;

use kaiten_client::{CardFilter, CreateCard, KaitenClient, KaitenError, UpdateCard};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};

use crate::error::CliError;
use projections::{
    CardDetail, CardSummary, ChecklistItemView, ChecklistView, CommentResult, CommentView,
    MemberView, MutationResult,
};

#[derive(Clone)]
pub struct KaitenMcp {
    client: Arc<KaitenClient>,
    /// Web origin for short card links, e.g. "https://mycompany.kaiten.ru"
    /// (the API base URL without the `/api/latest` path).
    web_base: String,
    tool_router: ToolRouter<Self>,
}

/// Map a Kaiten API/network failure to a tool-level error result.
///
/// Per MCP convention this is `Ok(CallToolResult { is_error: true, .. })`,
/// not a JSON-RPC protocol error (`Err(McpError)`): the request was valid
/// and routed correctly, executing it against the Kaiten API just failed.
/// The client renders `content` to the model, so `err.to_string()` (e.g.
/// "API error 403: Forbidden" or "rate limited, retry after 5s") reaches it
/// verbatim. Parameter-validation/serialization errors from the framework
/// stay protocol errors — this helper is only for API-call failures.
fn error_result(err: &KaitenError) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(err.to_string())])
}

/// Unwrap a `Result<T, KaitenError>` produced by a client call inside a tool
/// method, returning early with `Ok(error_result(e))` on failure (see
/// `error_result` for why this is `Ok`, not `Err`, at the tool boundary).
macro_rules! try_api {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => return Ok(error_result(&e)),
        }
    };
}

/// Compact (non-pretty) serialization: tool output is agent context,
/// indentation would only inflate it.
fn json_result<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let text =
        serde_json::to_string(value).map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListBoardsParams {
    /// Space id to list boards from (see list_spaces)
    pub space_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetBoardParams {
    /// Board id
    pub board_id: u64,
}

/// Card state name accepted by list_cards/poll_updates filters.
#[derive(Debug, Clone, Copy, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CardStateParam {
    Queued,
    InProgress,
    Done,
}

impl CardStateParam {
    fn as_u8(self) -> u8 {
        match self {
            Self::Queued => 1,
            Self::InProgress => 2,
            Self::Done => 3,
        }
    }
}

#[derive(Debug, Default, serde::Deserialize, schemars::JsonSchema)]
pub struct ListCardsParams {
    /// Filter by space id
    pub space_id: Option<u64>,
    /// Filter by board id
    pub board_id: Option<u64>,
    /// Filter by column id
    pub column_id: Option<u64>,
    /// Filter by lane id
    pub lane_id: Option<u64>,
    /// Full-text search query
    pub query: Option<String>,
    /// Filter by member user id
    pub member_id: Option<u64>,
    /// Filter by owner user id
    pub owner_id: Option<u64>,
    /// If true, only cards where the current user is a member
    pub mine: Option<bool>,
    /// Filter by tag name
    pub tag: Option<String>,
    /// Filter by card type id
    pub type_id: Option<u64>,
    /// Include archived cards
    pub archived: Option<bool>,
    /// Filter by card states (queued/in_progress/done)
    pub states: Option<Vec<CardStateParam>>,
    /// Only cards updated at/after this ISO 8601 time (inclusive bound)
    pub updated_after: Option<String>,
    /// Only cards created at/after this ISO 8601 time (inclusive bound)
    pub created_after: Option<String>,
    /// Card field to sort by, e.g. "updated" or "created"
    pub order_by: Option<String>,
    /// Sort direction: "asc" or "desc"
    pub order_direction: Option<String>,
    /// Max number of cards to return (default 50; the server caps at 100)
    pub limit: Option<u32>,
    /// Number of cards to skip — pagination beyond the 100-card server cap
    pub offset: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetCardParams {
    /// Card id
    pub card_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListCommentsParams {
    /// Card id
    pub card_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListChecklistsParams {
    /// Card id
    pub card_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateCardParams {
    /// Board id to create the card on
    pub board_id: u64,
    /// Card title
    pub title: String,
    /// Column id (defaults to the first column of the board)
    pub column_id: Option<u64>,
    /// Lane id (defaults to the first lane of the board)
    pub lane_id: Option<u64>,
    /// Card description (markdown)
    pub description: Option<String>,
    /// Card type id (see the board's default_card_type_id)
    pub type_id: Option<u64>,
    /// Mark the card as ASAP
    pub asap: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateCardParams {
    /// Card id
    pub card_id: u64,
    /// New title
    pub title: Option<String>,
    /// New description (markdown)
    pub description: Option<String>,
    /// New card type id
    pub type_id: Option<u64>,
    /// Set or clear the ASAP flag
    pub asap: Option<bool>,
}

// The `_id` postfix on every field is the public MCP tool-parameter contract
// (schemars-derived JSON schema seen by callers), not incidental naming; the
// lint's fix would rename a documented external interface, not just style.
#[allow(clippy::struct_field_names)]
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct MoveCardParams {
    /// Card id
    pub card_id: u64,
    /// Target column id (see get_board)
    pub column_id: u64,
    /// Target lane id
    pub lane_id: Option<u64>,
    /// Target board id (for cross-board moves)
    pub board_id: Option<u64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CardMemberParams {
    /// Card id
    pub card_id: u64,
    /// User id (see current_user or list_cards owners)
    pub user_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddCommentParams {
    /// Card id
    pub card_id: u64,
    /// Comment text (markdown)
    pub text: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddChecklistItemParams {
    /// Card id
    pub card_id: u64,
    /// Checklist id (see list_checklists)
    pub checklist_id: u64,
    /// Item text
    pub text: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetChecklistItemCheckedParams {
    /// Card id
    pub card_id: u64,
    /// Checklist id (see list_checklists)
    pub checklist_id: u64,
    /// Checklist item id
    pub item_id: u64,
    /// true to check, false to uncheck
    pub checked: bool,
}

#[tool_router]
impl KaitenMcp {
    pub fn new(client: Arc<KaitenClient>) -> Self {
        let web_base = client.base_url().origin().ascii_serialization();
        Self {
            client,
            web_base,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get the current authenticated Kaiten user (id, name, email).")]
    async fn current_user(&self) -> Result<CallToolResult, McpError> {
        let user = try_api!(self.client.users().current().await);
        json_result(&user)
    }

    #[tool(description = "List all Kaiten spaces visible to the current user.")]
    async fn list_spaces(&self) -> Result<CallToolResult, McpError> {
        let spaces = try_api!(self.client.spaces().list().await);
        json_result(&spaces)
    }

    #[tool(description = "List boards in a space.")]
    async fn list_boards(
        &self,
        Parameters(p): Parameters<ListBoardsParams>,
    ) -> Result<CallToolResult, McpError> {
        let boards = try_api!(self.client.boards().list(p.space_id).await);
        json_result(&boards)
    }

    #[tool(
        description = "Get a board with its columns and lanes. Use it to discover column/lane ids before creating or moving cards."
    )]
    async fn get_board(
        &self,
        Parameters(p): Parameters<GetBoardParams>,
    ) -> Result<CallToolResult, McpError> {
        let board = try_api!(self.client.boards().get(p.board_id).await);
        json_result(&board)
    }

    #[tool(
        description = "Search and list cards with optional filters. Returns compact summaries (id, title, column, state, counts); call get_card for full details."
    )]
    async fn list_cards(
        &self,
        Parameters(p): Parameters<ListCardsParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut member_ids: Vec<u64> = p.member_id.into_iter().collect();
        if p.mine == Some(true) {
            let me = try_api!(self.client.users().current().await);
            if !member_ids.contains(&me.id) {
                member_ids.push(me.id);
            }
        }
        let filter = CardFilter {
            space_id: p.space_id,
            board_id: p.board_id,
            column_id: p.column_id,
            lane_id: p.lane_id,
            query: p.query,
            member_ids,
            owner_id: p.owner_id,
            tag: p.tag,
            type_id: p.type_id,
            archived: Some(p.archived.unwrap_or(false)),
            states: p
                .states
                .unwrap_or_default()
                .into_iter()
                .map(CardStateParam::as_u8)
                .collect(),
            updated_after: p.updated_after,
            created_after: p.created_after,
            order_by: p.order_by,
            order_direction: p.order_direction,
            limit: Some(p.limit.unwrap_or(50)),
            offset: p.offset,
            ..Default::default()
        };
        let cards = try_api!(self.client.cards().list(&filter).await);
        let summaries: Vec<CardSummary> = cards.iter().map(CardSummary::from).collect();
        json_result(&summaries)
    }

    #[tool(
        description = "Get a full card by id: description, members, tags, checklists, custom properties, linked cards (children/parents), blockers and attached files. For the raw API JSON use the CLI (kaiten card view --json)."
    )]
    async fn get_card(
        &self,
        Parameters(p): Parameters<GetCardParams>,
    ) -> Result<CallToolResult, McpError> {
        let card = try_api!(self.client.cards().get(p.card_id).await);
        json_result(&CardDetail::from(&card))
    }

    #[tool(description = "List all comments of a card.")]
    async fn list_comments(
        &self,
        Parameters(p): Parameters<ListCommentsParams>,
    ) -> Result<CallToolResult, McpError> {
        let comments = try_api!(self.client.comments().list(p.card_id).await);
        let views: Vec<CommentView> = comments.iter().map(CommentView::from).collect();
        json_result(&views)
    }

    #[tool(description = "List checklists of a card, including their items.")]
    async fn list_checklists(
        &self,
        Parameters(p): Parameters<ListChecklistsParams>,
    ) -> Result<CallToolResult, McpError> {
        // GET /cards/{id}/checklists does not exist in the Kaiten API (405);
        // checklists come embedded in the full card.
        let card = try_api!(self.client.cards().get(p.card_id).await);
        let views: Vec<ChecklistView> = card.checklists.iter().map(ChecklistView::from).collect();
        json_result(&views)
    }

    #[tool(
        description = "Create a new card on a board. Returns {id, url, title, column}; call get_card for full details."
    )]
    async fn create_card(
        &self,
        Parameters(p): Parameters<CreateCardParams>,
    ) -> Result<CallToolResult, McpError> {
        let req = CreateCard {
            board_id: p.board_id,
            title: p.title,
            column_id: p.column_id,
            lane_id: p.lane_id,
            description: p.description,
            type_id: p.type_id,
            asap: p.asap,
        };
        let card = try_api!(self.client.cards().create(&req).await);
        json_result(&MutationResult::new(&card, &self.web_base))
    }

    #[tool(
        description = "Update card title, description, type or ASAP flag. Only provided fields are changed."
    )]
    async fn update_card(
        &self,
        Parameters(p): Parameters<UpdateCardParams>,
    ) -> Result<CallToolResult, McpError> {
        let req = UpdateCard {
            title: p.title,
            description: p.description,
            type_id: p.type_id,
            asap: p.asap,
            ..Default::default()
        };
        let card = try_api!(self.client.cards().update(p.card_id, &req).await);
        json_result(&MutationResult::new(&card, &self.web_base))
    }

    #[tool(
        description = "Move a card to another column (and optionally lane or board). Use get_board to discover column and lane ids."
    )]
    async fn move_card(
        &self,
        Parameters(p): Parameters<MoveCardParams>,
    ) -> Result<CallToolResult, McpError> {
        let req = UpdateCard {
            column_id: Some(p.column_id),
            lane_id: p.lane_id,
            board_id: p.board_id,
            ..Default::default()
        };
        let card = try_api!(self.client.cards().update(p.card_id, &req).await);
        json_result(&MutationResult::new(&card, &self.web_base))
    }

    #[tool(description = "Add a user to the card members by user id.")]
    async fn add_card_member(
        &self,
        Parameters(p): Parameters<CardMemberParams>,
    ) -> Result<CallToolResult, McpError> {
        let member = try_api!(self.client.members().add(p.card_id, p.user_id).await);
        json_result(&MemberView::from(&member))
    }

    #[tool(description = "Remove a user from the card members by user id.")]
    async fn remove_card_member(
        &self,
        Parameters(p): Parameters<CardMemberParams>,
    ) -> Result<CallToolResult, McpError> {
        try_api!(self.client.members().remove(p.card_id, p.user_id).await);
        json_result(&serde_json::json!({ "removed": true, "user_id": p.user_id }))
    }

    #[tool(
        description = "Add a comment to a card. Returns {id, created} — the text is not echoed back."
    )]
    async fn add_comment(
        &self,
        Parameters(p): Parameters<AddCommentParams>,
    ) -> Result<CallToolResult, McpError> {
        let comment = try_api!(self.client.comments().add(p.card_id, &p.text).await);
        json_result(&CommentResult::from(&comment))
    }

    #[tool(description = "Add an item to an existing checklist on a card.")]
    async fn add_checklist_item(
        &self,
        Parameters(p): Parameters<AddChecklistItemParams>,
    ) -> Result<CallToolResult, McpError> {
        let item = try_api!(
            self.client
                .checklists()
                .add_item(p.card_id, p.checklist_id, &p.text)
                .await
        );
        json_result(&ChecklistItemView::from(&item))
    }

    #[tool(description = "Check or uncheck a checklist item on a card.")]
    async fn set_checklist_item_checked(
        &self,
        Parameters(p): Parameters<SetChecklistItemCheckedParams>,
    ) -> Result<CallToolResult, McpError> {
        let item = try_api!(
            self.client
                .checklists()
                .set_item_checked(p.card_id, p.checklist_id, p.item_id, p.checked)
                .await
        );
        json_result(&ChecklistItemView::from(&item))
    }
}

// rmcp 2.2.0's `#[tool_handler]` default router expression is `Self::tool_router()`,
// which would rebuild a fresh router per request and leave the `tool_router` instance
// field dead; route through the field explicitly so it's the single source of truth.
#[tool_handler(router = self.tool_router.clone())]
impl ServerHandler for KaitenMcp {
    fn get_info(&self) -> ServerInfo {
        // `ServerInfo` (= `InitializeResult`) is `#[non_exhaustive]`, so it can't be
        // built with struct-literal `..Default::default()` update syntax from this
        // crate; start from `Default::default()` and mutate fields instead.
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "Kaiten tracker tools: browse spaces, boards and cards, create and edit \
             cards, manage members, comments and checklists. Start with list_spaces \
             to discover structure, or list_cards with mine=true to see the current \
             user's cards."
                .into(),
        );
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
}

pub async fn serve(client: KaitenClient) -> Result<(), CliError> {
    use rmcp::ServiceExt;

    let server = KaitenMcp::new(Arc::new(client));
    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| CliError::Io(std::io::Error::other(e)))?;
    service
        .waiting()
        .await
        .map_err(|e| CliError::Io(std::io::Error::other(e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kaiten_client::KaitenClient;
    use rmcp::handler::server::wrapper::Parameters;
    use rmcp::model::CallToolResult;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{CreateCardParams, GetCardParams, KaitenMcp, ListCardsParams};

    const SPACES_FIXTURE: &str = include_str!("../../tests/fixtures/mcp_spaces.json");
    const CARD_CREATE_FIXTURE: &str = include_str!("../../tests/fixtures/mcp_card_create.json");
    const USER_CURRENT_FIXTURE: &str = include_str!("../../tests/fixtures/mcp_user_current.json");
    const CARD_FULL_FIXTURE: &str = include_str!("../../tests/fixtures/mcp_card_full.json");

    fn mcp_for(server: &MockServer) -> KaitenMcp {
        let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
        KaitenMcp::new(Arc::new(client))
    }

    fn tool_text(result: &CallToolResult) -> String {
        result.content[0]
            .as_text()
            .expect("tool result must be text content")
            .text
            .clone()
    }

    #[tokio::test]
    async fn list_spaces_returns_spaces_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/spaces"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SPACES_FIXTURE))
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp.list_spaces().await.unwrap();
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(value[0]["id"], 810_669);
        assert_eq!(value[0]["title"], "Первое пространство");
    }

    #[tokio::test]
    async fn get_card_403_maps_to_tool_error_with_api_text() {
        let server = MockServer::start().await;
        // Kaiten returns 403 with an EMPTY body for foreign/missing cards.
        Mock::given(method("GET"))
            .and(path("/cards/999"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .get_card(Parameters(GetCardParams { card_id: 999 }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let text = tool_text(&result);
        assert!(
            text.contains("API error 403"),
            "expected KaitenError text in tool error content, got: {text}"
        );
    }

    #[tokio::test]
    async fn get_card_exhausted_429_maps_to_tool_error_with_rate_limit_text() {
        let server = MockServer::start().await;
        // Reset=0 keeps the retry_wait_secs sleeps at 0s so this test stays fast.
        // 1 initial request + 3 retries = 4 requests before giving up.
        Mock::given(method("GET"))
            .and(path("/cards/999"))
            .respond_with(ResponseTemplate::new(429).insert_header("X-RateLimit-Reset", "0"))
            .expect(4)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .get_card(Parameters(GetCardParams { card_id: 999 }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let text = tool_text(&result);
        assert!(
            text.contains("rate limited, retry after"),
            "expected RateLimited text in tool error content, got: {text}"
        );
    }

    #[tokio::test]
    async fn create_card_sends_exact_body() {
        let server = MockServer::start().await;
        // None-fields must be skipped: the body is exactly board_id + title.
        Mock::given(method("POST"))
            .and(path("/cards"))
            .and(header("Authorization", "Bearer test-token"))
            .and(body_json(serde_json::json!({
                "board_id": 1_826_109,
                "title": "from mcp"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(CARD_CREATE_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .create_card(Parameters(CreateCardParams {
                board_id: 1_826_109,
                title: "from mcp".to_string(),
                column_id: None,
                lane_id: None,
                description: None,
                type_id: None,
                asap: None,
            }))
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(value["id"], 67_089_469);
        assert_eq!(value["board_id"], 1_826_109);
    }

    #[tokio::test]
    async fn list_cards_mine_resolves_current_user_to_member_filter() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/current"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(USER_CURRENT_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("member_ids", "1068514"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .list_cards(Parameters(ListCardsParams {
                mine: Some(true),
                ..Default::default()
            }))
            .await
            .unwrap();
        assert_eq!(tool_text(&result).trim(), "[]");
    }

    #[tokio::test]
    async fn list_cards_passes_states_dates_sort_and_offset() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("states", "1,2"))
            .and(query_param("updated_after", "2026-07-01T00:00:00Z"))
            .and(query_param("order_by", "updated"))
            .and(query_param("order_direction", "asc"))
            .and(query_param("offset", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .list_cards(Parameters(ListCardsParams {
                states: Some(vec![
                    super::CardStateParam::Queued,
                    super::CardStateParam::InProgress,
                ]),
                updated_after: Some("2026-07-01T00:00:00Z".to_string()),
                order_by: Some("updated".to_string()),
                order_direction: Some("asc".to_string()),
                offset: Some(100),
                ..Default::default()
            }))
            .await
            .unwrap();
        assert_eq!(tool_text(&result).trim(), "[]");
    }

    #[tokio::test]
    async fn list_cards_default_excludes_archived() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("archived", "false"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .list_cards(Parameters(ListCardsParams {
                ..Default::default()
            }))
            .await
            .unwrap();
        assert_eq!(tool_text(&result).trim(), "[]");
    }

    #[tokio::test]
    async fn get_card_returns_compact_detail_with_links_blockers_files() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cards/67089469"))
            .respond_with(ResponseTemplate::new(200).set_body_string(CARD_FULL_FIXTURE))
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .get_card(Parameters(GetCardParams {
                card_id: 67_089_469,
            }))
            .await
            .unwrap();
        let text = tool_text(&result);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();

        // links, blockers and files are projected compactly
        assert_eq!(value["children"][0]["id"], 67_089_310);
        assert_eq!(value["children"][0]["title"], "child card");
        assert_eq!(value["blockers"][0]["reason"], "waiting for child card");
        assert_eq!(value["blockers"][0]["blocker_card_id"], 67_089_310);
        assert_eq!(value["files"][0]["name"], "probe-attach.txt");
        assert_eq!(value["members"][0]["name"], "dxmuser");
        assert_eq!(value["tags"][0], "cli-test");
        // nested objects are flattened to names; raw keys must be gone
        let obj = value.as_object().unwrap();
        for absent in ["board", "lane", "owner_id", "uid", "parents", "properties"] {
            assert!(!obj.contains_key(absent), "unexpected key {absent}: {text}");
        }
        // compact serialization: no pretty-print indentation
        assert!(!text.contains("\n  "), "output must not be pretty-printed");
    }

    #[tokio::test]
    async fn create_card_returns_mutation_result_with_web_url() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cards"))
            .respond_with(ResponseTemplate::new(200).set_body_string(CARD_CREATE_FIXTURE))
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .create_card(Parameters(CreateCardParams {
                board_id: 1_826_109,
                title: "from mcp".to_string(),
                column_id: None,
                lane_id: None,
                description: None,
                type_id: None,
                asap: None,
            }))
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(value["url"], format!("{}/67089469", server.uri()));
        // the mutation result must NOT echo the description back
        assert!(value.get("description").is_none());
    }

    #[test]
    fn registers_exactly_16_tools_with_spec_names() {
        let tools = KaitenMcp::tool_router().list_all();
        let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
        names.sort();
        let mut expected = vec![
            "current_user",
            "list_spaces",
            "list_boards",
            "get_board",
            "list_cards",
            "get_card",
            "create_card",
            "update_card",
            "move_card",
            "add_card_member",
            "remove_card_member",
            "list_comments",
            "add_comment",
            "list_checklists",
            "add_checklist_item",
            "set_checklist_item_checked",
        ];
        expected.sort_unstable();
        assert_eq!(names, expected);
    }
}
