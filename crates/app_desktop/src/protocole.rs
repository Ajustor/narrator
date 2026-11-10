use domain::{EtatPartie, Genre, Vainqueur};

pub use crate::histoires::InfoHistoire;
pub use crate::modeles::PaletteModele;

#[derive(Debug, Clone)]
pub enum MessageVersWorker {
    ReprendreHistoire(String),
    SupprimerHistoire(String),
    DemarrerCreationPersonnage,
    CreerPersonnage {
        nom: String,
        classe: String,
        valeurs: [u32; 6],
        palette_modele: PaletteModele,
        genre: Genre,
        premisse_libre: String,
    },
    JouerTour(String),
    /// Le joueur a touché le dé virtuel affiché à l'écran — déclenche le
    /// lancer (voir `MessageVersUi::DeDemande`), ne porte aucun résultat :
    /// le vrai tirage reste du côté du worker (`worker::GenerateurDeJoueur`),
    /// pas choisi par l'UI.
    DeLancee,
    RetourSelection,
    ConfirmerTelechargement(bool),
}

#[derive(Debug, Clone)]
pub enum MessageVersUi {
    ListeHistoires(Vec<InfoHistoire>),
    ValeursProposees([u32; 6]),
    PartieCreee(EtatPartie),
    IntroductionRacontee(String),
    NarrationFragment(String),
    TourTermine { etat: EtatPartie, resultat_narration: String, resume_mecanique: Vec<String>, vainqueur_combat: Option<Vainqueur>, partie_terminee: bool },
    TourEchoue(String),
    ProgressionTelechargement(f32),
    ConfirmationTelechargementRequise { palette: PaletteModele, taille_octets: u64 },
    /// Un jet du JOUEUR (attaque ou dégâts) est requis avant que le tour
    /// puisse continuer — `etiquette` dit lequel ("Jet d'attaque"/"Jet de
    /// dégâts"), `faces` le type de dé à afficher. Les jets des ennemis et
    /// de la rencontre initiale restent automatiques et n'envoient jamais
    /// ce message.
    DeDemande { faces: u32, etiquette: String },
    /// Résultat réel du dé demandé ci-dessus, envoyé dès le lancer (avant
    /// la narration complète du tour) pour un retour immédiat sur le dé
    /// affiché.
    DeResultat(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Vue {
    SelectionHistoire(Vec<InfoHistoire>),
    Creation([u32; 6]),
    Jeu,
    FinDePartie(Option<Vainqueur>),
    ErreurChargement(String),
    Telechargement { palette: PaletteModele, progression: f32 },
    ConfirmationTelechargement { palette: PaletteModele, taille_octets: u64 },
}
