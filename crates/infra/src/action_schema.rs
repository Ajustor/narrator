use domain::{EffetObjet, EtatPartie};
use serde_json::Value;

/// Pas appelée en production : `LlamaNarrateur::proposer_action` n'utilise
/// plus la grammaire GBNF (crash connu et non résolu de llama.cpp, voir le
/// commentaire dans `llama_narrateur.rs`). Conservée et testée ici — y
/// compris sa conversion en grammaire, dans `llama_narrateur.rs` — pour une
/// future réactivation si ce bug est corrigé côté llama.cpp.
#[allow(dead_code)]
pub fn construire_schema_action(etat: &EtatPartie) -> Value {
    let ennemis_vivants: Vec<Value> = etat
        .combat_en_cours
        .as_ref()
        .filter(|c| !c.termine)
        .map(|c| {
            c.ennemis
                .iter()
                .filter(|e| !e.est_mort())
                .map(|e| Value::String(e.nom.clone()))
                .collect()
        })
        .unwrap_or_default();

    let objets_consommables: Vec<Value> = etat
        .personnage
        .inventaire
        .iter()
        .filter(|o| matches!(o.effet, Some(EffetObjet::Consommable { .. })))
        .map(|o| Value::String(o.nom.clone()))
        .collect();

    let combat_actif = etat.combat_en_cours.as_ref().map(|c| !c.termine).unwrap_or(false);

    let mut variantes = vec![serde_json::json!({
        "type": "object",
        "properties": {
            "action": {"const": "libre"}
        },
        "required": ["action"]
    })];

    if !ennemis_vivants.is_empty() {
        variantes.push(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {"const": "attaquer"},
                "cible": {"enum": ennemis_vivants}
            },
            "required": ["action", "cible"]
        }));
    }

    if !objets_consommables.is_empty() {
        variantes.push(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {"const": "utiliser_objet"},
                "objet": {"enum": objets_consommables}
            },
            "required": ["action", "objet"]
        }));
    }

    if combat_actif {
        variantes.push(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {"const": "fuir"}
            },
            "required": ["action"]
        }));
        variantes.push(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {"const": "esquiver"}
            },
            "required": ["action"]
        }));
    }

    serde_json::json!({ "oneOf": variantes })
}

#[derive(Debug, PartialEq)]
pub struct ErreurInterpretation(pub String);

