mod projections;

use std::path::Path;
use std::sync::Arc;

use kaiten_client::{CardFilter, CreateCard, FileRef, KaitenClient, KaitenError, UpdateCard};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};

use crate::download;
use crate::error::CliError;
use crate::properties;
use crate::urls;
use projections::{
    CardDetail, CardSummary, ChecklistItemView, ChecklistView, CommentResult, CommentView,
    ExternalLinkView, MemberView, MutationResult, TimeLogView, UserView,
};

#[derive(Clone)]
pub struct KaitenMcp {
    client: Arc<KaitenClient>,
    /// Web origin for short card links, e.g. "https://mycompany.kaiten.ru"
    /// (the API base URL without the `/api/latest` path).
    web_base: String,
    /// Where `download_file` saves when no `save_path` is given: a per-user
    /// cache directory, `<card_id>/<file ref>/<name>` underneath.
    download_dir: std::path::PathBuf,
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
/// Validation this crate does itself (the `properties` shape, see
/// `coerce_properties`) uses the same tool-level shape as the framework's
/// deserialization errors: `isError: true`, raised before any API call.
fn error_result(err: &KaitenError) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(err.to_string())])
}

/// Validate the optional `properties` argument of create_card/update_card
/// (issue #15): a JSON string holding an object is unwrapped, anything that
/// is not an object is a tool-level error — the API used to ignore it and
/// report success. The message is user-facing, see `try_args!`.
fn coerce_properties(
    value: Option<serde_json::Value>,
) -> Result<Option<serde_json::Value>, String> {
    value
        .map(properties::coerce_object)
        .transpose()
        .map_err(|msg| format!("properties {msg}"))
}

/// `try_api!` for this crate's own argument validation: the `Err` is already
/// a user-facing message, returned as a tool-level error before any API call.
macro_rules! try_args {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(msg) => return Ok(CallToolResult::error(vec![ContentBlock::text(msg)])),
        }
    };
}

