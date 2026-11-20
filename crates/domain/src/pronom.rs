/// Pronom à utiliser par le narrateur pour parler du personnage joueur.
/// Sans ce champ, rien n'indiquait au modèle quel pronom utiliser pour le
/// joueur : il le devinait à partir du prénom, séparément à chaque phrase
/// et à chaque appel, ce qui produisait des récits mélangeant "il" et
/// "elle" pour le même personnage (rapport utilisateur : le narrateur se
/// perd sur l'identité du joueur).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pronom {
    #[default]
    Masculin,
    Feminin,
}

impl Pronom {
    pub const TOUTES: [Pronom; 2] = [Pronom::Masculin, Pronom::Feminin];

    pub fn nom_affiche(&self) -> &'static str {
        match self {
            Pronom::Masculin => "Masculin (il)",
            Pronom::Feminin => "Féminin (elle)",
        }
    }

    pub fn sujet(&self) -> &'static str {
        match self {
            Pronom::Masculin => "il",
            Pronom::Feminin => "elle",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_pronom_a_un_nom_affiche_et_un_sujet_non_vides() {
        for pronom in Pronom::TOUTES {
            assert!(!pronom.nom_affiche().is_empty());
            assert!(!pronom.sujet().is_empty());
        }
    }

    #[test]
    fn le_pronom_par_defaut_est_masculin() {
        assert_eq!(Pronom::default(), Pronom::Masculin);
    }

    #[test]
    fn les_deux_pronoms_ont_un_sujet_different() {
        assert_ne!(Pronom::Masculin.sujet(), Pronom::Feminin.sujet());
    }
}
