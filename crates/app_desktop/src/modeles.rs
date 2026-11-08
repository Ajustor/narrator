#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteModele {
    Leger,
    Equilibre,
    Costaud,
}

impl PaletteModele {
    pub const TOUTES: [PaletteModele; 3] = [PaletteModele::Leger, PaletteModele::Equilibre, PaletteModele::Costaud];

    pub fn nom_affiche(&self) -> &'static str {
        match self {
            PaletteModele::Leger => "Léger",
            PaletteModele::Equilibre => "Équilibré",
            PaletteModele::Costaud => "Costaud",
        }
    }

    pub fn nom_fichier(&self) -> &'static str {
        match self {
            PaletteModele::Leger => "Qwen2.5-1.5B-Instruct-Q4_K_M.gguf",
            PaletteModele::Equilibre => "Qwen2.5-3B-Instruct-Q4_K_M.gguf",
            PaletteModele::Costaud => "Qwen2.5-7B-Instruct-Q4_K_M.gguf",
        }
    }

    pub fn url(&self) -> &'static str {
        match self {
            PaletteModele::Leger => "https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf",
            PaletteModele::Equilibre => "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf",
            PaletteModele::Costaud => "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf",
        }
    }

    pub fn taille_approx_octets(&self) -> u64 {
        match self {
            PaletteModele::Leger => 986_048_768,
            PaletteModele::Equilibre => 1_929_903_264,
            PaletteModele::Costaud => 4_683_074_240,
        }
    }

    pub fn avertissement(&self) -> Option<&'static str> {
        match self {
            PaletteModele::Costaud => Some("Plus lent et plus gourmand en mémoire, surtout sur mobile."),
            _ => None,
        }
    }

    pub fn chemin_dans(&self, repertoire_modeles: &std::path::Path) -> std::path::PathBuf {
        repertoire_modeles.join(self.nom_fichier())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_palette_a_un_nom_affiche_non_vide() {
        for palette in PaletteModele::TOUTES {
            assert!(!palette.nom_affiche().is_empty());
        }
    }

    #[test]
    fn chaque_palette_a_une_url_huggingface_qui_pointe_vers_son_fichier() {
        for palette in PaletteModele::TOUTES {
            assert!(palette.url().starts_with("https://huggingface.co/"));
            assert!(palette.nom_fichier().ends_with(".gguf"));
            assert!(palette.url().ends_with(palette.nom_fichier()));
        }
    }

    #[test]
    fn seul_le_palier_costaud_a_un_avertissement() {
        assert_eq!(PaletteModele::Leger.avertissement(), None);
        assert_eq!(PaletteModele::Equilibre.avertissement(), None);
        assert!(PaletteModele::Costaud.avertissement().is_some());
    }

    #[test]
    fn la_palette_equilibre_reprend_exactement_le_modele_actuel() {
        assert_eq!(
            PaletteModele::Equilibre.url(),
            "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf"
        );
        assert_eq!(PaletteModele::Equilibre.taille_approx_octets(), 1_929_903_264);
    }

    #[test]
    fn chemin_dans_joint_le_repertoire_et_le_nom_de_fichier() {
        let repertoire = std::path::PathBuf::from("modeles");
        assert_eq!(
            PaletteModele::Equilibre.chemin_dans(&repertoire),
            repertoire.join("Qwen2.5-3B-Instruct-Q4_K_M.gguf")
        );
    }

    #[test]
    fn deux_palettes_differentes_partagent_le_meme_repertoire_mais_pas_le_meme_fichier() {
        let repertoire = std::path::PathBuf::from("modeles");
        assert_ne!(
            PaletteModele::Leger.chemin_dans(&repertoire),
            PaletteModele::Costaud.chemin_dans(&repertoire)
        );
    }
}
