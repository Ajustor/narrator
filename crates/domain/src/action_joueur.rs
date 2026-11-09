#[derive(Debug, Clone, PartialEq)]
pub enum ActionJoueur {
    Attaquer { cible: String },
    UtiliserObjet { objet: String },
    Fuir,
    /// Se mettre en garde : jusqu'à sa prochaine action, les attaques
    /// ennemies contre le joueur sont faites avec désavantage.
    Esquiver,
    ActionLibre(String),
}
