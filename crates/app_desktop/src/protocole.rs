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
    /// L'utilisateur a confirmé le téléchargement de la mise à jour
    /// annoncée par `MessageVersUi::MiseAJourDisponible` — `url_apk` est
    /// repris tel quel depuis cette annonce.
    TelechargerMiseAJour { url_apk: String },
    /// L'utilisateur a confirmé l'installation de la mise à jour déjà
    /// téléchargée à `chemin_apk` (voir `MessageVersUi::MiseAJourTelechargee`).
    InstallerMiseAJour { chemin_apk: String },
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
    /// Une version plus récente que `mise_a_jour::VERSION_ACTUELLE` est
    /// disponible sur le serveur LAN de mise à jour (vérifié au démarrage,
    /// Android seulement — voir `worker::lancer`).
    MiseAJourDisponible { version: String, url_apk: String },
    /// Progression (0.0 à 1.0) du téléchargement de l'APK de mise à jour,
    /// même convention que `ProgressionTelechargement`.
    ProgressionMiseAJour(f32),
    /// L'APK de mise à jour a été téléchargé avec succès à `chemin_apk`,
    /// prêt à être installé.
    MiseAJourTelechargee { chemin_apk: String },
    /// Le téléchargement de l'APK de mise à jour a échoué — message
    /// d'erreur déjà formaté, prêt à être affiché.
    MiseAJourEchouee(String),
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

/// État de la bannière de mise à jour, affichée au-dessus de n'importe
/// quel écran (`Vue`) plutôt qu'à l'intérieur de l'un d'eux — voir
/// `main.rs`, qui la garde dans son propre `Signal` à côté des autres
/// signaux de superposition (`de_en_attente`, etc.) plutôt que dans `Vue`.
#[derive(Debug, Clone, PartialEq)]
pub enum EtatBanniereMaj {
    Masquee,
    Disponible { version: String, url_apk: String },
    EnTelechargement { progression: f32 },
    PreteAInstaller { chemin_apk: String },
    Echouee(String),
}
