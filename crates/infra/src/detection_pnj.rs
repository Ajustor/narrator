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
        .to_string();

    let nouveau = valeur.get("nouveau").and_then(Value::as_bool).unwrap_or(false);

    let nouvelle_description = if nouveau {
        Some(
            valeur
                .get("description")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    ErreurInterpretationPnj("champ \"description\" manquant pour un nouveau PNJ".to_string())
                })?
                .to_string(),
        )
    } else {
        None
    };

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
    fn interprete_un_pnj_deja_connu() {
        let resultat = interpreter_json_detection_pnj(r#"{"pnj": "Aldric", "nouveau": false}"#).unwrap();
        assert_eq!(
            resultat,
            Some(PnjDetecte { nom: "Aldric".to_string(), nouvelle_description: None })
        );
    }

    #[test]
    fn interprete_un_nouveau_pnj_avec_sa_description() {
        let resultat =
            interpreter_json_detection_pnj(r#"{"pnj": "Aldric", "nouveau": true, "description": "forgeron bourru"}"#)
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
    fn un_nouveau_pnj_sans_description_echoue() {
        let resultat = interpreter_json_detection_pnj(r#"{"pnj": "Aldric", "nouveau": true}"#);
        assert!(resultat.is_err());
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