/// `url` of an external link must be an absolute http(s) URL; the value is
/// never echoed (it may hold a secret).
fn link_url(raw: &str) -> Result<String, String> {
    urls::absolute_http_url(raw).map_err(|e| format!("url {e}"))
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
#[serde(deny_unknown_fields)]
pub struct ListBoardsParams {
    /// Space id to list boards from (see list_spaces)
    pub space_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct GetCardParams {
    /// Card id
    pub card_id: u64,
    /// Extra sections: "comments" (one more request). External links come
    /// with the card itself; "external_links" is deprecated and does nothing.
    pub include: Option<Vec<IncludeSection>>,
}

/// Extra sections of `get_card`. The names match the CLI `card view --include`
/// values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IncludeSection {
    ExternalLinks,
    Comments,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListCommentsParams {
    /// Card id
    pub card_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListChecklistsParams {
    /// Card id
    pub card_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
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
    /// Custom property values as a JSON OBJECT keyed as "id_{property_id}"
    /// (see list_custom_properties). Formats: select/multi-select = ARRAY of
    /// option ids from list_property_select_values, e.g. {"id_612634":
    /// [18929916]}; string/number/url = plain value; date = {"date":
    /// "2026-07-16", "time": "19:00", "tzOffset": 180}; a property value of
    /// null clears that property
    #[schemars(with = "Option<serde_json::Map<String, serde_json::Value>>")]
    pub properties: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
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
    /// Custom property values as a JSON OBJECT keyed as "id_{property_id}"
    /// (see list_custom_properties). Formats: select/multi-select = ARRAY of
    /// option ids from list_property_select_values, e.g. {"id_612634":
    /// [18929916]}; string/number/url = plain value; date = {"date":
    /// "2026-07-16", "time": "19:00", "tzOffset": 180}; a property value of
    /// null clears that property
    #[schemars(with = "Option<serde_json::Map<String, serde_json::Value>>")]
    pub properties: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListPropertySelectValuesParams {
    /// Custom property id (see list_custom_properties)
    pub property_id: u64,
}

// The `_id` postfix on every field is the public MCP tool-parameter contract
// (schemars-derived JSON schema seen by callers), not incidental naming; the
// lint's fix would rename a documented external interface, not just style.
#[allow(clippy::struct_field_names)]
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct CardMemberParams {
    /// Card id
    pub card_id: u64,
    /// User id (see current_user or list_cards owners)
    pub user_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddCommentParams {
    /// Card id
    pub card_id: u64,
    /// Comment text (markdown)
    pub text: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddChecklistItemParams {
    /// Card id
    pub card_id: u64,
    /// Checklist id (see list_checklists)
    pub checklist_id: u64,
    /// Item text
    pub text: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PollUpdatesParams {
    /// Cursor: pass next_since from the previous poll_updates response verbatim.
    /// ISO 8601; the bound is INCLUSIVE, so a card updated exactly at `since`
    /// is returned again — deduplicate by (id, updated).
    pub since: String,
    /// Limit scope to a space
    pub space_id: Option<u64>,
    /// Limit scope to a board
    pub board_id: Option<u64>,
    /// Only cards where the current user is a member; adds mine_card_ids
    /// to the response (diff it against your previous poll to detect
    /// added/removed membership — member changes do not bump `updated`)
    pub mine: Option<bool>,
    /// Like mine, but for an explicit user id
    pub member_id: Option<u64>,
    /// Also detect cards with new comments (default true). Comments do not
    /// bump `updated`, so they are tracked separately
    pub track_comments: Option<bool>,
    /// Max cards per section (default 50; the server caps at 100)
    pub limit: Option<u32>,
}

/// A card that received new comments since the cursor.
#[derive(Debug, serde::Serialize)]
struct CommentedCard {
    id: u64,
    title: String,
    comment_last_added_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    comments_total: Option<u32>,
}

#[derive(Debug, serde::Serialize)]
struct PollUpdatesResponse {
    since: String,
    /// Cursor for the next call.
    next_since: String,
    /// True when updated_cards hit the limit — poll again with next_since
    /// to fetch the rest before acting.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    has_more: bool,
    /// Cards whose fields changed or that were moved (field edits and moves
    /// bump `updated`; comments and membership changes do not).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    updated_cards: Vec<CardSummary>,
    /// Cards with comments added since the cursor.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    commented_cards: Vec<CommentedCard>,
    /// Full id list of cards where the tracked user is a member
    /// (present only with mine/member_id).
    #[serde(skip_serializing_if = "Option::is_none")]
    mine_card_ids: Option<Vec<u64>>,
}

/// Direction of a card link relative to `card_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// target_id becomes a CHILD of card_id
    Child,
    /// target_id becomes a PARENT of card_id
    Parent,
    /// card_id BLOCKS target_id
    Blocks,
    /// card_id is BLOCKED BY target_id
    BlockedBy,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkCardsParams {
    /// Card id the link is described from
    pub card_id: u64,
    /// The other card
    pub target_id: u64,
    /// child: target becomes a child of card; parent: target becomes a
    /// parent of card; blocks: card blocks target; blocked_by: card is
    /// blocked by target
    pub kind: LinkKind,
    /// Optional block reason (blocks/blocked_by only)
    pub reason: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnlinkCardsParams {
    /// Card id the link is described from
    pub card_id: u64,
    /// The other card
    pub target_id: u64,
    /// Same semantics as in link_cards
    pub kind: LinkKind,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseBlocksParams {
    /// Card id
    pub card_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateCommentParams {
    /// Card id
    pub card_id: u64,
    /// Comment id (see list_comments)
    pub comment_id: u64,
    /// New comment text (markdown)
    pub text: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveCommentParams {
    /// Card id
    pub card_id: u64,
    /// Comment id (see list_comments)
    pub comment_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListCardExternalLinksParams {
    /// Card id
    pub card_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddCardExternalLinkParams {
    /// Card id
    pub card_id: u64,
    /// Absolute http(s) URL
    pub url: String,
    /// Optional description shown next to the link
    pub description: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateCardExternalLinkParams {
    /// Card id
    pub card_id: u64,
    /// External link id (see list_card_external_links)
    pub link_id: u64,
    /// New absolute http(s) URL
    pub url: Option<String>,
    /// New description
    pub description: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveCardExternalLinkParams {
    /// Card id
    pub card_id: u64,
    /// External link id (see list_card_external_links)
    pub link_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetCardResponsibleParams {
    /// Card id
    pub card_id: u64,
    /// User id (must already be a card member — use add_card_member first)
    pub user_id: u64,
    /// false demotes the user back to a regular member
    pub responsible: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddTimeLogParams {
    /// Card id
    pub card_id: u64,
    /// Time spent in MINUTES
    pub time_spent: i64,
    /// Date the work was done, YYYY-MM-DD (defaults to today per Kaiten)
    pub for_date: String,
    /// Optional comment
    pub comment: Option<String>,
    /// User role id; omitted = the company's first role is used
    /// (built-in roles are negative, e.g. -1 = Employee)
    pub role_id: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListTimeLogsParams {
    /// Card id
    pub card_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachFileParams {
    /// Card id
    pub card_id: u64,
    /// Absolute path of a LOCAL file to upload
    pub file_path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DetachFileParams {
    /// Card id
    pub card_id: u64,
    /// File id (see get_card files)
    pub file_id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DownloadFileParams {
    /// Card id
    pub card_id: u64,
    /// File id (numeric) or uid (UUID), as listed by get_card files
    pub file_id: String,
    /// Target file path, or an existing directory (the original name inside
    /// it). Default: <user cache dir>/kaiten/files/<card_id>/<file ref>/<name>
    pub save_path: Option<String>,
    /// Overwrite an existing file at save_path (default false: the call fails
    /// instead); the default location is always refreshed
    pub overwrite: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArchiveCardParams {
    /// Card id
    pub card_id: u64,
    /// true to RESTORE the card from the archive instead
    pub unarchive: Option<bool>,
}

#[derive(Debug, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListUsersParams {
    /// Case-insensitive substring matched against username, full name and email
    pub query: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CardTagParams {
    /// Card id
    pub card_id: u64,
    /// Tag name
    pub name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddChecklistParams {
    /// Card id
    pub card_id: u64,
    /// Checklist name
    pub name: String,
}

#[tool_router]
impl KaitenMcp {
    pub fn new(client: Arc<KaitenClient>) -> Self {
        let web_base = client.base_url().origin().ascii_serialization();
        Self {
            client,
            web_base,
            download_dir: dirs::cache_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("kaiten")
                .join("files"),
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
        let mut filter = CardFilter::default();
        filter.space_id = p.space_id;
        filter.board_id = p.board_id;
        filter.column_id = p.column_id;
        filter.lane_id = p.lane_id;
        filter.query = p.query;
        filter.member_ids = member_ids;
        filter.owner_id = p.owner_id;
        filter.tag = p.tag;
        filter.type_id = p.type_id;
        filter.archived = Some(p.archived.unwrap_or(false));
        filter.states = p
            .states
            .unwrap_or_default()
            .into_iter()
            .map(CardStateParam::as_u8)
            .collect();
        filter.updated_after = p.updated_after;
        filter.created_after = p.created_after;
        filter.order_by = p.order_by;
        filter.order_direction = p.order_direction;
        filter.limit = Some(p.limit.unwrap_or(50));
        filter.offset = p.offset;
        let cards = try_api!(self.client.cards().list(&filter).await);
        let summaries: Vec<CardSummary> = cards.iter().map(CardSummary::from).collect();
        json_result(&summaries)
    }

    #[tool(
        description = "Get a full card by id: description, members, tags, checklists, custom properties, linked cards (children/parents), blockers and attached files. For the raw API JSON use the CLI (kaiten card view --json). External links come with the card; pass include: [\"comments\"] to fetch comments too (one extra request)."
    )]
    async fn get_card(
        &self,
        Parameters(p): Parameters<GetCardParams>,
    ) -> Result<CallToolResult, McpError> {
        let card = try_api!(self.client.cards().get(p.card_id).await);
        let mut detail = CardDetail::from(&card);
        let include = p.include.unwrap_or_default();
        // IncludeSection::ExternalLinks is accepted for compatibility: the
        // links are already part of the card response; requesting them only
        // keeps the key present (as `[]`) when the card has none.
        if include.contains(&IncludeSection::ExternalLinks) {
            detail.external_links.get_or_insert_with(Vec::new);
        }
        if include.contains(&IncludeSection::Comments) {
            let comments = try_api!(self.client.comments().list(p.card_id).await);
            detail.comments = Some(comments.iter().map(CommentView::from).collect());
        }
        json_result(&detail)
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
        let properties = try_args!(coerce_properties(p.properties));
        let mut req = CreateCard::new(p.board_id, p.title);
        req.column_id = p.column_id;
        req.lane_id = p.lane_id;
        req.description = p.description;
        req.type_id = p.type_id;
        req.asap = p.asap;
        req.properties = properties;
        let card = try_api!(self.client.cards().create(&req).await);
        json_result(&MutationResult::new(&card, &self.web_base))
    }

    #[tool(
        description = "Update card title, description, type, ASAP flag or custom properties. Only provided fields are changed. To archive/restore use archive_card; to move use move_card."
    )]
    async fn update_card(
        &self,
        Parameters(p): Parameters<UpdateCardParams>,
    ) -> Result<CallToolResult, McpError> {
        let properties = try_args!(coerce_properties(p.properties));
        let mut req = UpdateCard::default();
        req.title = p.title;
        req.description = p.description;
        req.type_id = p.type_id;
        req.asap = p.asap;
        req.properties = properties;
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
        let mut req = UpdateCard::default();
        req.column_id = Some(p.column_id);
        req.lane_id = p.lane_id;
        req.board_id = p.board_id;
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

    #[tool(
        description = "Archive a card (hide it from the board). Pass unarchive=true to restore it instead."
    )]
    async fn archive_card(
        &self,
        Parameters(p): Parameters<ArchiveCardParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut req = UpdateCard::default();
        req.condition = Some(if p.unarchive == Some(true) { 1 } else { 2 });
        let card = try_api!(self.client.cards().update(p.card_id, &req).await);
        json_result(&MutationResult::new(&card, &self.web_base))
    }

    #[tool(
        description = "List Kaiten users, optionally filtered by a case-insensitive substring of username/full name/email. Use it to find the user id for add_card_member."
    )]
    async fn list_users(
        &self,
        Parameters(p): Parameters<ListUsersParams>,
    ) -> Result<CallToolResult, McpError> {
        let users = try_api!(self.client.users().list().await);
        let needle = p.query.unwrap_or_default().to_lowercase();
        let views: Vec<UserView> = users
            .iter()
            .filter(|u| {
                needle.is_empty()
                    || [&u.username, &u.full_name, &u.email]
                        .into_iter()
                        .flatten()
                        .any(|f| f.to_lowercase().contains(&needle))
            })
            .map(UserView::from)
            .collect();
        json_result(&views)
    }

    #[tool(
        description = "Add a tag to a card by name. The company tag is created automatically when it does not exist yet."
    )]
    async fn add_card_tag(
        &self,
        Parameters(p): Parameters<CardTagParams>,
    ) -> Result<CallToolResult, McpError> {
        let tag = try_api!(self.client.tags().add_to_card(p.card_id, &p.name).await);
        json_result(&tag)
    }

    #[tool(description = "Remove a tag from a card by name.")]
    async fn remove_card_tag(
        &self,
        Parameters(p): Parameters<CardTagParams>,
    ) -> Result<CallToolResult, McpError> {
        let card = try_api!(self.client.cards().get(p.card_id).await);
        let Some(card_tag) = card.tags.iter().find(|t| t.name == p.name) else {
            let existing = if card.tags.is_empty() {
                "(none)".to_string()
            } else {
                card.tags
                    .iter()
                    .map(|t| t.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "card {} has no tag `{}`; existing tags: {existing}",
                p.card_id, p.name
            ))]));
        };
        let tag_id = card_tag.tag_id.unwrap_or(card_tag.id);
        try_api!(self.client.tags().remove_from_card(p.card_id, tag_id).await);
        json_result(&serde_json::json!({ "removed": true, "tag": p.name }))
    }

    #[tool(
        description = "Create a new (empty) checklist on a card; add items with add_checklist_item."
    )]
    async fn add_checklist(
        &self,
        Parameters(p): Parameters<AddChecklistParams>,
    ) -> Result<CallToolResult, McpError> {
        let checklist = try_api!(self.client.checklists().add(p.card_id, &p.name).await);
        json_result(&ChecklistView::from(&checklist))
    }

    #[tool(
        description = "List card types of the company (their ids feed create_card/update_card type_id)."
    )]
    async fn list_card_types(&self) -> Result<CallToolResult, McpError> {
        let types = try_api!(self.client.tags().card_types().await);
        json_result(&types)
    }

    #[tool(
        description = "List company custom properties: id, name, type. Property values are set via create_card/update_card `properties` keyed as id_{property_id}."
    )]
    async fn list_custom_properties(&self) -> Result<CallToolResult, McpError> {
        let props = try_api!(self.client.properties().list().await);
        json_result(&props)
    }

    #[tool(
        description = "List options of a select-type custom property: id and value. Pass ids (as an ARRAY) in the card `properties`, e.g. {\"id_612634\": [18929916]}."
    )]
    async fn list_property_select_values(
        &self,
        Parameters(p): Parameters<ListPropertySelectValuesParams>,
    ) -> Result<CallToolResult, McpError> {
        let values = try_api!(self.client.properties().select_values(p.property_id).await);
        json_result(&values)
    }

    #[tool(
        description = "Link two cards: kind=child makes target a child of card, kind=parent makes target a parent of card, kind=blocks makes card block target, kind=blocked_by blocks card by target (reason is optional for the block kinds). Links are visible in get_card (children/parents/blockers)."
    )]
    async fn link_cards(
        &self,
        Parameters(p): Parameters<LinkCardsParams>,
    ) -> Result<CallToolResult, McpError> {
        match p.kind {
            LinkKind::Child => {
                try_api!(self.client.links().add_child(p.card_id, p.target_id).await);
            }
            LinkKind::Parent => {
                try_api!(self.client.links().add_child(p.target_id, p.card_id).await);
            }
            LinkKind::Blocks => {
                try_api!(
                    self.client
                        .links()
                        .add_blocker(p.target_id, Some(p.card_id), p.reason.as_deref())
                        .await
                );
            }
            LinkKind::BlockedBy => {
                try_api!(
                    self.client
                        .links()
                        .add_blocker(p.card_id, Some(p.target_id), p.reason.as_deref())
                        .await
                );
            }
        }
        json_result(&serde_json::json!({
            "linked": true,
            "card_id": p.card_id,
            "target_id": p.target_id,
            "kind": format!("{:?}", p.kind).to_lowercase(),
        }))
    }

    #[tool(
        description = "Remove a card link created with link_cards (same kind semantics). For block kinds the matching blocker is looked up and removed."
    )]
    async fn unlink_cards(
        &self,
        Parameters(p): Parameters<UnlinkCardsParams>,
    ) -> Result<CallToolResult, McpError> {
        match p.kind {
            LinkKind::Child => {
                try_api!(
                    self.client
                        .links()
                        .remove_child(p.card_id, p.target_id)
                        .await
                );
            }
            LinkKind::Parent => {
                try_api!(
                    self.client
                        .links()
                        .remove_child(p.target_id, p.card_id)
                        .await
                );
            }
            LinkKind::Blocks | LinkKind::BlockedBy => {
                // the blocker entry lives on the BLOCKED card
                let (blocked_id, blocker_card_id) = if p.kind == LinkKind::Blocks {
                    (p.target_id, p.card_id)
                } else {
                    (p.card_id, p.target_id)
                };
                let card = try_api!(self.client.cards().get(blocked_id).await);
                let Some(blocker) = card
                    .blockers
                    .iter()
                    .find(|b| b.blocker_card_id == Some(blocker_card_id))
                else {
                    return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                        "card {blocked_id} has no blocker with card {blocker_card_id}"
                    ))]));
                };
                try_api!(
                    self.client
                        .links()
                        .remove_blocker(blocked_id, blocker.id)
                        .await
                );
            }
        }
        json_result(&serde_json::json!({
            "unlinked": true,
            "card_id": p.card_id,
            "target_id": p.target_id,
        }))
    }

    #[tool(
        description = "Attach a local file to a card (multipart upload). WARNING: Kaiten serves attachments from a public unguessable URL without authentication — never attach secrets. Returns the file id and its public url."
    )]
    async fn attach_file(
        &self,
        Parameters(p): Parameters<AttachFileParams>,
    ) -> Result<CallToolResult, McpError> {
        let file = try_api!(
            self.client
                .files()
                .attach(p.card_id, std::path::Path::new(&p.file_path))
                .await
        );
        json_result(&projections::FileView::from(&file))
    }

    #[tool(
        description = "Download a card attachment to the LOCAL filesystem (binary content cannot be returned over MCP). file_id is the numeric id or the uid from get_card files. Saved to save_path (a file path, or an existing directory) or, when omitted, to <user cache dir>/kaiten/files/<card_id>/<file ref>/<original name> (e.g. ~/.cache/kaiten/files/... on Linux), which is always refreshed; an existing file at an explicit save_path makes the call fail unless overwrite=true. Returns {path (absolute), name, size, mime_type}. SECURITY: classic attachments live on a public unguessable URL without authentication; the API token is sent only to your Kaiten host, never to the file storage."
    )]
    async fn download_file(
        &self,
        Parameters(p): Parameters<DownloadFileParams>,
    ) -> Result<CallToolResult, McpError> {
        let Ok(file_ref) = p.file_id.parse::<FileRef>();
        let files = try_api!(self.client.files().list(p.card_id).await);
        let file =
            try_args!(download::find_file(p.card_id, &files, &file_ref).map_err(|e| e.to_string()));
        let name = download::safe_file_name(file);
        let default_dir = self
            .download_dir
            .join(p.card_id.to_string())
            .join(FileRef::from(file).to_string());
        let target = try_args!(
            download::target_path(p.save_path.as_deref().map(Path::new), &default_dir, &name)
                .map_err(|e| e.to_string())
        );
        if p.save_path.is_some() {
            try_args!(
                download::ensure_writable(&target, p.overwrite.unwrap_or(false), "overwrite=true")
                    .map_err(|e| e.to_string())
            );
        } else {
            try_args!(
                std::fs::create_dir_all(&default_dir)
                    .map_err(|e| format!("cannot create {}: {e}", default_dir.display()))
            );
        }
        let saved = try_args!(
            download::save(&self.client, file, &target)
                .await
                .map_err(|e| e.to_string())
        );
        json_result(&saved)
    }

    #[tool(description = "Detach (remove) a file from a card by file id (see get_card files).")]
    async fn detach_file(
        &self,
        Parameters(p): Parameters<DetachFileParams>,
    ) -> Result<CallToolResult, McpError> {
        try_api!(self.client.files().detach(p.card_id, p.file_id).await);
        json_result(&serde_json::json!({ "detached": true, "file_id": p.file_id }))
    }

    #[tool(description = "Release ALL blocks on a card at once.")]
    async fn release_blocks(
        &self,
        Parameters(p): Parameters<ReleaseBlocksParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut req = UpdateCard::default();
        req.blocked = Some(false);
        let card = try_api!(self.client.cards().update(p.card_id, &req).await);
        json_result(&MutationResult::new(&card, &self.web_base))
    }

    #[tool(description = "Edit an existing comment on a card.")]
    async fn update_comment(
        &self,
        Parameters(p): Parameters<UpdateCommentParams>,
    ) -> Result<CallToolResult, McpError> {
        let comment = try_api!(
            self.client
                .comments()
                .update(p.card_id, p.comment_id, &p.text)
                .await
        );
        json_result(&CommentResult::from(&comment))
    }

    #[tool(description = "Delete a comment from a card.")]
    async fn remove_comment(
        &self,
        Parameters(p): Parameters<RemoveCommentParams>,
    ) -> Result<CallToolResult, McpError> {
        try_api!(self.client.comments().remove(p.card_id, p.comment_id).await);
        json_result(&serde_json::json!({ "removed": true, "comment_id": p.comment_id }))
    }

    #[tool(description = "List the external links (Links (common links)) of a card.")]
    async fn list_card_external_links(
        &self,
        Parameters(p): Parameters<ListCardExternalLinksParams>,
    ) -> Result<CallToolResult, McpError> {
        let links = try_api!(self.client.external_links().list(p.card_id).await);
        let views: Vec<ExternalLinkView> = links.iter().map(ExternalLinkView::from).collect();
        json_result(&views)
    }

    #[tool(
        description = "Add an external link to a card. `url` must be an absolute http(s) URL. Kaiten does not reject duplicates — check list_card_external_links first when that matters."
    )]
    async fn add_card_external_link(
        &self,
        Parameters(p): Parameters<AddCardExternalLinkParams>,
    ) -> Result<CallToolResult, McpError> {
        let url = try_args!(link_url(&p.url));
        let link = try_api!(
            self.client
                .external_links()
                .add(p.card_id, &url, p.description.as_deref())
                .await
        );
        json_result(&ExternalLinkView::from(&link))
    }

    #[tool(
        description = "Change the url and/or description of an external link on a card (at least one of them)."
    )]
    async fn update_card_external_link(
        &self,
        Parameters(p): Parameters<UpdateCardExternalLinkParams>,
    ) -> Result<CallToolResult, McpError> {
        if p.url.is_none() && p.description.is_none() {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                "nothing to change: pass url and/or description",
            )]));
        }
        let url = match p.url.as_deref() {
            Some(raw) => Some(try_args!(link_url(raw))),
            None => None,
        };
        let link = try_api!(
            self.client
                .external_links()
                .update(
                    p.card_id,
                    p.link_id,
                    url.as_deref(),
                    p.description.as_deref()
                )
                .await
        );
        json_result(&ExternalLinkView::from(&link))
    }

    #[tool(description = "Remove an external link from a card.")]
    async fn remove_card_external_link(
        &self,
        Parameters(p): Parameters<RemoveCardExternalLinkParams>,
    ) -> Result<CallToolResult, McpError> {
        try_api!(
            self.client
                .external_links()
                .remove(p.card_id, p.link_id)
                .await
        );
        json_result(&serde_json::json!({ "removed": true, "link_id": p.link_id }))
    }

    #[tool(
        description = "Make a card member the responsible person (or demote with responsible=false). The user must already be a member."
    )]
    async fn set_card_responsible(
        &self,
        Parameters(p): Parameters<SetCardResponsibleParams>,
    ) -> Result<CallToolResult, McpError> {
        let member = try_api!(
            self.client
                .members()
                .update_role(p.card_id, p.user_id, p.responsible.unwrap_or(true))
                .await
        );
        json_result(&MemberView::from(&member))
    }

    #[tool(
        description = "Log time spent on a card (minutes). role_id defaults to the company's first user role."
    )]
    async fn add_time_log(
        &self,
        Parameters(p): Parameters<AddTimeLogParams>,
    ) -> Result<CallToolResult, McpError> {
        let role_id = if let Some(id) = p.role_id {
            id
        } else {
            let roles = try_api!(self.client.users().roles().await);
            match roles.first() {
                Some(role) => role.id,
                None => {
                    return Ok(CallToolResult::error(vec![ContentBlock::text(
                        "no user roles in the company; pass role_id explicitly",
                    )]));
                }
            }
        };
        let log = try_api!(
            self.client
                .time_logs()
                .add(
                    p.card_id,
                    p.time_spent,
                    &p.for_date,
                    role_id,
                    p.comment.as_deref(),
                )
                .await
        );
        json_result(&TimeLogView::from(&log))
    }

    #[tool(description = "List time logs of a card (minutes per entry).")]
    async fn list_time_logs(
        &self,
        Parameters(p): Parameters<ListTimeLogsParams>,
    ) -> Result<CallToolResult, McpError> {
        let logs = try_api!(self.client.time_logs().list(p.card_id).await);
        let views: Vec<TimeLogView> = logs.iter().map(TimeLogView::from).collect();
        json_result(&views)
    }

    #[tool(
        description = "Poll for changes since a cursor: field edits/moves (updated_cards), new comments (commented_cards) and — with mine=true — the full membership id list for diffing. Pass next_since back as `since` on the next call; entries can repeat at the cursor boundary (at-least-once), deduplicate by (id, updated). Start with e.g. the current time minus your poll interval."
    )]
    async fn poll_updates(
        &self,
        Parameters(p): Parameters<PollUpdatesParams>,
    ) -> Result<CallToolResult, McpError> {
        let limit = p.limit.unwrap_or(50).min(100);
        let mut member_ids: Vec<u64> = p.member_id.into_iter().collect();
        if p.mine == Some(true) {
            let me = try_api!(self.client.users().current().await);
            if !member_ids.contains(&me.id) {
                member_ids.push(me.id);
            }
        }
        let mut scope = CardFilter::default();
        scope.space_id = p.space_id;
        scope.board_id = p.board_id;
        scope.member_ids = member_ids.clone();
        scope.limit = Some(limit);

        // A: field edits and moves bump `updated`; ascending order makes the
        // cursor safe when the page overflows (has_more).
        let mut filter_updates = scope.clone();
        filter_updates.updated_after = Some(p.since.clone());
        filter_updates.order_by = Some("updated".to_string());
        filter_updates.order_direction = Some("asc".to_string());
        let updated = try_api!(self.client.cards().list(&filter_updates).await);
        let has_more = updated.len() >= limit as usize;

        // B: comments leave only comment_last_added_at behind (they do NOT
        // bump `updated`) — sort by it and cut client-side. The comparison is
        // lexicographic, valid for the API's uniform UTC ISO 8601 strings.
        let mut commented: Vec<CommentedCard> = Vec::new();
        if p.track_comments != Some(false) {
            let mut filter_comments = scope.clone();
            filter_comments.order_by = Some("comment_last_added_at".to_string());
            filter_comments.order_direction = Some("desc".to_string());
            let cards = try_api!(self.client.cards().list(&filter_comments).await);
            commented = cards
                .iter()
                .filter_map(|c| {
                    let at = c.comment_last_added_at.clone()?;
                    (at.as_str() >= p.since.as_str()).then(|| CommentedCard {
                        id: c.id,
                        title: c.title.clone(),
                        comment_last_added_at: at,
                        comments_total: c.comments_total,
                    })
                })
                .collect();
        }

        // C: membership snapshot — member add/remove bumps nothing on the
        // card, so the agent detects it by diffing this list between polls.
        let mine_card_ids = if member_ids.is_empty() {
            None
        } else {
            let mut filter_mine = scope;
            filter_mine.condition = Some(1);
            filter_mine.limit = Some(100);
            let cards = try_api!(self.client.cards().list(&filter_mine).await);
            Some(cards.iter().map(|c| c.id).collect())
        };

        // Cursor: when page A overflowed, everything after its last row is
        // unseen — the cursor must not jump past it (section B will simply be
        // re-scanned on the next call; at-least-once by design).
        let next_since = if has_more {
            updated
                .last()
                .and_then(|c| c.updated.clone())
                .unwrap_or_else(|| p.since.clone())
        } else {
            let max_updated = updated.iter().filter_map(|c| c.updated.as_deref());
            let max_commented = commented.iter().map(|c| c.comment_last_added_at.as_str());
            max_updated
                .chain(max_commented)
                .chain(std::iter::once(p.since.as_str()))
                .max()
                .unwrap_or(p.since.as_str())
                .to_string()
        };

        json_result(&PollUpdatesResponse {
            since: p.since,
            next_since,
            has_more,
            updated_cards: updated.iter().map(CardSummary::from).collect(),
            commented_cards: commented,
            mine_card_ids,
        })
    }
}

