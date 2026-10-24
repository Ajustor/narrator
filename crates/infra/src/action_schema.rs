use domain::{EffetObjet, EtatPartie};
use serde_json::Value;

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
            "action": {"const": "libre"},
            "texte": {"type": "string"}
        },
        "required": ["action", "texte"]
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
    }

    serde_json::json!({ "oneOf": variantes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Combat, De, Ennemi, Objet, Personnage};

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
            ennemi_agit_avant_joueur: false,
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
            ennemi_agit_avant_joueur: false,
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
    fn un_combat_actif_ajoute_la_variante_fuir() {
        let mut etat = etat_sans_rien();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            ennemi_agit_avant_joueur: false,
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
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: true,
            vainqueur: Some(domain::Vainqueur::Joueur),
        });
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        assert!(variantes.iter().all(|v| v["properties"]["action"]["const"] != "fuir"));
    }
}
