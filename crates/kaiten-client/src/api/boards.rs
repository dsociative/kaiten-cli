use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::Board;

/// Boards resource facade. Construct via [`KaitenClient::boards`].
pub struct Boards<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Boards<'_> {
    /// GET /spaces/{space_id}/boards
    pub async fn list(&self, space_id: u64) -> Result<Vec<Board>> {
        self.client
            .request(
                reqwest::Method::GET,
                &format!("/spaces/{space_id}/boards"),
                None,
                None,
            )
            .await
    }

    /// GET /boards/{board_id}
    pub async fn get(&self, board_id: u64) -> Result<Board> {
        self.client
            .request(
                reqwest::Method::GET,
                &format!("/boards/{board_id}"),
                None,
                None,
            )
            .await
    }
}
