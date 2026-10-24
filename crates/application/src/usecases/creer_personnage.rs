use domain::{Caracteristiques, Objet, Personnage};

pub const CLASSES_VALIDES: [&str; 3] = ["Guerrier", "Mage", "Voleur"];
pub const VALEURS_STANDARD: [u32; 6] = [15, 14, 13, 12, 10, 8];

#[derive(Debug, PartialEq)]
pub enum ErreurCreationPersonnage {
    NomVide,
    ClasseInvalide,
    ValeursCaracteristiquesInvalides,
}

pub fn creer_personnage(
    nom: &str,
    classe: &str,
    valeurs: [u32; 6],
) -> Result<Personnage, ErreurCreationPersonnage> {
    if nom.trim().is_empty() {
        return Err(ErreurCreationPersonnage::NomVide);
    }
    if !CLASSES_VALIDES.contains(&classe) {
        return Err(ErreurCreationPersonnage::ClasseInvalide);
    }

    let mut triees = valeurs;
    triees.sort_unstable();
    let mut standard_triees = VALEURS_STANDARD;
    standard_triees.sort_unstable();
    if triees != standard_triees {
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
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALEURS_VALIDES: [u32; 6] = [15, 14, 13, 12, 10, 8];

    #[test]
    fn nom_vide_est_rejete() {
        let resultat = creer_personnage("  ", "Guerrier", VALEURS_VALIDES);
        assert_eq!(resultat, Err(ErreurCreationPersonnage::NomVide));
    }

    #[test]
    fn classe_invalide_est_rejetee() {
        let resultat = creer_personnage("Elowen", "Barde", VALEURS_VALIDES);
        assert_eq!(resultat, Err(ErreurCreationPersonnage::ClasseInvalide));
    }

    #[test]
    fn valeurs_hors_tableau_standard_sont_rejetees() {
        let resultat = creer_personnage("Elowen", "Guerrier", [16, 14, 13, 12, 10, 8]);
        assert_eq!(resultat, Err(ErreurCreationPersonnage::ValeursCaracteristiquesInvalides));
    }

    #[test]
    fn creation_valide_construit_le_personnage_avec_equipement_de_depart() {
        let perso = creer_personnage("Elowen", "Guerrier", VALEURS_VALIDES).unwrap();
        assert_eq!(perso.nom, "Elowen");
        assert_eq!(perso.classe, "Guerrier");
        assert_eq!(perso.caracteristiques.force, 15);
        assert_eq!(perso.caracteristiques.charisme, 8);
        assert_eq!(perso.inventaire.len(), 2);
    }
}