// clippy 1.98+ `unused_async_trait_impl` fires on the `async fn list_tools` that
// `#[tool_handler]` generates (its body has no `.await`). Hand-writing `list_tools`
// just to drop `async` would mean re-implementing the macro's output and tracking
// rmcp's evolving defaults, so the lint is allowed at this one site instead.
#[allow(
    clippy::unused_async_trait_impl,
    reason = "rmcp macro-generated `list_tools`"
)]
// rmcp's `#[tool_handler]` default router expression, `Self::tool_router()`, would
// rebuild a fresh router per request and leave the `tool_router` instance field
// dead; route through the field explicitly so it's the single source of truth.
#[tool_handler(router = self.tool_router.clone())]
impl ServerHandler for KaitenMcp {
    fn get_info(&self) -> ServerInfo {
        // `ServerInfo` (= `InitializeResult`) is `#[non_exhaustive]`, so it can't be
        // built with struct-literal `..Default::default()` update syntax from this
        // crate; start from `Default::default()` and mutate fields instead.
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "Kaiten tracker tools: browse spaces, boards and cards, create and edit \
             cards, manage members, comments, checklists and external links. Start with list_spaces \
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

    use super::{
        AddCardExternalLinkParams, CreateCardParams, GetCardParams, IncludeSection, KaitenMcp,
        ListCardExternalLinksParams, ListCardsParams, RemoveCardExternalLinkParams,
        UpdateCardExternalLinkParams,
    };

    const SPACES_FIXTURE: &str = include_str!("../../tests/fixtures/mcp_spaces.json");
    const CARD_CREATE_FIXTURE: &str = include_str!("../../tests/fixtures/mcp_card_create.json");
    const USER_CURRENT_FIXTURE: &str = include_str!("../../tests/fixtures/mcp_user_current.json");
    const CARD_FULL_FIXTURE: &str = include_str!("../../tests/fixtures/mcp_card_full.json");
    const CARD_WITH_PROPERTIES_FIXTURE: &str =
        include_str!("../../tests/fixtures/card_get_full.json");

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
            .get_card(Parameters(GetCardParams {
                card_id: 999,
                include: None,
            }))
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
            .get_card(Parameters(GetCardParams {
                card_id: 999,
                include: None,
            }))
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
                properties: None,
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
                include: None,
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
                properties: None,
            }))
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(value["url"], format!("{}/67089469", server.uri()));
        // the mutation result must NOT echo the description back
        assert!(value.get("description").is_none());
    }

    #[tokio::test]
    async fn archive_card_sends_condition_2() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/cards/67089469"))
            .and(body_json(serde_json::json!({ "condition": 2 })))
            .respond_with(ResponseTemplate::new(200).set_body_string(CARD_CREATE_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .archive_card(Parameters(super::ArchiveCardParams {
                card_id: 67_089_469,
                unarchive: None,
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn list_users_filters_by_substring_across_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[
                    {"id": 1, "uid": "u1", "username": "dxmuser", "full_name": "DX Muser", "email": "dxm@example.com"},
                    {"id": 2, "uid": "u2", "username": "other", "full_name": "Someone Else", "email": "someone@example.com"}
                ]"#,
            ))
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .list_users(Parameters(super::ListUsersParams {
                query: Some("MUSER".to_string()),
            }))
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 1, "case-insensitive match on one user: {value}");
        assert_eq!(arr[0]["id"], 1);
        // projection drops uid/activated noise
        assert!(arr[0].get("uid").is_none());
    }

    #[tokio::test]
    async fn remove_card_tag_unknown_name_is_tool_error_listing_existing() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cards/67089469"))
            .respond_with(ResponseTemplate::new(200).set_body_string(CARD_FULL_FIXTURE))
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .remove_card_tag(Parameters(super::CardTagParams {
                card_id: 67_089_469,
                name: "nope".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let text = tool_text(&result);
        assert!(
            text.contains("cli-test"),
            "error must list existing tags, got: {text}"
        );
    }

    #[tokio::test]
    async fn poll_updates_empty_keeps_cursor_and_sends_expected_queries() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("updated_after", "2026-07-16T10:00:00.000Z"))
            .and(query_param("order_by", "updated"))
            .and(query_param("order_direction", "asc"))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("order_by", "comment_last_added_at"))
            .and(query_param("order_direction", "desc"))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .poll_updates(Parameters(super::PollUpdatesParams {
                since: "2026-07-16T10:00:00.000Z".to_string(),
                ..Default::default()
            }))
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(value["next_since"], "2026-07-16T10:00:00.000Z");
        assert!(value.get("updated_cards").is_none());
        assert!(value.get("commented_cards").is_none());
        assert!(value.get("mine_card_ids").is_none());
        assert!(value.get("has_more").is_none());
    }

    #[tokio::test]
    async fn poll_updates_full_page_sets_has_more_and_cursor_from_last_row() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("order_by", "updated"))
            .and(query_param("limit", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[
                    {"id": 1, "title": "a", "updated": "2026-07-16T10:05:00.000Z"},
                    {"id": 2, "title": "b", "updated": "2026-07-16T10:10:00.000Z"}
                ]"#,
            ))
            .mount(&server)
            .await;
        // section B sees a LATER comment which must NOT advance the cursor
        // past the unseen tail of section A
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("order_by", "comment_last_added_at"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[{"id": 9, "title": "c", "comment_last_added_at": "2026-07-16T11:00:00.000Z"}]"#,
            ))
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .poll_updates(Parameters(super::PollUpdatesParams {
                since: "2026-07-16T10:00:00.000Z".to_string(),
                limit: Some(2),
                ..Default::default()
            }))
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(value["has_more"], true);
        assert_eq!(value["next_since"], "2026-07-16T10:10:00.000Z");
        assert_eq!(value["updated_cards"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn poll_updates_cuts_comments_before_cursor_and_advances_to_max() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("order_by", "updated"))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("order_by", "comment_last_added_at"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[
                    {"id": 1, "title": "new", "comments_total": 3, "comment_last_added_at": "2026-07-16T10:30:00.000Z"},
                    {"id": 2, "title": "old", "comment_last_added_at": "2026-07-16T09:00:00.000Z"},
                    {"id": 3, "title": "never"}
                ]"#,
            ))
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .poll_updates(Parameters(super::PollUpdatesParams {
                since: "2026-07-16T10:00:00.000Z".to_string(),
                ..Default::default()
            }))
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        let commented = value["commented_cards"].as_array().unwrap();
        assert_eq!(commented.len(), 1, "only the fresh comment: {value}");
        assert_eq!(commented[0]["id"], 1);
        assert_eq!(value["next_since"], "2026-07-16T10:30:00.000Z");
    }

    #[tokio::test]
    async fn poll_updates_mine_resolves_user_and_returns_membership_ids() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/current"))
            .respond_with(ResponseTemplate::new(200).set_body_string(USER_CURRENT_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("member_ids", "1068514"))
            .and(query_param("order_by", "updated"))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("member_ids", "1068514"))
            .and(query_param("order_by", "comment_last_added_at"))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cards"))
            .and(query_param("member_ids", "1068514"))
            .and(query_param("condition", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[{"id": 5, "title": "mine"}, {"id": 6, "title": "also mine"}]"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .poll_updates(Parameters(super::PollUpdatesParams {
                since: "2026-07-16T10:00:00.000Z".to_string(),
                mine: Some(true),
                ..Default::default()
            }))
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(value["mine_card_ids"], serde_json::json!([5, 6]));
    }

    #[tokio::test]
    async fn update_card_passes_properties_object_through() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/cards/67089469"))
            .and(body_json(serde_json::json!({
                "properties": { "id_612634": [18_929_916] }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(CARD_CREATE_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .update_card(Parameters(super::UpdateCardParams {
                card_id: 67_089_469,
                title: None,
                description: None,
                type_id: None,
                asap: None,
                properties: Some(serde_json::json!({ "id_612634": [18_929_916] })),
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn link_cards_child_and_parent_invert_direction() {
        let server = MockServer::start().await;
        // kind=child: target 20 becomes a child of card 10
        Mock::given(method("POST"))
            .and(path("/cards/10/children"))
            .and(body_json(serde_json::json!({ "card_id": 20 })))
            .respond_with(ResponseTemplate::new(200).set_body_string(CARD_CREATE_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;
        // kind=parent: target 20 becomes a parent of card 10 (inverted call)
        Mock::given(method("POST"))
            .and(path("/cards/20/children"))
            .and(body_json(serde_json::json!({ "card_id": 10 })))
            .respond_with(ResponseTemplate::new(200).set_body_string(CARD_CREATE_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        for kind in [super::LinkKind::Child, super::LinkKind::Parent] {
            let result = mcp
                .link_cards(Parameters(super::LinkCardsParams {
                    card_id: 10,
                    target_id: 20,
                    kind,
                    reason: None,
                }))
                .await
                .unwrap();
            assert_ne!(result.is_error, Some(true));
        }
    }

    #[tokio::test]
    async fn link_cards_block_kinds_target_the_blocked_card() {
        let server = MockServer::start().await;
        // kind=blocks: card 10 blocks target 20 -> blocker entry on 20
        Mock::given(method("POST"))
            .and(path("/cards/20/blockers"))
            .and(body_json(serde_json::json!({ "blocker_card_id": 10 })))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"id": 1, "blocker_card_id": 10}"#),
            )
            .expect(1)
            .mount(&server)
            .await;
        // kind=blocked_by: card 10 is blocked by target 20 -> blocker entry on 10
        Mock::given(method("POST"))
            .and(path("/cards/10/blockers"))
            .and(body_json(
                serde_json::json!({ "blocker_card_id": 20, "reason": "wait" }),
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"id": 2, "blocker_card_id": 20}"#),
            )
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .link_cards(Parameters(super::LinkCardsParams {
                card_id: 10,
                target_id: 20,
                kind: super::LinkKind::Blocks,
                reason: None,
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
        let result = mcp
            .link_cards(Parameters(super::LinkCardsParams {
                card_id: 10,
                target_id: 20,
                kind: super::LinkKind::BlockedBy,
                reason: Some("wait".to_string()),
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn unlink_blocked_by_finds_and_removes_matching_blocker() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cards/10"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"id": 10, "title": "x", "blockers": [
                    {"id": 501, "blocker_card_id": 99},
                    {"id": 502, "blocker_card_id": 20}
                ]}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/cards/10/blockers/502"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .unlink_cards(Parameters(super::UnlinkCardsParams {
                card_id: 10,
                target_id: 20,
                kind: super::LinkKind::BlockedBy,
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn set_card_responsible_patches_member_type_2() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/cards/10/members/77"))
            .and(body_json(serde_json::json!({ "type": 2 })))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"card_id": 10, "user_id": 77, "type": 2, "updated": "2026-07-16T15:18:12.114Z"}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .set_card_responsible(Parameters(super::SetCardResponsibleParams {
                card_id: 10,
                user_id: 77,
                responsible: None,
            }))
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(value["responsible"], true);
    }

    /// The exact trap a live agent hit on v0.1: unknown params (e.g.
    /// `archived`/`condition` sent to update_card) were silently dropped and
    /// the tool reported success. deny_unknown_fields turns that into a
    /// validation error instead of a silent no-op.
    #[test]
    fn unknown_params_are_rejected_not_silently_dropped() {
        let err = serde_json::from_value::<super::UpdateCardParams>(serde_json::json!({
            "card_id": 1, "archived": true
        }))
        .unwrap_err();
        assert!(err.to_string().contains("archived"), "{err}");

        let err = serde_json::from_value::<super::UpdateCardParams>(serde_json::json!({
            "card_id": 1, "condition": 2
        }))
        .unwrap_err();
        assert!(err.to_string().contains("condition"), "{err}");

        let err = serde_json::from_value::<super::PollUpdatesParams>(serde_json::json!({
            "since": "2026-07-17T00:00:00Z", "updated_after": "typo"
        }))
        .unwrap_err();
        assert!(err.to_string().contains("updated_after"), "{err}");
    }

    #[test]
    fn registers_exactly_40_tools_with_spec_names() {
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
            "archive_card",
            "add_card_member",
            "remove_card_member",
            "list_users",
            "list_comments",
            "add_comment",
            "list_checklists",
            "add_checklist",
            "add_checklist_item",
            "set_checklist_item_checked",
            "add_card_tag",
            "remove_card_tag",
            "list_card_types",
            "poll_updates",
            "list_custom_properties",
            "list_property_select_values",
            "link_cards",
            "unlink_cards",
            "release_blocks",
            "attach_file",
            "detach_file",
            "update_comment",
            "remove_comment",
            "set_card_responsible",
            "add_time_log",
            "list_time_logs",
            "download_file",
            "list_card_external_links",
            "add_card_external_link",
            "update_card_external_link",
            "remove_card_external_link",
        ];
        expected.sort_unstable();
        assert_eq!(names, expected);
    }

    // --- issue #12: download_file ---

    fn card_with_storage(server: &MockServer) -> String {
        CARD_FULL_FIXTURE.replace("https://files.kaiten.ru", &server.uri())
    }

    async fn mock_card_and_classic_file(server: &MockServer, downloads: u64) {
        Mock::given(method("GET"))
            .and(path("/cards/67089469"))
            .respond_with(ResponseTemplate::new(200).set_body_string(card_with_storage(server)))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/d4586f6a-3e00-4253-aac7-a6f6c4190f40.txt"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"attachment body".to_vec()))
            .expect(downloads)
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn download_file_saves_into_given_dir_and_returns_descriptor() {
        let server = MockServer::start().await;
        mock_card_and_classic_file(&server, 1).await;
        let dir = tempfile::tempdir().unwrap();

        let mcp = mcp_for(&server);
        let result = mcp
            .download_file(Parameters(super::DownloadFileParams {
                card_id: 67_089_469,
                file_id: "62769658".into(),
                save_path: Some(dir.path().to_string_lossy().into_owned()),
                overwrite: None,
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true), "{}", tool_text(&result));
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        let expected = dir.path().join("probe-attach.txt");
        assert_eq!(value["path"], expected.to_string_lossy().as_ref());
        assert_eq!(value["name"], "probe-attach.txt");
        assert_eq!(value["size"], 15);
        assert_eq!(
            std::fs::read_to_string(expected).unwrap(),
            "attachment body"
        );
    }

    /// Matches a request that carries NO `Authorization` header.
    struct NoAuthHeader;

    impl wiremock::Match for NoAuthHeader {
        fn matches(&self, request: &wiremock::Request) -> bool {
            !request.headers.contains_key("authorization")
        }
    }

    /// Newer storage: the API path (bearer) answers with metadata whose `url`
    /// is a signed storage link; the bytes come from there without the token.
    #[tokio::test]
    async fn download_file_by_uid_follows_the_signed_url_from_the_metadata() {
        let server = MockServer::start().await;
        let storage = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cards/5"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"id": 5, "title": "t", "files": [{"id": "6a8e66af-0000-0000-0000-000000000000",
                    "name": "report.xlsx", "size": "3",
                    "url": "/api/v1/cards/cu/files/6a8e66af-0000-0000-0000-000000000000"}]}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/api/v1/cards/cu/files/6a8e66af-0000-0000-0000-000000000000",
            ))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                format!(
                    r#"{{"id": "6a8e66af-0000-0000-0000-000000000000", "url": "{}/signed/blob"}}"#,
                    storage.uri()
                ),
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/signed/blob"))
            .and(NoAuthHeader)
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"xls".to_vec()))
            .expect(1)
            .mount(&storage)
            .await;
        let dir = tempfile::tempdir().unwrap();

        let mcp = mcp_for(&server);
        let result = mcp
            .download_file(Parameters(super::DownloadFileParams {
                card_id: 5,
                file_id: "6a8e66af-0000-0000-0000-000000000000".into(),
                save_path: Some(dir.path().to_string_lossy().into_owned()),
                overwrite: None,
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true), "{}", tool_text(&result));
        assert_eq!(
            std::fs::read(dir.path().join("report.xlsx")).unwrap(),
            b"xls"
        );
    }

    /// No save_path: a per-user cache dir keyed by card and file ref, so two
    /// same-named attachments never clobber each other; the path is absolute.
    #[tokio::test]
    async fn download_file_default_location_is_per_file_under_the_cache_dir() {
        let server = MockServer::start().await;
        mock_card_and_classic_file(&server, 2).await;
        let cache = tempfile::tempdir().unwrap();

        let mut mcp = mcp_for(&server);
        mcp.download_dir = cache.path().to_path_buf();
        for _ in 0..2 {
            // the default location is refreshed, never refused
            let result = mcp
                .download_file(Parameters(super::DownloadFileParams {
                    card_id: 67_089_469,
                    file_id: "62769658".into(),
                    save_path: None,
                    overwrite: None,
                }))
                .await
                .unwrap();
            assert_ne!(result.is_error, Some(true), "{}", tool_text(&result));
            let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
            let expected = cache
                .path()
                .join("67089469")
                .join("62769658")
                .join("probe-attach.txt");
            assert_eq!(value["path"], expected.to_string_lossy().as_ref());
            assert!(std::path::Path::new(value["path"].as_str().unwrap()).is_absolute());
            assert_eq!(
                std::fs::read_to_string(expected).unwrap(),
                "attachment body"
            );
        }
    }

    #[tokio::test]
    async fn download_file_unknown_id_is_tool_error_listing_files() {
        let server = MockServer::start().await;
        mock_card_and_classic_file(&server, 0).await;

        let mcp = mcp_for(&server);
        let result = mcp
            .download_file(Parameters(super::DownloadFileParams {
                card_id: 67_089_469,
                file_id: "999".into(),
                save_path: None,
                overwrite: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let text = tool_text(&result);
        assert!(text.contains("no file `999`"), "{text}");
        assert!(text.contains("62769658"), "{text}");
    }

    #[tokio::test]
    async fn download_file_refuses_existing_save_path_without_overwrite() {
        let server = MockServer::start().await;
        mock_card_and_classic_file(&server, 0).await;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("keep.txt");
        std::fs::write(&target, "old").unwrap();

        let mcp = mcp_for(&server);
        let result = mcp
            .download_file(Parameters(super::DownloadFileParams {
                card_id: 67_089_469,
                file_id: "62769658".into(),
                save_path: Some(target.to_string_lossy().into_owned()),
                overwrite: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        assert!(
            tool_text(&result).contains("overwrite=true"),
            "{}",
            tool_text(&result)
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "old");
    }
    fn update_params(properties: serde_json::Value) -> super::UpdateCardParams {
        super::UpdateCardParams {
            card_id: 67_089_469,
            title: None,
            description: None,
            type_id: None,
            asap: None,
            properties: Some(properties),
        }
    }

    /// Issue #15: a JSON *string* holding the object is applied, not dropped.
    #[tokio::test]
    async fn update_card_parses_stringified_properties_object() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/cards/67089469"))
            .and(body_json(serde_json::json!({
                "properties": { "id_612634": [18_929_916] }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(CARD_CREATE_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .update_card(Parameters(update_params(serde_json::json!(
                "{\"id_612634\": [18929916]}"
            ))))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true), "{}", tool_text(&result));
    }

    /// Issue #15: the wrong shape is a tool-level error raised before any
    /// API call — previously it was forwarded and silently ignored.
    #[tokio::test]
    async fn update_card_rejects_non_object_properties_before_any_request() {
        let server = MockServer::start().await;
        let mcp = mcp_for(&server);
        for bad in [
            serde_json::json!("abc"),
            serde_json::json!("[1]"),
            serde_json::json!(42),
            serde_json::json!([{ "id_1": 2 }]),
        ] {
            let result = mcp
                .update_card(Parameters(update_params(bad.clone())))
                .await
                .unwrap();
            assert_eq!(result.is_error, Some(true), "{bad}");
            let text = tool_text(&result);
            assert!(text.starts_with("properties "), "{text}");
            assert!(
                text.contains("JSON object") || text.contains("not valid JSON"),
                "{text}"
            );
        }
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "invalid properties must never reach the API"
        );
    }

    #[tokio::test]
    async fn create_card_parses_stringified_properties_object() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cards"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "board_id": 1,
                "properties": { "id_612634": [18_929_916] }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(CARD_CREATE_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .create_card(Parameters(CreateCardParams {
                board_id: 1,
                title: "t".into(),
                column_id: None,
                lane_id: None,
                description: None,
                type_id: None,
                asap: None,
                properties: Some(serde_json::json!("{\"id_612634\": [18929916]}")),
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true), "{}", tool_text(&result));
    }

    #[tokio::test]
    async fn create_card_rejects_non_object_properties_before_any_request() {
        let server = MockServer::start().await;
        let mcp = mcp_for(&server);
        let result = mcp
            .create_card(Parameters(CreateCardParams {
                board_id: 1,
                title: "t".into(),
                column_id: None,
                lane_id: None,
                description: None,
                type_id: None,
                asap: None,
                properties: Some(serde_json::json!("nope")),
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        assert!(
            tool_text(&result).starts_with("properties "),
            "{}",
            tool_text(&result)
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    /// Issue #15: the mutation result echoes the resulting properties so the
    /// agent can verify its write without a second call.
    #[tokio::test]
    async fn update_card_echoes_properties_in_mutation_result() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/cards/67089469"))
            .respond_with(ResponseTemplate::new(200).set_body_string(CARD_WITH_PROPERTIES_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;

        let mcp = mcp_for(&server);
        let result = mcp
            .update_card(Parameters(update_params(
                serde_json::json!({ "id_19": "S" }),
            )))
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(
            value["properties"],
            serde_json::json!({ "id_19": "S" }),
            "{value}"
        );
    }

    /// Issue #15: a client can tell from the schema that an object is required.
    #[test]
    fn create_and_update_card_schemas_type_properties_as_object() {
        let tools = KaitenMcp::tool_router().list_all();
        for name in ["create_card", "update_card"] {
            let tool = tools.iter().find(|t| t.name == name).unwrap();
            let schema = serde_json::to_value(tool.input_schema.as_ref()).unwrap();
            let prop = &schema["properties"]["properties"];
            // schemars renders `Option<Map>` as `"type": ["object", "null"]`;
            // accept the scalar, the type array and an `anyOf` encoding.
            let mentions_object = |v: &serde_json::Value| {
                v == "object"
                    || v.as_array()
                        .is_some_and(|ts| ts.iter().any(|t| t == "object"))
            };
            let object_typed = mentions_object(&prop["type"])
                || prop["anyOf"]
                    .as_array()
                    .is_some_and(|alts| alts.iter().any(|s| mentions_object(&s["type"])));
            assert!(object_typed, "{name}: {prop}");
            assert!(
                prop["description"]
                    .as_str()
                    .is_some_and(|d| d.contains("id_")),
                "{name} lost its description: {prop}"
            );
        }
    }

    // --- issue #21: external links ---------------------------------------

    const EXTERNAL_LINKS_FIXTURE: &str =
        include_str!("../../tests/fixtures/external_links_list.json");
    const EXTERNAL_LINK_ADD_FIXTURE: &str =
        include_str!("../../tests/fixtures/external_link_add.json");

    async fn mount_external_links(server: &MockServer, expect: u64) {
        Mock::given(method("GET"))
            .and(path("/cards/67089469/external-links"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(EXTERNAL_LINKS_FIXTURE, "application/json"),
            )
            .expect(expect)
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn list_card_external_links_returns_compact_views() {
        let server = MockServer::start().await;
        mount_external_links(&server, 1).await;
        let mcp = mcp_for(&server);

        let result = mcp
            .list_card_external_links(Parameters(ListCardExternalLinksParams {
                card_id: 67_089_469,
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
        let links: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(links.as_array().unwrap().len(), 2);
        assert_eq!(links[0]["id"], 21_177_131);
        assert_eq!(links[0]["url"], "https://example.com/spike");
        assert_eq!(links[0]["description"], "Source");
        assert_eq!(links[0]["created"], "2026-08-27T13:24:19.088Z");
        assert!(links[1].get("description").is_none(), "{links}");
        for noisy in ["uid", "card_id", "external_link_id", "external_link_uid"] {
            assert!(links[0].get(noisy).is_none(), "{noisy} leaked: {links}");
        }
    }

    #[tokio::test]
    async fn add_card_external_link_posts_and_returns_the_view() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cards/67089469/external-links"))
            .and(header("Authorization", "Bearer test-token"))
            .and(body_json(serde_json::json!({
                "url": "https://example.com/spike",
                "description": "Source"
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(EXTERNAL_LINK_ADD_FIXTURE, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let mcp = mcp_for(&server);

        let result = mcp
            .add_card_external_link(Parameters(AddCardExternalLinkParams {
                card_id: 67_089_469,
                url: "https://example.com/spike".into(),
                description: Some("Source".into()),
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true), "{}", tool_text(&result));
        let link: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(link["id"], 21_177_131);
        assert_eq!(link["url"], "https://example.com/spike");
    }

    /// Kaiten stores any string as a url; the tool refuses garbage before
    /// any request and never echoes the value (it may hold a secret).
    #[tokio::test]
    async fn add_card_external_link_rejects_a_bad_url_before_any_request() {
        let server = MockServer::start().await;
        let mcp = mcp_for(&server);

        for bad in ["notaurl", "ftp://host/x", "https://user:s3cret@host/x", " "] {
            let result = mcp
                .add_card_external_link(Parameters(AddCardExternalLinkParams {
                    card_id: 67_089_469,
                    url: bad.into(),
                    description: None,
                }))
                .await
                .unwrap();
            assert_eq!(result.is_error, Some(true), "{bad:?}");
            let text = tool_text(&result);
            assert!(text.starts_with("url "), "{text}");
            assert!(!text.contains("s3cret"), "{text}");
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn update_card_external_link_patches_only_the_given_fields() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/cards/67089469/external-links/21177131"))
            .and(body_json(serde_json::json!({
                "description": "patched"
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(EXTERNAL_LINK_ADD_FIXTURE, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let mcp = mcp_for(&server);

        let result = mcp
            .update_card_external_link(Parameters(UpdateCardExternalLinkParams {
                card_id: 67_089_469,
                link_id: 21_177_131,
                url: None,
                description: Some("patched".into()),
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true), "{}", tool_text(&result));
        let link: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(link["id"], 21_177_131);
    }

    #[tokio::test]
    async fn update_card_external_link_sends_both_fields_when_given() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/cards/67089469/external-links/21177131"))
            .and(body_json(serde_json::json!({
                "url": "https://example.org/moved",
                "description": "moved"
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(EXTERNAL_LINK_ADD_FIXTURE, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let mcp = mcp_for(&server);

        let result = mcp
            .update_card_external_link(Parameters(UpdateCardExternalLinkParams {
                card_id: 67_089_469,
                link_id: 21_177_131,
                url: Some("https://example.org/moved".into()),
                description: Some("moved".into()),
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true), "{}", tool_text(&result));
    }

    #[tokio::test]
    async fn update_card_external_link_rejects_a_bad_url_before_any_request() {
        let server = MockServer::start().await;
        let mcp = mcp_for(&server);

        let result = mcp
            .update_card_external_link(Parameters(UpdateCardExternalLinkParams {
                card_id: 67_089_469,
                link_id: 21_177_131,
                url: Some("https://user:s3cret@host/x".into()),
                description: Some("not sent".into()),
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let text = tool_text(&result);
        assert!(text.starts_with("url "), "{text}");
        assert!(!text.contains("s3cret"), "{text}");
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn update_card_external_link_without_changes_is_a_tool_error() {
        let server = MockServer::start().await;
        let mcp = mcp_for(&server);

        let result = mcp
            .update_card_external_link(Parameters(UpdateCardExternalLinkParams {
                card_id: 67_089_469,
                link_id: 21_177_131,
                url: None,
                description: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let text = tool_text(&result);
        assert!(
            text.contains("url") && text.contains("description"),
            "{text}"
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn remove_card_external_link_reports_the_link_id() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/cards/67089469/external-links/21177131"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id": 21177131}"#))
            .expect(1)
            .mount(&server)
            .await;
        let mcp = mcp_for(&server);

        let result = mcp
            .remove_card_external_link(Parameters(RemoveCardExternalLinkParams {
                card_id: 67_089_469,
                link_id: 21_177_131,
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true), "{}", tool_text(&result));
        let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "removed": true, "link_id": 21_177_131 })
        );
    }

    /// The card response carries its external links: `get_card` returns them
    /// without any extra request, with or without `include`; `comments` is
    /// still an opt-in second request.
    #[tokio::test]
    async fn get_card_returns_external_links_from_the_card_and_fetches_comments_on_include() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cards/67089469"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(CARD_FULL_FIXTURE, "application/json"),
            )
            .expect(3)
            .mount(&server)
            .await;
        mount_external_links(&server, 0).await;
        Mock::given(method("GET"))
            .and(path("/cards/67089469/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"[{"id": 5, "text": "hi", "created": "2026-08-27T10:00:00.000Z"}]"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;
        let mcp = mcp_for(&server);

        let plain = mcp
            .get_card(Parameters(GetCardParams {
                card_id: 67_089_469,
                include: None,
            }))
            .await
            .unwrap();
        let plain: serde_json::Value = serde_json::from_str(&tool_text(&plain)).unwrap();
        assert_eq!(
            plain["external_links"][0]["url"], "https://example.com/fixture-link",
            "{plain}"
        );
        assert!(plain.get("comments").is_none(), "{plain}");

        let links_only = mcp
            .get_card(Parameters(GetCardParams {
                card_id: 67_089_469,
                include: Some(vec![IncludeSection::ExternalLinks]),
            }))
            .await
            .unwrap();
        let links_only: serde_json::Value = serde_json::from_str(&tool_text(&links_only)).unwrap();
        assert_eq!(links_only["external_links"][0]["id"], 21_181_168);
        assert!(links_only.get("comments").is_none(), "{links_only}");

        let both = mcp
            .get_card(Parameters(GetCardParams {
                card_id: 67_089_469,
                include: Some(vec![
                    IncludeSection::ExternalLinks,
                    IncludeSection::Comments,
                ]),
            }))
            .await
            .unwrap();
        let both: serde_json::Value = serde_json::from_str(&tool_text(&both)).unwrap();
        assert_eq!(both["external_links"].as_array().unwrap().len(), 1);
        assert_eq!(both["comments"][0]["text"], "hi");
        assert_eq!(both["id"], 67_089_469, "{both}");
    }

    /// `include: ["external_links"]` on a card without links keeps the pre-0.6
    /// shape — an empty array — instead of dropping the key.
    #[tokio::test]
    async fn get_card_include_external_links_on_a_card_without_links_gives_an_empty_array() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cards/67089469"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                include_str!("../../tests/fixtures/card_with_files.json"),
                "application/json",
            ))
            .expect(2)
            .mount(&server)
            .await;
        mount_external_links(&server, 0).await;
        let mcp = mcp_for(&server);

        let plain = mcp
            .get_card(Parameters(GetCardParams {
                card_id: 67_089_469,
                include: None,
            }))
            .await
            .unwrap();
        let plain: serde_json::Value = serde_json::from_str(&tool_text(&plain)).unwrap();
        assert!(plain.get("external_links").is_none(), "{plain}");

        let requested = mcp
            .get_card(Parameters(GetCardParams {
                card_id: 67_089_469,
                include: Some(vec![IncludeSection::ExternalLinks]),
            }))
            .await
            .unwrap();
        let requested: serde_json::Value = serde_json::from_str(&tool_text(&requested)).unwrap();
        assert_eq!(
            requested["external_links"],
            serde_json::json!([]),
            "{requested}"
        );
    }

    #[test]
    fn get_card_include_schema_lists_the_section_names() {
        let tools = KaitenMcp::tool_router().list_all();
        let get_card = tools.iter().find(|t| t.name == "get_card").unwrap();
        let schema = serde_json::to_value(&get_card.input_schema).unwrap();
        assert_eq!(
            schema["properties"]["include"]["items"]["$ref"], "#/$defs/IncludeSection",
            "{schema}"
        );
        assert_eq!(
            schema["$defs"]["IncludeSection"]["enum"],
            serde_json::json!(["external_links", "comments"]),
            "{schema}"
        );
        assert!(
            schema["required"]
                .as_array()
                .is_none_or(|r| !r.iter().any(|v| v == "include")),
            "include must be optional: {schema}"
        );
    }
}
