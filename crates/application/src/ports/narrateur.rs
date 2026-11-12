use domain::{ActionJoueur, EtatPartie, Genre, Personnage, PremisseHistoire};

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
    /// Saisie textuelle brute du joueur, verbatim — `action` seul ne la
    /// garde que pour `ActionJoueur::ActionLibre` (son champ `texte`) ;
    /// pour toute autre variante (`Attaquer`, `UtiliserObjet`, `Fuir`,
    /// `Esquiver`, `ProvoquerCombat`), la classification ne conserve que
    /// des champs structurés (cible, objet...) et jette la formulation du
    /// joueur. Sans ce champ, `construire_prompt_narration` (dans
    /// `infra`) ne peut donc raconter qu'une version générique de
    /// l'action du joueur dès qu'elle est classifiée dans l'une de ces
    /// variantes — la cause probable du rapport « mes actions ne sont pas
    /// prises en compte » pour toute action un peu détaillée.
    pub saisie_joueur: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PnjDetecte {
    pub nom: String,
    pub nouvelle_description: Option<String>,
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
/// 4. `condenser_resumes` must behave as a pure text-compression step: it
///    never inspects or mutates `EtatPartie`, and the decision of which
///    résumés are "old enough" to send to it is entirely `application`'s
///    responsibility, not the implementation's.
/// 5. `detecter_pnj` must behave as a pure detection/extraction step over
///    the narration text it's given: it never inspects or mutates
///    `EtatPartie` directly, and the decision of whether a PNJ is already
///    known (by matching `noms_connus`) versus new is `application`'s to
///    act on, not the implementation's to enforce.
/// 6. `raconter_introduction` sets the scene for a brand-new character
///    before anything mechanical happens (no combat, no dice, nothing to
///    report) — it is streamed like `raconter_resultat`, but never takes a
///    `ContexteResultat` (there is no resolved action yet to narrate).
/// 7. `generer_lieu_initial` is only ever called when the player wrote a
///    free-text premise — `Genre::lieu_initial` already covers the
///    genre-only case, deterministically, with no model call. It never
///    inspects or mutates `EtatPartie`; deciding whether the premise is
///    empty, and falling back to `Genre::lieu_initial` on failure, is
///    `app_desktop`'s responsibility, not the implementation's.
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

    /// Compresses older turn-résumés (oldest to newest) into a single
    /// shorter string, replacing them. Never shown to the player, no
    /// streaming — same as `proposer_action`.
    fn condenser_resumes(&mut self, anciens: &[String]) -> Result<String, ErreurNarrateur>;

    /// Detects whether a PNJ (new or already known) appeared in the
    /// narration text just produced. `noms_connus` lets the model match a
    /// returning PNJ instead of inventing a duplicate. Never shown to the
    /// player, no streaming.
    fn detecter_pnj(
        &mut self,
        narration: &str,
        noms_connus: &[String],
    ) -> Result<Option<PnjDetecte>, ErreurNarrateur>;

    /// Narrates the opening scene for a freshly created character, before
    /// any turn is played — sets up the world/situation instead of
    /// dropping the player straight into whatever comes first. Streamed to
    /// the player via `sur_fragment`, same contract as `raconter_resultat`.
    fn raconter_introduction(
        &mut self,
        personnage: &Personnage,
        lieu: &str,
        premisse: &PremisseHistoire,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, ErreurNarrateur>;

    /// Generates a short starting-location phrase consistent with the
    /// player's freeform premise (and genre) — used only when that
    /// premise is non-empty, in place of the generic, genre-only
    /// `Genre::lieu_initial`. Never shown to the player as-is; no
    /// streaming.
    fn generer_lieu_initial(
        &mut self,
        genre: Genre,
        premisse_libre: &str,
    ) -> Result<String, ErreurNarrateur>;
}
