use domain::{ActionJoueur, EtatPartie};

#[derive(Debug, Clone)]
pub struct ContexteTour {
    pub etat: EtatPartie,
    pub saisie_joueur: String,
}

#[derive(Debug, Clone)]
pub struct ReponseMj {
    pub narration: String,
    pub action_proposee: ActionJoueur,
}

#[derive(Debug)]
pub enum ErreurNarrateur {
    EchecGeneration(String),
}

/// Contract for any implementation of this port, including a future real
/// (LLM-backed) one — today only `infra::FauxNarrateur` implements it.
///
/// 1. An implementation must NEVER resolve dice, apply damage, or decide any
///    numeric game outcome. It only proposes an `ActionJoueur` (interpreted
///    from the player's free text) and narration text; `application`'s use
///    cases validate and resolve everything mechanical afterward.
/// 2. `raconter`'s `sur_fragment` callback must be invoked with pieces of
///    text that, concatenated in call order, reconstruct the returned
///    `narration` string exactly.
/// 3. A real LLM-backed implementation is responsible for extracting/parsing
///    the proposed action from whatever structured format the model outputs
///    (e.g. a JSON block in its response) — that parsing is the
///    implementation's responsibility, not something `application` does for it.
pub trait Narrateur: Send {
    /// Proposes an action and narration for the player's free-text input.
    /// See the trait-level docs above for the full contract an implementation
    /// (including a future real LLM-backed one) must honor.
    fn raconter(
        &mut self,
        contexte: &ContexteTour,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ReponseMj, ErreurNarrateur>;
}
