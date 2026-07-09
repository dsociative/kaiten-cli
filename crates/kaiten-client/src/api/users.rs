use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::User;

/// Users resource facade. Construct via [`KaitenClient::users`].
pub struct Users<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Users<'_> {
    /// GET /users/current
    pub async fn current(&self) -> Result<User> {
        self.client
            .request(reqwest::Method::GET, "/users/current", None, None)
            .await
    }

    /// GET /users
    pub async fn list(&self) -> Result<Vec<User>> {
        self.client
            .request(reqwest::Method::GET, "/users", None, None)
            .await
    }
}
