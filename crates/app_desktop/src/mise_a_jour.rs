use serde::Deserialize;

/// Version compilée dans le binaire : le hash court du commit git au
/// moment du build, gravé par `build.rs` via la variable d'environnement
/// de compilation `NARRATEUR_GIT_HASH` (jamais vide : `build.rs` retombe
/// sur `"inconnu"` si `git` est indisponible).
pub const VERSION_ACTUELLE: &str = env!("NARRATEUR_GIT_HASH");

/// Adresse par défaut du serveur de développement local servant
/// `version.json` et l'APK à jour sur le réseau local (LAN).
pub const ADRESSE_MAJ_PAR_DEFAUT: &str = "192.168.1.48:8000";

/// Information de mise à jour renvoyée par le serveur LAN lorsqu'une
/// version différente de `VERSION_ACTUELLE` est disponible.
#[derive(Debug, Clone, PartialEq)]
pub struct InfoMiseAJour {
    pub version: String,
    pub url_apk: String,
}

#[derive(Debug, Deserialize)]
struct ReponseVersion {
    version: String,
    url: String,
}

/// Délai maximal d'établissement de connexion et d'attente de données.
/// Plus court que celui de `telechargement.rs` (30s) : cette vérification
/// se fait silencieusement à chaque lancement, elle doit donc échouer vite
/// si le serveur LAN n'est pas joignable plutôt que de retarder le
/// démarrage de l'application.
const DELAI_RESEAU: std::time::Duration = std::time::Duration::from_secs(3);

/// Interroge `adresse` (hôte:port) pour savoir si une version plus récente
/// de l'application est disponible. Retourne `None` sur toute erreur
/// (réseau, JSON invalide, champ manquant) aussi bien que lorsque la
/// version annoncée est identique à `VERSION_ACTUELLE` — ce n'est jamais
/// une erreur fatale, seulement l'absence de mise à jour à proposer.
pub fn verifier_disponibilite(adresse: &str) -> Option<InfoMiseAJour> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(DELAI_RESEAU)
        .timeout_read(DELAI_RESEAU)
        .build();
    let reponse = agent
        .get(&format!("http://{adresse}/version.json"))
        .call()
        .ok()?;
    let corps = reponse.into_string().ok()?;
    let corps: ReponseVersion = serde_json::from_str(&corps).ok()?;
    if corps.version == VERSION_ACTUELLE {
        return None;
    }
    Some(InfoMiseAJour {
        version: corps.version,
        url_apk: corps.url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// Serveur HTTP minimal sur la boucle locale, le temps d'une seule
    /// requête, servant `corps` en réponse à `/version.json`. Mirroir de
    /// `telechargement.rs::servir_une_fois`, en plus simple : toujours un
    /// `Content-Length` (le corps JSON est connu à l'avance) et toujours
    /// `Connection: close`.
    fn servir_version_une_fois(corps: &str) -> (String, std::thread::JoinHandle<()>) {
        let ecouteur =
            std::net::TcpListener::bind("127.0.0.1:0").expect("écoute sur un port libre");
        let adresse = ecouteur.local_addr().expect("adresse locale");
        let corps = corps.to_string();
        let fil = std::thread::spawn(move || {
            let (mut flux, _) = ecouteur.accept().expect("connexion entrante");
            // Consommer les en-têtes de la requête jusqu'à la ligne vide.
            let mut octet = [0u8; 1];
            let mut fin = Vec::new();
            while flux.read_exact(&mut octet).is_ok() {
                fin.push(octet[0]);
                if fin.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let entetes = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                corps.len()
            );
            let _ = flux.write_all(entetes.as_bytes());
            let _ = flux.write_all(corps.as_bytes());
            let _ = flux.flush();
        });
        (format!("http://{adresse}/version.json"), fil)
    }

    #[test]
    fn version_actuelle_n_est_jamais_vide() {
        assert!(!VERSION_ACTUELLE.is_empty());
    }

    #[test]
    fn verifier_disponibilite_retourne_some_si_la_version_differe() {
        let (url, fil) = servir_version_une_fois(&format!(
            r#"{{"version":"autre-version","url":"http://exemple/narrateur.apk"}}"#
        ));
        let adresse = url.strip_prefix("http://").unwrap().trim_end_matches("/version.json");
        let resultat = verifier_disponibilite(adresse);
        fil.join().expect("fil serveur");
        assert_eq!(resultat, Some(InfoMiseAJour {
            version: "autre-version".into(),
            url_apk: "http://exemple/narrateur.apk".into(),
        }));
    }

    #[test]
    fn verifier_disponibilite_retourne_none_si_la_version_est_identique() {
        let corps = format!(r#"{{"version":"{VERSION_ACTUELLE}","url":"http://exemple/narrateur.apk"}}"#);
        let (url, fil) = servir_version_une_fois(&corps);
        let adresse = url.strip_prefix("http://").unwrap().trim_end_matches("/version.json");
        let resultat = verifier_disponibilite(adresse);
        fil.join().expect("fil serveur");
        assert_eq!(resultat, None);
    }

    #[test]
    fn verifier_disponibilite_retourne_none_si_json_invalide() {
        let (url, fil) = servir_version_une_fois("ceci n'est pas du json");
        let adresse = url.strip_prefix("http://").unwrap().trim_end_matches("/version.json");
        let resultat = verifier_disponibilite(adresse);
        fil.join().expect("fil serveur");
        assert_eq!(resultat, None);
    }

    #[test]
    fn verifier_disponibilite_retourne_none_si_le_port_est_ferme() {
        // Un port lié puis immédiatement fermé : connexion refusée garantie,
        // sans dépendre d'un vrai hôte distant injoignable.
        let ecouteur = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let adresse = ecouteur.local_addr().unwrap().to_string();
        drop(ecouteur);
        assert_eq!(verifier_disponibilite(&adresse), None);
    }
}
