use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct InitHookParams {
    pub rush_version: String,
}
