use serde::{Deserialize, Serialize};

/// Une ligne de la conversation affichée au joueur : soit ce qu'il a
/// lui-même écrit, soit la narration qui en a résulté. Distinct de
/// `EtatPartie::resumes` (narration seule, utilisée comme mémoire dans les
/// prompts et condensée au fil du temps) : `transcript` existe pour que la
/// reprise d'une histoire sauvegardée puisse réafficher la conversation
/// complète, des deux côtés, pas seulement ce que le narrateur a raconté.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LigneTranscript {
    Joueur(String),
    Narrateur(String),
}
