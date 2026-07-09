use std::sync::Arc;

use kaiten_client::{CardFilter, KaitenClient, KaitenError};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};

use crate::error::CliError;

#[derive(Clone)]
pub struct KaitenMcp {
    client: Arc<KaitenClient>,
    tool_router: ToolRouter<Self>,
}

fn to_mcp_error(err: KaitenError) -> McpError {
    // RateLimited already renders as "rate limited, retry after Ns".
    McpError::internal_error(err.to_string(), None)
}

fn json_result<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
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

#[derive(Debug, Default, serde::Deserialize, schemars::JsonSchema)]
pub struct ListCardsParams {
    /// Filter by space id
    pub space_id: Option<u64>,
    /// Filter by board id
    pub board_id: Option<u64>,
    /// Filter by column id
    pub column_id: Option<u64>,
    /// Full-text search query
    pub query: Option<String>,
    /// Filter by member user id
    pub member_id: Option<u64>,
    /// Filter by tag name
    pub tag: Option<String>,
    /// Filter by card type id
    pub type_id: Option<u64>,
    /// Include archived cards
    pub archived: Option<bool>,
    /// Max number of cards to return (default 50)
    pub limit: Option<u32>,
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

#[tool_router]
impl KaitenMcp {
    pub fn new(client: Arc<KaitenClient>) -> Self {
        Self {
            client,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get the current authenticated Kaiten user (id, name, email).")]
    async fn current_user(&self) -> Result<CallToolResult, McpError> {
        let user = self.client.users().current().await.map_err(to_mcp_error)?;
        json_result(&user)
    }

    #[tool(description = "List all Kaiten spaces visible to the current user.")]
    async fn list_spaces(&self) -> Result<CallToolResult, McpError> {
        let spaces = self.client.spaces().list().await.map_err(to_mcp_error)?;
        json_result(&spaces)
    }

    #[tool(description = "List boards in a space.")]
    async fn list_boards(
        &self,
        Parameters(p): Parameters<ListBoardsParams>,
    ) -> Result<CallToolResult, McpError> {
        let boards = self
            .client
            .boards()
            .list(p.space_id)
            .await
            .map_err(to_mcp_error)?;
        json_result(&boards)
    }

    #[tool(
        description = "Get a board with its columns and lanes. Use it to discover column/lane ids before creating or moving cards."
    )]
    async fn get_board(
        &self,
        Parameters(p): Parameters<GetBoardParams>,
    ) -> Result<CallToolResult, McpError> {
        let board = self
            .client
            .boards()
            .get(p.board_id)
            .await
            .map_err(to_mcp_error)?;
        json_result(&board)
    }

    #[tool(
        description = "Search and list cards with optional filters. Returned cards have no description/members/checklists; call get_card for full details."
    )]
    async fn list_cards(
        &self,
        Parameters(p): Parameters<ListCardsParams>,
    ) -> Result<CallToolResult, McpError> {
        let filter = CardFilter {
            space_id: p.space_id,
            board_id: p.board_id,
            column_id: p.column_id,
            query: p.query,
            member_ids: p.member_id.into_iter().collect(),
            tag: p.tag,
            type_id: p.type_id,
            archived: p.archived,
            limit: Some(p.limit.unwrap_or(50)),
            ..Default::default()
        };
        let cards = self.client.cards().list(&filter).await.map_err(to_mcp_error)?;
        json_result(&cards)
    }

    #[tool(
        description = "Get a full card by id: description, members, tags, checklists with items, custom properties."
    )]
    async fn get_card(
        &self,
        Parameters(p): Parameters<GetCardParams>,
    ) -> Result<CallToolResult, McpError> {
        let card = self.client.cards().get(p.card_id).await.map_err(to_mcp_error)?;
        json_result(&card)
    }

    #[tool(description = "List all comments of a card.")]
    async fn list_comments(
        &self,
        Parameters(p): Parameters<ListCommentsParams>,
    ) -> Result<CallToolResult, McpError> {
        let comments = self
            .client
            .comments()
            .list(p.card_id)
            .await
            .map_err(to_mcp_error)?;
        json_result(&comments)
    }

    #[tool(description = "List checklists of a card, including their items.")]
    async fn list_checklists(
        &self,
        Parameters(p): Parameters<ListChecklistsParams>,
    ) -> Result<CallToolResult, McpError> {
        // GET /cards/{id}/checklists does not exist in the Kaiten API (405);
        // checklists come embedded in the full card.
        let card = self.client.cards().get(p.card_id).await.map_err(to_mcp_error)?;
        json_result(&card.checklists)
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
             cards, manage comments and checklists. Start with list_spaces to discover \
             structure, or list_cards with filters to find work items."
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
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{GetCardParams, KaitenMcp};

    const SPACES_FIXTURE: &str = include_str!("../../tests/fixtures/mcp_spaces.json");

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
        assert_eq!(value[0]["id"], 810669);
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
        let err = mcp
            .get_card(Parameters(GetCardParams { card_id: 999 }))
            .await
            .unwrap_err();
        assert!(
            err.message.contains("API error 403"),
            "expected KaitenError text in tool error, got: {}",
            err.message
        );
    }

    #[test]
    fn registers_exactly_8_read_only_tools() {
        let tools = KaitenMcp::tool_router().list_all();
        let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "current_user",
                "get_board",
                "get_card",
                "list_boards",
                "list_cards",
                "list_checklists",
                "list_comments",
                "list_spaces",
            ]
        );
    }
}
