use application::ports::narrateur::PnjDetecte;
use serde_json::Value;

#[derive(Debug, PartialEq)]
pub struct ErreurInterpretationPnj(pub String);

pub fn interpreter_json_detection_pnj(json_brut: &str) -> Result<Option<PnjDetecte>, ErreurInterpretationPnj> {
    let valeur: Value = serde_json::from_str(json_brut)
        .map_err(|e| ErreurInterpretationPnj(format!("JSON invalide : {e}")))?;

    let pnj = valeur
        .get("pnj")
        .ok_or_else(|| ErreurInterpretationPnj("champ \"pnj\" manquant".to_string()))?;

    if pnj.is_null() {
        return Ok(None);
    }

    let nom = pnj
        .as_str()
        .ok_or_else(|| ErreurInterpretationPnj("champ \"pnj\" doit être une chaîne ou null".to_string()))?
        .trim()
        .to_string();

    if nom.is_empty() {
        return Ok(None);
    }

    // La décision "nouveau ou déjà connu" appartient entièrement à
    // `application` (qui compare `nom` à `etat.pnj_connus`), pas à ce
    // parseur — voir la règle 5 du trait `Narrateur`. On extrait donc la
    // description si le modèle en a fourni une, sans condition sur un
    // éventuel champ "nouveau" dans sa réponse JSON (qui n'est plus lu du
    // tout : le faire dépendre de ce champ jetait la description quand le
    // modèle omettait "nouveau" ou répondait "nouveau": false, même en
    // présence d'une description valide).
    let nouvelle_description = valeur
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(str::to_string);

    Ok(Some(PnjDetecte { nom, nouvelle_description }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interprete_l_absence_de_pnj() {
        let resultat = interpreter_json_detection_pnj(r#"{"pnj": null}"#).unwrap();
        assert_eq!(resultat, None);
    }

    #[test]
    fn interprete_un_pnj_sans_description() {
        let resultat = interpreter_json_detection_pnj(r#"{"pnj": "Aldric"}"#).unwrap();
        assert_eq!(
            resultat,
            Some(PnjDetecte { nom: "Aldric".to_string(), nouvelle_description: None })
        );
    }

    #[test]
    fn interprete_un_pnj_avec_sa_description() {
        let resultat =
            interpreter_json_detection_pnj(r#"{"pnj": "Aldric", "description": "forgeron bourru"}"#).unwrap();
        assert_eq!(
            resultat,
            Some(PnjDetecte {
                nom: "Aldric".to_string(),
                nouvelle_description: Some("forgeron bourru".to_string())
            })
        );
    }

    #[test]
    fn un_champ_nouveau_a_false_n_empeche_plus_la_lecture_de_la_description() {
        let resultat = interpreter_json_detection_pnj(
            r#"{"pnj": "Aldric", "nouveau": false, "description": "forgeron bourru"}"#,
        )
        .unwrap();
        assert_eq!(
            resultat,
            Some(PnjDetecte {
                nom: "Aldric".to_string(),
                nouvelle_description: Some("forgeron bourru".to_string())
            })
        );
    }

    #[test]
    fn un_nom_vide_est_traite_comme_aucun_pnj() {
        let resultat = interpreter_json_detection_pnj(r#"{"pnj": ""}"#).unwrap();
        assert_eq!(resultat, None);
    }

    #[test]
    fn une_description_vide_est_traitee_comme_aucune_description() {
        let resultat = interpreter_json_detection_pnj(r#"{"pnj": "Aldric", "description": "   "}"#).unwrap();
        assert_eq!(
            resultat,
            Some(PnjDetecte { nom: "Aldric".to_string(), nouvelle_description: None })
        );
    }

    #[test]
    fn un_json_invalide_echoue() {
        let resultat = interpreter_json_detection_pnj("ceci n'est pas du json");
        assert!(resultat.is_err());
    }

    #[test]
    fn un_champ_pnj_manquant_echoue() {
        let resultat = interpreter_json_detection_pnj(r#"{"autre_chose": true}"#);
        assert!(resultat.is_err());
    }
}
