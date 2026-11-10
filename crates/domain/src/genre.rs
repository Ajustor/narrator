#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Genre {
    #[default]
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

    /// Lieu de départ cohérent avec le genre — utilisé à la création d'une
    /// nouvelle histoire. Avant l'ajout du genre, ce lieu était toujours
    /// "Clairière" (un nom de fantasy médiévale) quel que soit le genre
    /// choisi, ce qui n'avait aucun sens pour la science-fiction ou
    /// l'horreur par exemple.
    pub fn lieu_initial(&self) -> &'static str {
        match self {
            Genre::FantasyMedievale => "une clairière, à l'orée d'une forêt ancienne",
            Genre::ScienceFiction => "un poste avancé orbital, en vue d'une planète inconnue",
            Genre::Horreur => "un hameau brumeux, aux volets tous clos",
            Genre::PolarNoir => "une ruelle détrempée, sous le néon d'une enseigne défaillante",
            Genre::PostApocalyptique => "les ruines d'une station-service, le long d'une route craquelée",
            Genre::Steampunk => "un terminal de dirigeables, entre vapeur et rouages",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_genre_a_un_nom_affiche_une_indication_et_un_lieu_non_vides() {
        for genre in Genre::TOUTES {
            assert!(!genre.nom_affiche().is_empty());
            assert!(!genre.indication_prompt().is_empty());
            assert!(!genre.lieu_initial().is_empty());
        }
    }

    #[test]
    fn le_lieu_initial_varie_selon_le_genre() {
        assert_ne!(Genre::FantasyMedievale.lieu_initial(), Genre::ScienceFiction.lieu_initial());
    }

    #[test]
    fn le_genre_par_defaut_est_fantasy_medievale() {
        assert_eq!(Genre::default(), Genre::FantasyMedievale);
    }
}
