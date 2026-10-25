use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FichePnj {
    pub nom: String,
    pub description: String,
    pub historique: Vec<String>,
}
