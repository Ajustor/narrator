#[derive(Debug, Clone, PartialEq)]
pub enum ActionJoueur {
    Attaquer { cible: String },
    UtiliserObjet { objet: String },
    Fuir,
    ActionLibre(String),
}
