use domain::{EtatPartie, Vainqueur};

#[derive(Debug, Clone)]
pub enum MessageVersWorker {
    CreerPersonnage { nom: String, classe: String, valeurs: [u32; 6] },
    JouerTour(String),
    NouvellePartie,
}

#[derive(Debug, Clone)]
pub enum MessageVersUi {
    EtatInitial(Option<EtatPartie>),
    PartieCreee(EtatPartie),
    NarrationFragment(String),
    TourTermine { etat: EtatPartie, resultat_narration: String, resume_mecanique: Vec<String>, combat_termine: bool },
    TourEchoue(String),
    PartieReinitialisee,
    ProgressionTelechargement(f32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Vue {
    Creation,
    Jeu,
    FinDePartie(Option<Vainqueur>),
    ErreurChargement(String),
    Telechargement(f32),
}
