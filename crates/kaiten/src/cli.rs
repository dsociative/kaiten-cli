use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "kaiten",
    version,
    about = "Kaiten tracker CLI",
    propagate_version = true
)]
pub struct Cli {
    /// Print raw JSON instead of tables
    #[arg(long, global = true)]
    pub json: bool,

    /// Increase log verbosity (-v: debug, -vv: trace)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Log in and inspect authentication
    #[command(subcommand)]
    Auth(AuthCmd),
    /// Work with spaces
    #[command(subcommand)]
    Space(SpaceCmd),
    /// Work with boards
    #[command(subcommand)]
    Board(BoardCmd),
    /// Work with cards
    #[command(subcommand)]
    Card(CardCmd),
    /// Work with company tags
    #[command(subcommand)]
    Tag(TagCmd),
    /// Work with card types
    #[command(name = "card-type", subcommand)]
    CardType(CardTypeCmd),
    /// Company custom properties (reference for --properties-json)
    #[command(subcommand)]
    Property(PropertyCmd),
    /// Raw API request (like `gh api`)
    Api {
        /// HTTP method: GET|POST|PATCH|PUT|DELETE
        method: String,
        /// Path starting with '/', query string included
        path: String,
        /// JSON request body
        #[arg(long)]
        data: Option<String>,
    },
    /// Generate shell completion script
    Completion {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// MCP server
    #[command(subcommand)]
    Mcp(McpCmd),
}

#[derive(Subcommand)]
pub enum AuthCmd {
    /// Verify credentials against /users/current and save config
    Login {
        /// Kaiten domain: <domain>.kaiten.ru
        #[arg(long)]
        domain: Option<String>,
        /// API token (from your Kaiten profile)
        #[arg(long)]
        token: Option<String>,
    },
    /// Show current authentication info
    Status,
}

#[derive(Subcommand)]
pub enum PropertyCmd {
    /// List company custom properties
    List,
    /// List select values of a property
    Values { property_id: u64 },
}

#[derive(Subcommand)]
pub enum SpaceCmd {
    /// List spaces
    List,
}

#[derive(Subcommand)]
pub enum BoardCmd {
    /// List boards of a space
    List {
        /// Space id (default: defaults.space from config)
        #[arg(long)]
        space: Option<u64>,
    },
    /// Show board columns and lanes
    View { board_id: u64 },
}

/// Card state for `card list --state` (API values 1/2/3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum CardState {
    Queued,
    InProgress,
    Done,
}

impl CardState {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Queued => 1,
            Self::InProgress => 2,
            Self::Done => 3,
        }
    }
}

