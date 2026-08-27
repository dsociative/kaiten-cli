//! All Kaiten API models.
//!
//! Deserialization is tolerant: unknown fields are ignored (no
//! `deny_unknown_fields`), fields that may be absent in a particular
//! response are `Option<...>` with `#[serde(default)]`.
//! Dates are plain ISO strings (no chrono).

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub id: u64,
    #[serde(default)]
    pub uid: Option<String>,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub activated: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Space {
    pub id: u64,
    #[serde(default)]
    pub uid: Option<String>,
    pub title: String,
    #[serde(default)]
    pub archived: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Column {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub board_id: Option<u64>,
    /// 1 = queued, 2 = in progress, 3 = done
    #[serde(rename = "type", default)]
    pub column_type: Option<u8>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Lane {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub board_id: Option<u64>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

/// A nested `board` inside a card has no `columns`/`lanes` keys,
/// so both default to empty vectors.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Board {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub columns: Vec<Column>,
    #[serde(default)]
    pub lanes: Vec<Lane>,
    #[serde(default)]
    pub default_card_type_id: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CardType {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub letter: Option<String>,
    #[serde(default)]
    pub color: Option<i64>,
    #[serde(default)]
    pub archived: Option<bool>,
}

/// A tag inside `card.tags`: `id` is the link id, `tag_id` is the company tag id.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CardTag {
    pub id: u64,
    #[serde(default)]
    pub tag_id: Option<u64>,
    pub name: String,
    #[serde(default)]
    pub color: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CardMember {
    /// User id. ABSENT in PATCH /members/{id} responses (only `user_id`
    /// is present there), hence the default.
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub user_id: Option<u64>,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    /// 2 = responsible
    #[serde(rename = "type", default)]
    pub member_type: Option<u8>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChecklistItem {
    pub id: u64,
    pub text: String,
    #[serde(default)]
    pub checked: Option<bool>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Checklist {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub items: Vec<ChecklistItem>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

/// A blocker entry inside `card.blockers`.
///
/// The `blockers` key is ABSENT from the API response until the card has
/// been blocked at least once, hence `#[serde(default)]` on `Card.blockers`.
/// The blocking card is referenced by `blocker_card_id`/`blocker_card_title`
/// (the `blocker` key in the raw JSON is the *user* who created the block).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Blocker {
    pub id: u64,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub blocker_card_id: Option<u64>,
    #[serde(default)]
    pub blocker_card_title: Option<String>,
    #[serde(default)]
    pub blocker_id: Option<u64>,
    #[serde(default)]
    pub released: Option<bool>,
    #[serde(default)]
    pub created: Option<String>,
}

/// A file attached to a card.
///
/// Two wire shapes exist (issue #12). The classic storage (`type` 1) sends
/// a numeric `id` and an absolute `url` on the public file host, served
/// WITHOUT authentication (an unguessable link — treat every attachment as
/// public). The newer storage (`type` 11) identifies the file by a UUID in
/// `id`, sends `size` as a string and a host-root-relative `url` under
/// `/api/v1` that requires the API token. Both parse into this struct: `id`
/// is `0` when the API identifies the file only by a UUID — address such
/// files through [`FileRef`], which recovers the UUID from `url`. Parsing is
/// tolerant only for self-describing formats such as JSON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(from = "RawCardFile")]
pub struct CardFile {
    /// Numeric id on the classic storage; `0` on the newer storage (see above).
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(rename = "type", default)]
    pub file_type: Option<u8>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub external: Option<bool>,
    #[serde(default)]
    pub deleted: Option<bool>,
    #[serde(default)]
    pub author_id: Option<u64>,
    #[serde(default)]
    pub created: Option<String>,
}

/// Wire form of [`CardFile`], tolerant of both storages' encodings. Field
/// names mirror `CardFile` so `serde_path_to_error` paths stay the same.
#[derive(serde::Deserialize)]
struct RawCardFile {
    id: de::NumOrStr,
    name: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default, deserialize_with = "de::opt_u64_or_string")]
    size: Option<u64>,
    #[serde(rename = "type", default)]
    file_type: Option<u8>,
    #[serde(default)]
    mime_type: Option<String>,
    #[serde(default)]
    external: Option<bool>,
    #[serde(default)]
    deleted: Option<bool>,
    #[serde(default)]
    author_id: Option<u64>,
    #[serde(default)]
    created: Option<String>,
}

impl From<RawCardFile> for CardFile {
    fn from(raw: RawCardFile) -> Self {
        Self {
            id: raw.id.into_u64().unwrap_or(0),
            name: raw.name,
            url: raw.url,
            size: raw.size,
            file_type: raw.file_type,
            mime_type: raw.mime_type,
            external: raw.external,
            deleted: raw.deleted,
            author_id: raw.author_id,
            created: raw.created,
        }
    }
}

impl CardFile {
    /// UUID under which the newer storage addresses the file: the last path
    /// segment of `url` (`/api/v1/cards/{card_uid}/files/{uuid}`), minus any
    /// extension (a classic url ends in `<uuid>.ext`).
    fn uuid_from_url(&self) -> Option<&str> {
        let path = self.url.as_deref()?.split(['?', '#']).next()?;
        let last = path.trim_end_matches('/').rsplit('/').next()?;
        let stem = last.split('.').next()?;
        (!stem.is_empty()).then_some(stem)
    }
}

/// Deserializers for values the newer file storage sends as strings.
mod de {
    use serde::de::{Error, Visitor};

    /// A JSON number, or a string holding one (`"58818"`).
    pub(super) enum NumOrStr {
        Num(u64),
        Str(String),
    }

    impl NumOrStr {
        pub(super) fn into_u64(self) -> Option<u64> {
            match self {
                NumOrStr::Num(n) => Some(n),
                NumOrStr::Str(s) => s.trim().parse().ok(),
            }
        }
    }

    impl<'de> serde::Deserialize<'de> for NumOrStr {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;
            impl Visitor<'_> for V {
                type Value = NumOrStr;

                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("a non-negative integer or a string")
                }

                fn visit_u64<E: Error>(self, v: u64) -> Result<NumOrStr, E> {
                    Ok(NumOrStr::Num(v))
                }

                fn visit_i64<E: Error>(self, v: i64) -> Result<NumOrStr, E> {
                    u64::try_from(v)
                        .map(NumOrStr::Num)
                        .map_err(|_| E::custom("negative integer"))
                }

                fn visit_str<E: Error>(self, v: &str) -> Result<NumOrStr, E> {
                    Ok(NumOrStr::Str(v.to_owned()))
                }

                fn visit_string<E: Error>(self, v: String) -> Result<NumOrStr, E> {
                    Ok(NumOrStr::Str(v))
                }
            }
            d.deserialize_any(V)
        }
    }

    /// `null` → `None`; a number, or a string holding one, → the value; a
    /// string holding anything else → `None`. Other JSON types still error.
    pub(super) fn opt_u64_or_string<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Option<u64>, D::Error> {
        Ok(<Option<NumOrStr> as serde::Deserialize>::deserialize(d)?.and_then(NumOrStr::into_u64))
    }
}

