use crate::modeles::PaletteModele;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InfoHistoire {
    pub id: String,
    pub nom_personnage: String,
    pub classe: String,
    pub palette_modele: PaletteModele,
    /// Nanosecondes depuis l'epoch Unix — sert uniquement à trier par
    /// ordre de création, pas affiché tel quel.
    pub cree_le: u128,
}

const NOM_INDEX: &str = "index.json";

/// Liste les histoires existantes, les plus récemment créées en premier.
/// Pas de répertoire/index = aucune histoire pour l'instant (premier
/// lancement) : retourne une liste vide, ce n'est pas une erreur.
pub fn lister(repertoire_sauvegardes: &Path) -> Vec<InfoHistoire> {
    let chemin_index = repertoire_sauvegardes.join(NOM_INDEX);
    let Ok(contenu) = std::fs::read_to_string(&chemin_index) else {
        return Vec::new(); // fichier absent : premier lancement, pas une erreur
    };
    let mut histoires: Vec<InfoHistoire> = match serde_json::from_str(&contenu) {
        Ok(histoires) => histoires,
        Err(_) => {
            // Index présent mais corrompu : ne pas le traiter comme "vide"
            // silencieusement, ce qui ferait perdre l'accès à toutes les
            // histoires existantes dès le prochain ajouter() (qui réécrirait
            // l'index avec la seule nouvelle entrée). On le renomme de côté
            // pour qu'un humain puisse le récupérer, et on continue avec une
            // liste vide pour cette fois (comportement visible plutôt que
            // perte silencieuse).
            let chemin_corrompu = chemin_index.with_extension("json.corrompu");
            let _ = std::fs::rename(&chemin_index, &chemin_corrompu);
            Vec::new()
        }
    };
    histoires.sort_by(|a, b| b.cree_le.cmp(&a.cree_le));
    histoires
}

/// Id interne (jamais affiché) : nom normalisé (minuscules, alphanumérique
/// uniquement) + suffixe temporel pour l'unicité, y compris entre deux
/// histoires du même nom.
pub fn generer_id(nom_personnage: &str) -> String {
    let normalise: String = nom_personnage
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    let horodatage = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{normalise}-{horodatage:x}")
}

pub fn ajouter(repertoire_sauvegardes: &Path, entree: InfoHistoire) -> std::io::Result<()> {
    std::fs::create_dir_all(repertoire_sauvegardes)?;
    let mut histoires = lister(repertoire_sauvegardes);
    histoires.push(entree);
    ecrire_index(repertoire_sauvegardes, &histoires)
}

pub fn retirer(repertoire_sauvegardes: &Path, id: &str) -> std::io::Result<()> {
    let mut histoires = lister(repertoire_sauvegardes);
    histoires.retain(|h| h.id != id);
    ecrire_index(repertoire_sauvegardes, &histoires)
}

/// Supprime une histoire entièrement : son fichier de sauvegarde ET son
/// entrée dans l'index — utilisé pour la suppression manuelle depuis
/// l'écran de sélection (le retrait automatique à la mort du personnage,
/// dans `worker::traiter_message`, fait les deux mêmes étapes séparément,
/// car il a déjà le chemin de sauvegarde sous la main).
///
/// Le fichier peut déjà être absent (entrée fantôme) — on ignore cette
/// erreur précise et on retire tout de même l'entrée de l'index. Pour
/// toute autre erreur de suppression du fichier, on s'arrête sans toucher
/// à l'index : mieux vaut une entrée qu'on peut retenter de supprimer
/// qu'une entrée fantôme pointant vers un fichier qui existe peut-être
/// encore.
pub fn supprimer(repertoire_sauvegardes: &Path, id: &str) -> std::io::Result<()> {
    let chemin = chemin_sauvegarde(repertoire_sauvegardes, id);
    if let Err(erreur) = std::fs::remove_file(&chemin) {
        if erreur.kind() != std::io::ErrorKind::NotFound {
            return Err(erreur);
        }
    }
    retirer(repertoire_sauvegardes, id)
}

/// Écrit l'index de façon atomique : contenu complet dans un `.tmp` puis
/// renommage vers `index.json`, même principe que `telechargement.rs`. Un
/// arrêt en plein milieu de l'écriture ne peut ainsi jamais laisser
/// `index.json` lui-même à moitié écrit (et donc illisible, ce qui ferait
/// croire à `lister` qu'il n'y a aucune histoire).
fn ecrire_index(repertoire_sauvegardes: &Path, histoires: &[InfoHistoire]) -> std::io::Result<()> {
    let chemin_index = repertoire_sauvegardes.join(NOM_INDEX);
    let chemin_temporaire = chemin_index.with_extension("json.tmp");
    let contenu = serde_json::to_string_pretty(histoires).expect("sérialisation de l'index");
    std::fs::write(&chemin_temporaire, contenu)?;
    std::fs::rename(&chemin_temporaire, &chemin_index)
}

