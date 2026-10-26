// ResultatTour n'est pas encore utilisé : TourTermine ci-dessous reprend ses
// champs directement plutôt que d'embarquer la struct (voir note en bas de
// fichier). L'import reste pour signaler le lien ; il sera consommé par la
// tâche qui convertira un ResultatTour en MessageVersUi::TourTermine.
#[allow(unused_imports)]
use application::usecases::jouer_tour::ResultatTour;
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum Vue {
    Creation,
    Jeu,
    FinDePartie(Option<Vainqueur>),
    ErreurChargement(String),
}