/// Addresses an attachment for the download APIs: the classic storage's
/// numeric id, or the newer storage's UUID. Parse one from user input with
/// `str::parse` (digits → [`Id`](FileRef::Id), anything else →
/// [`Uid`](FileRef::Uid)); derive one from a [`CardFile`] with `From`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRef {
    Id(u64),
    Uid(String),
}

impl FileRef {
    /// Does this reference address `file`? `Id(0)` never matches — `0` is
    /// the sentinel for "identified by UUID only".
    #[must_use]
    pub fn matches(&self, file: &CardFile) -> bool {
        match self {
            FileRef::Id(0) => false,
            FileRef::Id(id) => file.id == *id,
            FileRef::Uid(uid) => file
                .uuid_from_url()
                .is_some_and(|u| u.eq_ignore_ascii_case(uid)),
        }
    }
}

impl From<&CardFile> for FileRef {
    fn from(file: &CardFile) -> Self {
        if file.id != 0 {
            return FileRef::Id(file.id);
        }
        match file.uuid_from_url() {
            Some(uid) => FileRef::Uid(uid.to_owned()),
            None => FileRef::Id(0),
        }
    }
}

impl std::fmt::Display for FileRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileRef::Id(id) => write!(f, "{id}"),
            FileRef::Uid(uid) => f.write_str(uid),
        }
    }
}

impl std::str::FromStr for FileRef {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        Ok(match s.parse::<u64>() {
            Ok(id) => FileRef::Id(id),
            Err(_) => FileRef::Uid(s.to_owned()),
        })
    }
}