#[derive(Subcommand)]
pub enum CardCmd {
    /// List cards
    List {
        /// Filter by space id
        #[arg(long)]
        space: Option<u64>,
        /// Filter by board id
        #[arg(long)]
        board: Option<u64>,
        /// Filter by column id
        #[arg(long)]
        column: Option<u64>,
        /// Only cards where I am a member
        #[arg(long)]
        mine: bool,
        /// Only cards where user <id> is a member
        #[arg(long)]
        member: Option<u64>,
        /// Full-text search query
        #[arg(long)]
        query: Option<String>,
        /// Filter by tag name
        #[arg(long)]
        tag: Option<String>,
        /// Filter by card type id
        #[arg(long = "type")]
        type_id: Option<u64>,
        /// Include only archived cards
        #[arg(long)]
        archived: bool,
        /// Filter by state (repeatable)
        #[arg(long = "state", value_enum)]
        states: Vec<CardState>,
        /// Only cards updated at/after this ISO 8601 time (inclusive)
        #[arg(long)]
        updated_after: Option<String>,
        /// Only cards created at/after this ISO 8601 time (inclusive)
        #[arg(long)]
        created_after: Option<String>,
        /// Sort by a card field, e.g. "updated" or "created"
        #[arg(long)]
        sort: Option<String>,
        /// Sort descending (with --sort)
        #[arg(long, requires = "sort")]
        desc: bool,
        /// Max number of cards (the server caps this at 100)
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Number of cards to skip (pagination; use with --limit)
        #[arg(long)]
        offset: Option<u32>,
    },
    /// Show one card (accepts id or browser URL)
    View {
        card: String,
        /// Also fetch and print comments
        #[arg(long)]
        comments: bool,
    },
    /// Create a card
    Create {
        #[arg(long)]
        title: String,
        /// Board id (default: defaults.board from config)
        #[arg(long)]
        board: Option<u64>,
        #[arg(long)]
        column: Option<u64>,
        #[arg(long)]
        lane: Option<u64>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long = "type")]
        type_id: Option<u64>,
        /// Mark card as ASAP
        #[arg(long)]
        asap: bool,
        /// Custom property values as JSON, keyed as id_{property_id}
        /// (see `kaiten property list`), e.g. '{"id_612634": [18929916]}'
        #[arg(long = "properties-json")]
        properties_json: Option<String>,
    },
    /// Edit card fields
    Edit {
        card: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long = "type")]
        type_id: Option<u64>,
        /// true|false
        #[arg(long)]
        asap: Option<bool>,
        /// Custom property values as JSON, keyed as id_{property_id}
        /// (see `kaiten property list`), e.g. '{"id_612634": [18929916]}'
        #[arg(long = "properties-json")]
        properties_json: Option<String>,
    },
    /// Move card to another column/lane/board
    Move {
        card: String,
        #[arg(long)]
        column: u64,
        #[arg(long)]
        lane: Option<u64>,
        #[arg(long)]
        board: Option<u64>,
    },
    /// Archive card
    Archive { card: String },
    /// Link the card to another card (hierarchy or blocking)
    Link {
        card: String,
        /// Make <CHILD> a child of the card
        #[arg(long, group = "link_kind")]
        child: Option<u64>,
        /// Make <PARENT> a parent of the card
        #[arg(long, group = "link_kind")]
        parent: Option<u64>,
        /// The card blocks <BLOCKS>
        #[arg(long, group = "link_kind")]
        blocks: Option<u64>,
        /// The card is blocked by <BLOCKED_BY>
        #[arg(long = "blocked-by", group = "link_kind")]
        blocked_by: Option<u64>,
        /// Block reason (with --blocks/--blocked-by)
        #[arg(long, requires = "link_kind")]
        reason: Option<String>,
    },
    /// Remove a card link (same flags as `card link`)
    Unlink {
        card: String,
        #[arg(long, group = "link_kind")]
        child: Option<u64>,
        #[arg(long, group = "link_kind")]
        parent: Option<u64>,
        #[arg(long, group = "link_kind")]
        blocks: Option<u64>,
        #[arg(long = "blocked-by", group = "link_kind")]
        blocked_by: Option<u64>,
    },
    /// Release ALL blocks on the card
    Unblock { card: String },
    /// Card members
    #[command(subcommand)]
    Member(CardMemberCmd),
    /// Card comments
    #[command(subcommand)]
    Comment(CardCommentCmd),
    /// Card checklists
    #[command(subcommand)]
    Checklist(CardChecklistCmd),
    /// Card tags
    #[command(subcommand)]
    Tag(CardTagCmd),
    /// Card file attachments
    #[command(subcommand)]
    File(CardFileCmd),
}

#[derive(Subcommand)]
pub enum CardFileCmd {
    /// Attach a local file (served from a PUBLIC unguessable URL!)
    Add {
        card: String,
        /// Path of the file to upload
        path: std::path::PathBuf,
    },
    /// Detach a file by id (see `card view`)
    Rm { card: String, file_id: u64 },
}

#[derive(Subcommand)]
pub enum CardMemberCmd {
    /// Add member (user id or email)
    Add { card: String, user: String },
    /// Remove member (user id or email)
    Remove { card: String, user: String },
}

#[derive(Subcommand)]
pub enum CardCommentCmd {
    /// Add a comment
    Add {
        card: String,
        #[arg(long)]
        body: String,
    },
    /// List comments
    List { card: String },
}

#[derive(Subcommand)]
pub enum CardChecklistCmd {
    /// List checklists with items
    List { card: String },
    /// Add a checklist
    Add {
        card: String,
        #[arg(long)]
        name: String,
    },
    /// Checklist items
    #[command(subcommand)]
    Item(CardChecklistItemCmd),
}

#[derive(Subcommand)]
pub enum CardChecklistItemCmd {
    /// Add an item
    Add {
        card: String,
        checklist_id: u64,
        #[arg(long)]
        text: String,
    },
    /// Check an item
    Check {
        card: String,
        checklist_id: u64,
        item_id: u64,
    },
    /// Uncheck an item
    Uncheck {
        card: String,
        checklist_id: u64,
        item_id: u64,
    },
}

#[derive(Subcommand)]
pub enum CardTagCmd {
    /// Add tag by name
    Add { card: String, name: String },
    /// Remove tag by name
    Remove { card: String, name: String },
}

#[derive(Subcommand)]
pub enum TagCmd {
    /// List company tags
    List,
}

#[derive(Subcommand)]
pub enum CardTypeCmd {
    /// List card types
    List,
}

#[derive(Subcommand)]
pub enum McpCmd {
    /// Run MCP server on stdio
    Serve,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}
