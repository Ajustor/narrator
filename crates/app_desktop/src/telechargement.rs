use std::io::{Read, Write};
use std::path::Path;

pub const MODELE_URL: &str =
    "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf";

#[derive(Debug)]
pub enum ErreurTelechargement {
    Reseau(String),
    Ecriture(String),
}

/// Décision pure : le fichier modèle est-il déjà présent à l'emplacement
/// attendu ? Sur desktop, ce chemin pointe déjà vers un fichier existant
/// (téléchargé par `scripts/jouer.sh` avant le lancement) — cette fonction
/// retourne donc `false` et le téléchargement ne se déclenche jamais en
/// usage normal sur desktop.
pub fn faut_il_telecharger(chemin_modele: &Path) -> bool {
    !chemin_modele.exists()
}

/// Télécharge le modèle vers `chemin_destination`, en passant la
/// progression (0.0 à 1.0) à `sur_progression` au fil du flux. Écrit dans
/// un fichier `.tmp` puis renomme de façon atomique vers la destination
/// finale une fois le téléchargement terminé — même principe que
/// `scripts/jouer.sh`, pour ne jamais laisser un fichier partiel au nom
/// final si le téléchargement est interrompu.
pub fn telecharger_modele(
    chemin_destination: &Path,
    sur_progression: &mut dyn FnMut(f32),
) -> Result<(), ErreurTelechargement> {
    let reponse = ureq::get(MODELE_URL)
        .call()
        .map_err(|e| ErreurTelechargement::Reseau(e.to_string()))?;

    let taille_totale: u64 = reponse
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let chemin_temporaire = chemin_destination.with_extension("tmp");
    if let Some(parent) = chemin_temporaire.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ErreurTelechargement::Ecriture(e.to_string()))?;
    }
    let mut fichier = std::fs::File::create(&chemin_temporaire)
        .map_err(|e| ErreurTelechargement::Ecriture(e.to_string()))?;

    let mut lecteur = reponse.into_reader();
    let mut tampon = [0u8; 65536];
    let mut telecharge: u64 = 0;
    loop {
        let n = lecteur
            .read(&mut tampon)
            .map_err(|e| ErreurTelechargement::Reseau(e.to_string()))?;
        if n == 0 {
            break;
        }
        fichier
            .write_all(&tampon[..n])
            .map_err(|e| ErreurTelechargement::Ecriture(e.to_string()))?;
        telecharge += n as u64;
        if taille_totale > 0 {
            sur_progression(telecharge as f32 / taille_totale as f32);
        }
    }

    std::fs::rename(&chemin_temporaire, chemin_destination)
        .map_err(|e| ErreurTelechargement::Ecriture(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faut_il_telecharger_vrai_si_fichier_absent() {
        let chemin = std::env::temp_dir().join(format!(
            "app_desktop_test_absent_{}.gguf",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&chemin);
        assert!(faut_il_telecharger(&chemin));
    }

    #[test]
    fn faut_il_telecharger_faux_si_fichier_present() {
        let chemin = std::env::temp_dir().join(format!(
            "app_desktop_test_present_{}.gguf",
            std::process::id()
        ));
        std::fs::write(&chemin, b"contenu factice").expect("ecriture test");
        assert!(!faut_il_telecharger(&chemin));
        let _ = std::fs::remove_file(&chemin);
    }
}