pub fn chemin_sauvegarde(repertoire_sauvegardes: &Path, id: &str) -> PathBuf {
    repertoire_sauvegardes.join(format!("{id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeles::PaletteModele;
    use std::path::PathBuf;

    fn repertoire_temporaire(nom: &str) -> PathBuf {
        let chemin = std::env::temp_dir().join(format!(
            "app_desktop_histoires_test_{}_{}",
            std::process::id(),
            nom
        ));
        let _ = std::fs::remove_dir_all(&chemin);
        chemin
    }

    fn entree_test(id: &str, nom: &str) -> InfoHistoire {
        InfoHistoire {
            id: id.to_string(),
            nom_personnage: nom.to_string(),
            classe: "Guerrier".to_string(),
            palette_modele: PaletteModele::Equilibre,
            cree_le: 1,
        }
    }

    #[test]
    fn lister_sans_index_retourne_une_liste_vide() {
        let repertoire = repertoire_temporaire("vide");
        assert_eq!(lister(&repertoire), Vec::new());
    }

    #[test]
    fn ajouter_puis_lister_retrouve_l_entree() {
        let repertoire = repertoire_temporaire("ajout");
        ajouter(&repertoire, entree_test("elowen-abc123", "Elowen")).unwrap();
        let histoires = lister(&repertoire);
        assert_eq!(histoires.len(), 1);
        assert_eq!(histoires[0].id, "elowen-abc123");
        let _ = std::fs::remove_dir_all(&repertoire);
    }

    #[test]
    fn retirer_enleve_seulement_l_entree_visee() {
        let repertoire = repertoire_temporaire("retrait");
        ajouter(&repertoire, entree_test("elowen-abc123", "Elowen")).unwrap();
        ajouter(&repertoire, entree_test("thorn-def456", "Thorn")).unwrap();
        retirer(&repertoire, "elowen-abc123").unwrap();
        let histoires = lister(&repertoire);
        assert_eq!(histoires.len(), 1, "l'autre histoire ne doit pas disparaître");
        assert_eq!(histoires[0].id, "thorn-def456");
        let _ = std::fs::remove_dir_all(&repertoire);
    }

    #[test]
    fn supprimer_enleve_le_fichier_et_l_entree_d_index() {
        let repertoire = repertoire_temporaire("suppression");
        ajouter(&repertoire, entree_test("elowen-abc123", "Elowen")).unwrap();
        ajouter(&repertoire, entree_test("thorn-def456", "Thorn")).unwrap();
        std::fs::write(chemin_sauvegarde(&repertoire, "elowen-abc123"), "contenu factice").unwrap();

        supprimer(&repertoire, "elowen-abc123").unwrap();

        let histoires = lister(&repertoire);
        assert_eq!(histoires.len(), 1, "l'autre histoire ne doit pas disparaître");
        assert_eq!(histoires[0].id, "thorn-def456");
        assert!(
            !chemin_sauvegarde(&repertoire, "elowen-abc123").exists(),
            "le fichier de sauvegarde doit avoir été supprimé"
        );
        let _ = std::fs::remove_dir_all(&repertoire);
    }

    #[test]
    fn supprimer_retire_quand_meme_l_entree_d_index_si_le_fichier_est_deja_absent() {
        let repertoire = repertoire_temporaire("suppression_fantome");
        ajouter(&repertoire, entree_test("elowen-abc123", "Elowen")).unwrap();
        // Pas de fichier de sauvegarde écrit : entrée fantôme.

        supprimer(&repertoire, "elowen-abc123").unwrap();

        assert_eq!(lister(&repertoire), Vec::new());
        let _ = std::fs::remove_dir_all(&repertoire);
    }

    #[test]
    fn generer_id_reste_distinct_meme_pour_le_meme_nom() {
        let id1 = generer_id("Elowen");
        let id2 = generer_id("Elowen");
        assert_ne!(id1, id2, "même nom, deux appels : ids tout de même distincts");
        assert!(id1.starts_with("elowen-"));
    }

    /// Regression guard pour l'écriture atomique de l'index. Un arrêt brutal
    /// en pleine écriture n'est pas simulable de façon fiable dans un test
    /// unitaire : ce test vérifie seulement le chemin nominal (ajout puis
    /// retrait) — `index.json` doit toujours être un JSON complet et lisible,
    /// et le fichier `.tmp` intermédiaire ne doit pas subsister après le
    /// renommage.
    #[test]
    fn index_reste_un_json_complet_apres_ajout_et_retrait() {
        let repertoire = repertoire_temporaire("atomique");
        ajouter(&repertoire, entree_test("elowen-abc123", "Elowen")).unwrap();
        ajouter(&repertoire, entree_test("thorn-def456", "Thorn")).unwrap();
        retirer(&repertoire, "elowen-abc123").unwrap();

        let contenu = std::fs::read_to_string(repertoire.join("index.json")).unwrap();
        let attendu: Vec<InfoHistoire> = serde_json::from_str(&contenu)
            .expect("index.json doit être un JSON complet et valide");
        assert_eq!(attendu.len(), 1);
        assert_eq!(attendu[0].id, "thorn-def456");
        assert!(
            !repertoire.join("index.json.tmp").exists(),
            "le .tmp doit avoir été renommé vers index.json"
        );
        let _ = std::fs::remove_dir_all(&repertoire);
    }

    #[test]
    fn lister_avec_index_corrompu_renomme_le_fichier_et_retourne_une_liste_vide() {
        let repertoire = repertoire_temporaire("corrompu");
        std::fs::create_dir_all(&repertoire).unwrap();
        let chemin_index = repertoire.join(NOM_INDEX);
        std::fs::write(&chemin_index, "ceci n'est pas du json valide").unwrap();

        let histoires = lister(&repertoire);

        assert_eq!(histoires, Vec::new());
        assert!(
            !chemin_index.exists(),
            "l'index corrompu ne doit plus être à son emplacement d'origine"
        );
        assert!(
            repertoire.join("index.json.corrompu").exists(),
            "l'index corrompu doit être renommé en .json.corrompu"
        );
        let _ = std::fs::remove_dir_all(&repertoire);
    }

    #[test]
    fn chemin_sauvegarde_pointe_vers_id_json() {
        let repertoire = PathBuf::from("sauvegardes");
        assert_eq!(chemin_sauvegarde(&repertoire, "elowen-abc123"), repertoire.join("elowen-abc123.json"));
    }
}
