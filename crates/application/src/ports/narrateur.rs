use domain::{ActionJoueur, EtatPartie};

#[derive(Debug, Clone)]
pub struct ContexteTour {
    pub etat: EtatPartie,
    pub saisie_joueur: String,
}

#[derive(Debug, Clone)]
pub struct ContexteResultat {
    pub etat: EtatPartie,
    pub action: ActionJoueur,
    pub resume_mecanique: Vec<String>,
}

#[derive(Debug)]
pub enum ErreurNarrateur {
    EchecGeneration(String),
}

/// Contract for any implementation of this port, including the real
/// (LLM-backed) one in `infra::LlamaNarrateur` — `infra::FauxNarrateur` is
/// the test double.
///
/// 1. `proposer_action` must NEVER resolve dice, apply damage, or decide any
///    numeric game outcome — it only proposes an `ActionJoueur` interpreted
///    from the player's free text. `application`'s use cases validate and
///    resolve everything mechanical afterward, and only then is
///    `raconter_resultat` called with the real, already-resolved outcome.
/// 2. `raconter_resultat`'s `sur_fragment` callback must be invoked with
///    pieces of text that, concatenated in call order, reconstruct the
///    returned narration string exactly.
/// 3. A real LLM-backed implementation is responsible for extracting/parsing
///    the proposed action from whatever structured format the model
///    outputs (e.g. a JSON block) — that parsing is the implementation's
///    responsibility, not something `application` does for it.
pub trait Narrateur: Send {
    /// Proposes an action for the player's free-text input. No narration,
    /// no streaming — this result is never shown to the player directly.
    fn proposer_action(
        &mut self,
        contexte: &ContexteTour,
    ) -> Result<ActionJoueur, ErreurNarrateur>;

    /// Narrates the real outcome of an action already resolved by the
    /// engine (see `ContexteResultat`). Streamed to the player via
    /// `sur_fragment`.
    fn raconter_resultat(
        &mut self,
        contexte: &ContexteResultat,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, ErreurNarrateur>;
}