/// GET /cards/{id} returns the full card; GET /cards returns cards
/// without `description`/`members`/`checklists` — the same model parses both.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Card {
    pub id: u64,
    pub title: String,
    /// Human-readable card key; `null` unless the feature is enabled in Kaiten.
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub asap: Option<bool>,
    #[serde(default)]
    pub archived: Option<bool>,
    /// 1 = live, 2 = archived
    #[serde(default)]
    pub condition: Option<u8>,
    /// 1 = queued, 2 = in progress, 3 = done
    #[serde(default)]
    pub state: Option<u8>,
    #[serde(default)]
    pub board_id: Option<u64>,
    #[serde(default)]
    pub column_id: Option<u64>,
    #[serde(default)]
    pub lane_id: Option<u64>,
    #[serde(default)]
    pub type_id: Option<u64>,
    #[serde(default)]
    pub owner_id: Option<u64>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub comments_total: Option<u32>,
    /// Comments do NOT bump `updated` — this is the only signal they leave.
    #[serde(default)]
    pub comment_last_added_at: Option<String>,
    /// Nested board has no `columns`/`lanes` keys → they default to empty.
    #[serde(default)]
    pub board: Option<Board>,
    #[serde(default)]
    pub column: Option<Column>,
    #[serde(default)]
    pub lane: Option<Lane>,
    #[serde(rename = "type", default)]
    pub card_type: Option<CardType>,
    #[serde(default)]
    pub owner: Option<User>,
    #[serde(default)]
    pub members: Vec<CardMember>,
    #[serde(default)]
    pub tags: Vec<CardTag>,
    #[serde(default)]
    pub checklists: Vec<Checklist>,
    /// Custom properties; `null` when the card has none.
    #[serde(default)]
    pub properties: Option<serde_json::Value>,
    #[serde(default)]
    pub children_count: Option<u32>,
    #[serde(default)]
    pub parents_count: Option<u32>,
    #[serde(default)]
    pub blocked: Option<bool>,
    /// Linked cards: embedded in GET /cards/{id}, absent from list responses.
    #[serde(default)]
    pub children: Vec<Card>,
    #[serde(default)]
    pub parents: Vec<Card>,
    /// Conditional key: absent until the card has been blocked at least once.
    #[serde(default)]
    pub blockers: Vec<Blocker>,
    #[serde(default)]
    pub files: Vec<CardFile>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Comment {
    pub id: u64,
    pub text: String,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub edited: Option<bool>,
    #[serde(default)]
    pub author: Option<User>,
    #[serde(default)]
    pub author_id: Option<u64>,
}

/// Company-level tag (GET /tags, POST /cards/{id}/tags).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Tag {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub color: Option<i64>,
}

/// A time log entry (GET/POST /cards/{id}/time-logs).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimeLog {
    pub id: u64,
    /// Minutes.
    pub time_spent: i64,
    #[serde(default)]
    pub for_date: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    /// User role id; built-in roles have NEGATIVE ids (e.g. -1 = Employee).
    #[serde(default)]
    pub role_id: Option<i64>,
    #[serde(default)]
    pub author_id: Option<u64>,
    #[serde(default)]
    pub created: Option<String>,
}

/// Company user role (GET /user-roles). Built-in roles have negative ids.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserRole {
    pub id: i64,
    pub name: String,
}

/// Company-level custom property (GET /company/custom-properties).
///
/// Values are written through card create/update `properties`, keyed as
/// `id_{property_id}`; select values are referenced by id (see
/// [`SelectValue`]) and passed as an ARRAY even for single select.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CustomProperty {
    pub id: u64,
    pub name: String,
    /// "select", "multi_select", "string", "number", "date", ...
    #[serde(rename = "type")]
    pub property_type: String,
    #[serde(default)]
    pub archived: Option<bool>,
}

/// One option of a select-type custom property
/// (GET /company/custom-properties/{id}/select-values).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SelectValue {
    pub id: u64,
    pub value: String,
    #[serde(default)]
    pub color: Option<i64>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

