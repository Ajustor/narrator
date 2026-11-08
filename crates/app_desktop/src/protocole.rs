use domain::{EtatPartie, Vainqueur};

pub use crate::histoires::InfoHistoire;
pub use crate::modeles::PaletteModele;

#[derive(Debug, Clone)]
pub enum MessageVersWorker {
    ReprendreHistoire(String),
    CreerPersonnage { nom: String, classe: String, valeurs: [u32; 6], palette_modele: PaletteModele },
    JouerTour(String),
    RetourSelection,
    ConfirmerTelechargement(bool),
}

#[derive(Debug, Clone)]
pub enum MessageVersUi {
    ListeHistoires(Vec<InfoHistoire>),
    PartieCreee(EtatPartie),
    NarrationFragment(String),
    TourTermine { etat: EtatPartie, resultat_narration: String, resume_mecanique: Vec<String>, vainqueur_combat: Option<Vainqueur>, partie_terminee: bool },
    TourEchoue(String),
    ProgressionTelechargement(f32),
    ConfirmationTelechargementRequise { palette: PaletteModele, taille_octets: u64 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Vue {
    SelectionHistoire(Vec<InfoHistoire>),
    Creation,
    Jeu,
    FinDePartie(Option<Vainqueur>),
    ErreurChargement(String),
    Telechargement { palette: PaletteModele, progression: f32 },
    ConfirmationTelechargement { palette: PaletteModele, taille_octets: u64 },
}
