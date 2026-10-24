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

pub trait Narrateur: Send {
    fn raconter(
        &mut self,
        contexte: &ContexteTour,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ReponseMj, ErreurNarrateur>;
}