pub fn interpreter_json_action(json_brut: &str) -> Result<domain::ActionJoueur, ErreurInterpretation> {
    use domain::ActionJoueur;

    let valeur: Value = serde_json::from_str(json_brut)
        .map_err(|e| ErreurInterpretation(format!("JSON invalide : {e}")))?;

    let action = valeur
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| ErreurInterpretation("champ \"action\" manquant".to_string()))?;

    match action {
        "attaquer" => {
            let cible = valeur
                .get("cible")
                .and_then(Value::as_str)
                .ok_or_else(|| ErreurInterpretation("champ \"cible\" manquant".to_string()))?;
            Ok(ActionJoueur::Attaquer { cible: cible.to_string() })
        }
        "utiliser_objet" => {
            let objet = valeur
                .get("objet")
                .and_then(Value::as_str)
                .ok_or_else(|| ErreurInterpretation("champ \"objet\" manquant".to_string()))?;
            Ok(ActionJoueur::UtiliserObjet { objet: objet.to_string() })
        }
        "fuir" => Ok(ActionJoueur::Fuir),
        "esquiver" => Ok(ActionJoueur::Esquiver),
        "libre" => {
            let texte = valeur.get("texte").and_then(Value::as_str).unwrap_or("").to_string();
            Ok(ActionJoueur::ActionLibre(texte))
        }
        autre => Err(ErreurInterpretation(format!("action inconnue : {autre}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{ActionJoueur, Caracteristiques, Combat, De, Ennemi, Objet, Personnage};

    fn personnage_test(inventaire: Vec<Objet>) -> Personnage {
        Personnage::nouveau(
            "Elowen".into(),
            "Guerrier".into(),
            Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
            inventaire,
        )
    }

    fn etat_sans_rien() -> EtatPartie {
        EtatPartie {
            personnage: personnage_test(vec![]),
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
            pnj_connus: vec![],
            rencontre_initiale_declenchee: true,
        }
    }

    #[test]
    fn sans_combat_ni_objet_seule_l_action_libre_est_possible() {
        let schema = construire_schema_action(&etat_sans_rien());
        let variantes = schema["oneOf"].as_array().unwrap();
        assert_eq!(variantes.len(), 1);
        assert_eq!(variantes[0]["properties"]["action"]["const"], "libre");
    }

    #[test]
    fn un_ennemi_vivant_ajoute_la_variante_attaquer_avec_son_nom() {
        let mut etat = etat_sans_rien();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0],
            joueur_esquive: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        let attaquer = variantes
            .iter()
            .find(|v| v["properties"]["action"]["const"] == "attaquer")
            .expect("la variante attaquer doit être présente");
        let cibles = attaquer["properties"]["cible"]["enum"].as_array().unwrap();
        assert_eq!(cibles, &vec![Value::String("Gobelin".to_string())]);
    }

    #[test]
    fn un_ennemi_mort_n_apparait_pas_dans_les_cibles() {
        let mut etat = etat_sans_rien();
        let mut gobelin = Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3);
        gobelin.subir_degats(100);
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![gobelin],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0],
            joueur_esquive: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        assert!(variantes.iter().all(|v| v["properties"]["action"]["const"] != "attaquer"));
    }

    #[test]
    fn un_objet_consommable_ajoute_la_variante_utiliser_objet() {
        let mut etat = etat_sans_rien();
        etat.personnage = personnage_test(vec![Objet::potion_de_soin()]);
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        let utiliser = variantes
            .iter()
            .find(|v| v["properties"]["action"]["const"] == "utiliser_objet")
            .expect("la variante utiliser_objet doit être présente");
        let objets = utiliser["properties"]["objet"]["enum"].as_array().unwrap();
        assert_eq!(objets, &vec![Value::String("Potion de soin".to_string())]);
    }

    #[test]
    fn une_arme_n_apparait_pas_dans_les_objets_utilisables() {
        let mut etat = etat_sans_rien();
        etat.personnage = personnage_test(vec![Objet::epee()]);
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        assert!(variantes.iter().all(|v| v["properties"]["action"]["const"] != "utiliser_objet"));
    }

    #[test]
    fn un_combat_actif_ajoute_la_variante_esquiver() {
        let mut etat = etat_sans_rien();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0],
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
            joueur_esquive: false,
        });
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        assert!(variantes.iter().any(|v| v["properties"]["action"]["const"] == "esquiver"));
    }

    #[test]
    fn un_combat_termine_n_ajoute_pas_la_variante_esquiver() {
        let mut etat = etat_sans_rien();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0],
            tour_actuel: 0,
            termine: true,
            vainqueur: Some(domain::Vainqueur::Joueur),
            joueur_esquive: false,
        });
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        assert!(variantes.iter().all(|v| v["properties"]["action"]["const"] != "esquiver"));
    }

    #[test]
    fn un_combat_actif_ajoute_la_variante_fuir() {
        let mut etat = etat_sans_rien();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0],
            joueur_esquive: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        assert!(variantes.iter().any(|v| v["properties"]["action"]["const"] == "fuir"));
    }

    #[test]
    fn un_combat_termine_n_ajoute_pas_la_variante_fuir() {
        let mut etat = etat_sans_rien();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0],
            joueur_esquive: false,
            tour_actuel: 0,
            termine: true,
            vainqueur: Some(domain::Vainqueur::Joueur),
        });
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        assert!(variantes.iter().all(|v| v["properties"]["action"]["const"] != "fuir"));
    }

    #[test]
    fn interprete_une_attaque() {
        let action = interpreter_json_action(r#"{"action":"attaquer","cible":"Gobelin"}"#).unwrap();
        assert_eq!(action, ActionJoueur::Attaquer { cible: "Gobelin".into() });
    }

    #[test]
    fn interprete_l_utilisation_d_un_objet() {
        let action = interpreter_json_action(r#"{"action":"utiliser_objet","objet":"Potion de soin"}"#).unwrap();
        assert_eq!(action, ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() });
    }

    #[test]
    fn interprete_une_fuite() {
        let action = interpreter_json_action(r#"{"action":"fuir"}"#).unwrap();
        assert_eq!(action, ActionJoueur::Fuir);
    }

    #[test]
    fn interprete_une_esquive() {
        let action = interpreter_json_action(r#"{"action":"esquiver"}"#).unwrap();
        assert_eq!(action, ActionJoueur::Esquiver);
    }

    #[test]
    fn interprete_une_action_libre() {
        let action = interpreter_json_action(r#"{"action":"libre","texte":"J'observe"}"#).unwrap();
        assert_eq!(action, ActionJoueur::ActionLibre("J'observe".into()));
    }

    #[test]
    fn un_json_invalide_echoue() {
        let resultat = interpreter_json_action("ceci n'est pas du json");
        assert!(resultat.is_err());
    }

    #[test]
    fn un_champ_action_manquant_echoue() {
        let resultat = interpreter_json_action(r#"{"cible":"Gobelin"}"#);
        assert!(resultat.is_err());
    }

    #[test]
    fn une_attaque_sans_cible_echoue() {
        let resultat = interpreter_json_action(r#"{"action":"attaquer"}"#);
        assert!(resultat.is_err());
    }

    #[test]
    fn une_action_inconnue_echoue() {
        let resultat = interpreter_json_action(r#"{"action":"voler"}"#);
        assert!(resultat.is_err());
    }
}
