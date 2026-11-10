#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Genre {
    FantasyMedievale,
    ScienceFiction,
    Horreur,
    PolarNoir,
    PostApocalyptique,
    Steampunk,
}

impl Genre {
    pub const TOUTES: [Genre; 6] = [
        Genre::FantasyMedievale,
        Genre::ScienceFiction,
        Genre::Horreur,
        Genre::PolarNoir,
        Genre::PostApocalyptique,
        Genre::Steampunk,
    ];

    pub fn nom_affiche(&self) -> &'static str {
        match self {
            Genre::FantasyMedievale => "Fantasy médiévale",
            Genre::ScienceFiction => "Science-fiction",
            Genre::Horreur => "Horreur",
            Genre::PolarNoir => "Polar noir",
            Genre::PostApocalyptique => "Post-apocalyptique",
            Genre::Steampunk => "Steampunk",
        }
    }

    pub fn indication_prompt(&self) -> &'static str {
        match self {
            Genre::FantasyMedievale => "un monde de fantasy médiévale, avec magie, royaumes et créatures légendaires",
            Genre::ScienceFiction => "un futur de vaisseaux spatiaux et de technologies avancées",
            Genre::Horreur => "une ambiance d'horreur oppressante, pleine de menaces et de malaise",
            Genre::PolarNoir => "une enquête policière sombre dans une ville nocturne et corrompue",
            Genre::PostApocalyptique => "un monde ravagé où la civilisation s'est effondrée et où les ressources sont rares",
            Genre::Steampunk => "un monde rétro-futuriste de machines à vapeur et d'inventions mécaniques",
        }
    }
}

impl Default for Genre {
    fn default() -> Self {
        Genre::FantasyMedievale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_genre_a_un_nom_affiche_et_une_indication_non_vides() {
        for genre in Genre::TOUTES {
            assert!(!genre.nom_affiche().is_empty());
            assert!(!genre.indication_prompt().is_empty());
        }
    }

    #[test]
    fn le_genre_par_defaut_est_fantasy_medievale() {
        assert_eq!(Genre::default(), Genre::FantasyMedievale);
    }
}
