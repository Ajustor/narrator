use crate::ports::generateur_de::GenerateurDe;
use domain::{Caracteristiques, De, Objet, Personnage, Pronom};

/// Bornes d'une caractéristique tirée via `tirer_valeurs_caracteristiques`
/// (méthode "4d6, on retire le plus petit") : minimum 1+1+1=3, maximum
/// 6+6+6=18. `creer_personnage` valide chaque valeur contre ces bornes
/// plutôt que contre un tableau fixe, puisque les valeurs sont désormais
/// tirées au hasard à chaque nouvelle histoire.
const CARACTERISTIQUE_MIN: u32 = 3;
const CARACTERISTIQUE_MAX: u32 = 18;

#[derive(Debug, PartialEq)]
pub enum ErreurCreationPersonnage {
    NomVide,
    ClasseVide,
    ValeursCaracteristiquesInvalides,
}

/// Tire les 6 valeurs de caractéristiques d'un nouveau personnage, méthode
/// "4d6, on retire le plus petit" (classique, favorise des valeurs plus
/// hautes qu'un simple 3d6) — une fois par caractéristique. Le joueur reste
/// libre d'assigner ces 6 valeurs tirées à la caractéristique de son choix
/// (voir `creer_personnage`), pour permettre de travailler un build malgré
/// le tirage aléatoire.
pub fn tirer_valeurs_caracteristiques(de: &mut dyn GenerateurDe) -> [u32; 6] {
    std::array::from_fn(|_| {
        let mut jets: [u32; 4] = std::array::from_fn(|_| de.lancer(De { faces: 6 }, 0).resultat);
        jets.sort_unstable();
        jets[1] + jets[2] + jets[3]
    })
}

pub fn creer_personnage(
    nom: &str,
    classe: &str,
    valeurs: [u32; 6],
    pronom: Pronom,
) -> Result<Personnage, ErreurCreationPersonnage> {
    if nom.trim().is_empty() {
        return Err(ErreurCreationPersonnage::NomVide);
    }
    if classe.trim().is_empty() {
        return Err(ErreurCreationPersonnage::ClasseVide);
    }
    if valeurs.iter().any(|v| *v < CARACTERISTIQUE_MIN || *v > CARACTERISTIQUE_MAX) {
        return Err(ErreurCreationPersonnage::ValeursCaracteristiquesInvalides);
    }

    let caracteristiques = Caracteristiques {
        force: valeurs[0],
        dexterite: valeurs[1],
        constitution: valeurs[2],
        intelligence: valeurs[3],
        sagesse: valeurs[4],
        charisme: valeurs[5],
    };

    let inventaire = vec![Objet::epee(), Objet::potion_de_soin()];

    Ok(Personnage::nouveau(
        nom.to_string(),
        classe.to_string(),
        caracteristiques,
        inventaire,
    )
    .avec_pronom(pronom))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::generateur_de::GenerateurDeSeed;

    const VALEURS_VALIDES: [u32; 6] = [15, 14, 13, 12, 10, 8];

    #[test]
    fn nom_vide_est_rejete() {
        let resultat = creer_personnage("  ", "Guerrier", VALEURS_VALIDES, Pronom::Masculin);
        assert_eq!(resultat, Err(ErreurCreationPersonnage::NomVide));
    }

    #[test]
    fn classe_vide_est_rejetee() {
        let resultat = creer_personnage("Elowen", "  ", VALEURS_VALIDES, Pronom::Masculin);
        assert_eq!(resultat, Err(ErreurCreationPersonnage::ClasseVide));
    }

    #[test]
    fn classe_libre_est_acceptee() {
        let resultat = creer_personnage("Elowen", "Barde", VALEURS_VALIDES, Pronom::Masculin);
        assert!(resultat.is_ok());
        assert_eq!(resultat.unwrap().classe, "Barde");
    }

    #[test]
    fn valeur_sous_le_minimum_est_rejetee() {
        let resultat = creer_personnage("Elowen", "Guerrier", [2, 14, 13, 12, 10, 8], Pronom::Masculin);
        assert_eq!(resultat, Err(ErreurCreationPersonnage::ValeursCaracteristiquesInvalides));
    }

    #[test]
    fn valeur_au_dessus_du_maximum_est_rejetee() {
        let resultat = creer_personnage("Elowen", "Guerrier", [19, 14, 13, 12, 10, 8], Pronom::Masculin);
        assert_eq!(resultat, Err(ErreurCreationPersonnage::ValeursCaracteristiquesInvalides));
    }

    #[test]
    fn creation_valide_construit_le_personnage_avec_equipement_de_depart() {
        let perso = creer_personnage("Elowen", "Guerrier", VALEURS_VALIDES, Pronom::Feminin).unwrap();
        assert_eq!(perso.nom, "Elowen");
        assert_eq!(perso.classe, "Guerrier");
        assert_eq!(perso.caracteristiques.force, 15);
        assert_eq!(perso.caracteristiques.charisme, 8);
        assert_eq!(perso.inventaire.len(), 2);
        assert_eq!(perso.pronom, Pronom::Feminin);
    }

    #[test]
    fn tirage_additionne_les_3_meilleurs_des_4d6_pour_chaque_caracteristique() {
        // 6 groupes de 4 jets ; le plus petit de chaque groupe est retiré.
        let jets = vec![
            1, 2, 3, 4, // groupe 1 : retire 1 -> 2+3+4 = 9
            6, 6, 6, 6, // groupe 2 : retire 6 -> 6+6+6 = 18
            1, 1, 1, 1, // groupe 3 : retire 1 -> 1+1+1 = 3
            5, 1, 5, 5, // groupe 4 : retire 1 -> 5+5+5 = 15
            3, 4, 2, 6, // groupe 5 : retire 2 -> 3+4+6 = 13
            2, 2, 3, 2, // groupe 6 : retire 2 -> 2+3+2 = 7
        ];
        let mut de = GenerateurDeSeed::nouveau(jets);
        let valeurs = tirer_valeurs_caracteristiques(&mut de);
        assert_eq!(valeurs, [9, 18, 3, 15, 13, 7]);
    }

    #[test]
    fn tirage_reste_dans_les_bornes_valides_par_construction() {
        let mut de = GenerateurDeSeed::nouveau(vec![1; 24]);
        let valeurs = tirer_valeurs_caracteristiques(&mut de);
        assert!(valeurs.iter().all(|v| *v >= CARACTERISTIQUE_MIN && *v <= CARACTERISTIQUE_MAX));
    }
}
