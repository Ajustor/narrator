use crate::ennemi::Difficulte;

#[derive(Debug, Clone, PartialEq)]
pub enum ActionJoueur {
    Attaquer { cible: String },
    UtiliserObjet { objet: String },
    Fuir,
    /// Se mettre en garde : jusqu'à sa prochaine action, les attaques
    /// ennemies contre le joueur sont faites avec désavantage.
    Esquiver,
    ActionLibre { texte: String, agressif: bool },
    /// Déclenche un combat dynamique : soit parce que l'action du joueur
    /// est agressive envers une cible nommée, soit parce que le
    /// narrateur juge qu'une rencontre hostile doit survenir. Si un
    /// combat est déjà en cours, `resoudre_action_joueur` retombe sur le
    /// comportement de `Attaquer { cible: ennemi }` plutôt que de créer
    /// un second combat.
    ProvoquerCombat { ennemi: String, difficulte: Difficulte },
}
