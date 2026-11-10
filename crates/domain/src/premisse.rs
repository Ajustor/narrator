use crate::genre::Genre;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PremisseHistoire {
    pub genre: Genre,
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premisse_par_defaut_est_fantasy_sans_description() {
        let p = PremisseHistoire::default();
        assert_eq!(p.genre, Genre::FantasyMedievale);
        assert_eq!(p.description, "");
    }
}