/// An external link of a card (`Links (common links)` in Kaiten): a URL
/// with an optional description. `GET /cards/{id}` embeds them under
/// `external_links` as well, but [`Card`] does not model that field yet
/// (adding one would be a breaking change), so they are read through
/// [`crate::api::external_links::ExternalLinks::list`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternalLink {
    pub id: u64,
    #[serde(default)]
    pub uid: Option<String>,
    pub url: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape A: API upload (`type` 1) — numeric `id`, absolute public `url`.
    const CLASSIC: &str = r#"{
        "id": 61256602, "uid": "48c405aa-a7a3-455e-9752-f2c3225cfecb",
        "name": "probe-attach.txt", "size": 58, "type": 1, "mime_type": null,
        "url": "https://files.kaiten.ru/48c405aa-a7a3-455e-9752-f2c3225cfecb.txt",
        "external": false, "deleted": false, "author_id": 1068514,
        "created": "2026-07-16T19:00:00.000Z"
    }"#;

    /// Shape B (issue #12, newer storage, `type` 11): UUID in `id`, string
    /// `size`, host-root-relative `url` under /api/v1.
    const NEWER: &str = r#"{
        "id": "6a8e66af-0000-0000-0000-000000000000", "name": "report.xlsx",
        "size": "58818", "mime_type": "application/vnd.ms-excel",
        "author_uid": "51b4f5a0-0000-0000-0000-000000000000",
        "card_uid": "0ca503b2-0000-0000-0000-000000000000", "entity_type": "card",
        "created": "2026-08-10T07:26:08.653Z", "resizes": [], "card_cover": false,
        "deleted": false, "type": 11,
        "url": "/api/v1/cards/0ca503b2-0000-0000-0000-000000000000/files/6a8e66af-0000-0000-0000-000000000000",
        "card_id": 12345678
    }"#;

    fn file(json: &str) -> CardFile {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn card_file_classic_shape_keeps_numeric_id() {
        let f = file(CLASSIC);
        assert_eq!(f.id, 61_256_602);
        assert_eq!(f.size, Some(58));
        assert_eq!(f.file_type, Some(1));
        assert_eq!(f.name, "probe-attach.txt");
        assert!(
            f.url
                .as_deref()
                .unwrap()
                .starts_with("https://files.kaiten.ru/")
        );
    }

    #[test]
    fn card_file_uuid_id_becomes_zero_and_string_size_parses() {
        let f = file(NEWER);
        assert_eq!(f.id, 0, "a UUID cannot be a u64: documented sentinel");
        assert_eq!(f.size, Some(58_818));
        assert_eq!(f.file_type, Some(11));
        assert_eq!(f.mime_type.as_deref(), Some("application/vnd.ms-excel"));
        assert!(f.url.as_deref().unwrap().starts_with("/api/v1/cards/"));
    }

    #[test]
    fn card_file_numeric_string_id_parses() {
        let f = file(r#"{"id": "123", "name": "n"}"#);
        assert_eq!(f.id, 123);
    }

    #[test]
    fn card_file_size_null_missing_or_garbage_is_none() {
        assert_eq!(file(r#"{"id": 1, "name": "n", "size": null}"#).size, None);
        assert_eq!(file(r#"{"id": 1, "name": "n"}"#).size, None);
        assert_eq!(file(r#"{"id": 1, "name": "n", "size": "lots"}"#).size, None);
    }

    #[test]
    fn card_file_missing_id_is_error() {
        assert!(serde_json::from_str::<CardFile>(r#"{"name": "n"}"#).is_err());
    }

    /// Serialize output is part of the CLI's `--json` contract: no new keys,
    /// `file_type` still written as `type`.
    #[test]
    fn card_file_serializes_the_same_keys_as_before() {
        let value = serde_json::to_value(file(CLASSIC)).unwrap();
        let mut keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "author_id",
                "created",
                "deleted",
                "external",
                "id",
                "mime_type",
                "name",
                "size",
                "type",
                "url"
            ]
        );
        assert_eq!(value["type"], 1);
    }

    #[test]
    fn card_with_mixed_files_parses() {
        let json = format!(r#"{{"id": 1, "title": "t", "files": [{CLASSIC}, {NEWER}]}}"#);
        let card: Card = serde_json::from_str(&json).unwrap();
        assert_eq!(card.files.len(), 2);
        assert_eq!(card.files[0].id, 61_256_602);
        assert_eq!(card.files[1].id, 0);
    }

    #[test]
    fn file_ref_from_str_display_roundtrip() {
        assert_eq!("123".parse::<FileRef>().unwrap(), FileRef::Id(123));
        assert_eq!(" 42 ".parse::<FileRef>().unwrap(), FileRef::Id(42));
        let uid = "6a8e66af-0000-0000-0000-000000000000";
        assert_eq!(uid.parse::<FileRef>().unwrap(), FileRef::Uid(uid.into()));
        assert_eq!(FileRef::Id(123).to_string(), "123");
        assert_eq!(FileRef::Uid(uid.into()).to_string(), uid);
    }

    #[test]
    fn file_ref_from_card_file_uses_url_uuid_when_id_is_zero() {
        assert_eq!(FileRef::from(&file(CLASSIC)), FileRef::Id(61_256_602));
        assert_eq!(
            FileRef::from(&file(NEWER)),
            FileRef::Uid("6a8e66af-0000-0000-0000-000000000000".into())
        );
        let orphan = file(r#"{"id": "x-y", "name": "n"}"#);
        assert_eq!(FileRef::from(&orphan), FileRef::Id(0));
    }

    #[test]
    fn file_ref_matches_by_id_or_uid_and_zero_never_matches() {
        let classic = file(CLASSIC);
        let newer = file(NEWER);
        assert!(FileRef::Id(61_256_602).matches(&classic));
        assert!(!FileRef::Id(61_256_602).matches(&newer));
        assert!(FileRef::Uid("6A8E66AF-0000-0000-0000-000000000000".into()).matches(&newer));
        assert!(!FileRef::Uid("6a8e66af-0000-0000-0000-000000000000".into()).matches(&classic));
        // a classic url ends in `<uuid>.ext`: the uid is matched without the extension
        assert!(FileRef::Uid("48c405aa-a7a3-455e-9752-f2c3225cfecb".into()).matches(&classic));
        assert!(
            !FileRef::Id(0).matches(&newer),
            "0 is a sentinel, not an id"
        );
    }
}
