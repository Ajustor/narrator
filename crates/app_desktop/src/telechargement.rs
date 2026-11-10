use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug, PartialEq)]
pub enum ErreurTelechargement {
    Reseau(String),
    Ecriture(String),
}

/// Décision pure : le fichier modèle est-il déjà présent à l'emplacement
/// attendu ? Desktop télécharge désormais les modèles manquants à la
/// demande depuis le catalogue, exactement comme Android — ce n'est plus
/// une garantie de plateforme que le fichier existe déjà. La seule raison
/// pour laquelle un poste de développement avec `scripts/jouer.sh` évite
/// souvent le téléchargement est une coïncidence de nommage : le fichier
/// qu'il télécharge déjà (`modeles/Qwen2.5-3B-Instruct-Q4_K_M.gguf`)
/// correspond au nom attendu par la palette Équilibré.
pub fn faut_il_telecharger(chemin_modele: &Path) -> bool {
    !chemin_modele.exists()
}

/// Délai maximal d'établissement de connexion et d'attente de données.
/// Sans ces délais, une connexion mobile qui se bloque fige le thread
/// worker (et donc l'interface, qui reste sur l'écran de téléchargement)
/// indéfiniment, sans autre issue que de tuer l'application.
const DELAI_RESEAU: std::time::Duration = std::time::Duration::from_secs(30);

/// Télécharge le modèle vers `chemin_destination`, en passant la
/// progression (0.0 à 1.0) à `sur_progression` au fil du flux. Écrit dans
/// un fichier `.tmp` puis renomme de façon atomique vers la destination
/// finale une fois le téléchargement terminé — même principe que
/// `scripts/jouer.sh`, pour ne jamais laisser un fichier partiel au nom
/// final si le téléchargement est interrompu.
///
/// `sur_progression` est appelé une première fois avec `0.0` dès que la
/// réponse est ouverte, pour que l'interface bascule sur l'écran de
/// téléchargement même si le serveur n'envoie pas d'en-tête
/// `Content-Length` (dans ce cas la progression reste à `0.0` pendant
/// toute la durée du transfert — état indéterminé, plutôt qu'aucun écran
/// du tout). Ensuite il n'est rappelé que lorsque le pourcentage entier
/// change : la vue n'affiche qu'un entier arrondi, donc au plus 101
/// rendus sont visuellement utiles, contre ~32 000 appels (un par bloc de
/// 64 Kio) pour un fichier de 2 Gio.
pub fn telecharger_modele(
    palette: &crate::modeles::PaletteModele,
    chemin_destination: &Path,
    sur_progression: &mut dyn FnMut(f32),
) -> Result<(), ErreurTelechargement> {
    telecharger_depuis(palette.url(), chemin_destination, sur_progression)
}

/// Récupère la taille totale du modèle (en octets) sans le télécharger —
/// utilisé pour demander confirmation à l'utilisateur avant de lancer un
/// téléchargement de l'ordre du gigaoctet. Une requête `Range` ne portant
/// que sur le premier kilo-octet déclenche une réponse `206 Partial
/// Content` dont l'en-tête `Content-Range: bytes 0-1023/<total>` donne la
/// taille réelle, sans transférer le fichier complet.
pub fn verifier_taille_modele(palette: &crate::modeles::PaletteModele) -> Result<u64, ErreurTelechargement> {
    verifier_taille_depuis(palette.url())
}

fn verifier_taille_depuis(url: &str) -> Result<u64, ErreurTelechargement> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(DELAI_RESEAU)
        .timeout_read(DELAI_RESEAU)
        .build();
    let reponse = agent
        .get(url)
        .set("Range", "bytes=0-1023")
        .call()
        .map_err(|e| ErreurTelechargement::Reseau(e.to_string()))?;

    let content_range = reponse.header("Content-Range").ok_or_else(|| {
        ErreurTelechargement::Reseau("en-tête Content-Range absent de la réponse".to_string())
    })?;

    content_range
        .rsplit('/')
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(|| ErreurTelechargement::Reseau(format!("Content-Range inattendu : {content_range}")))
}

/// Formate une taille en octets en gigaoctets lisibles, avec une décimale
/// et la virgule française (ex. `1929903264` → `"1,8 Go"`).
pub fn formater_taille_go(octets: u64) -> String {
    format!("{:.1} Go", octets as f64 / 1_000_000_000.0).replace('.', ",")
}

/// Corps réel de `telecharger_modele`, paramétré par l'URL source. Séparé
/// uniquement pour que les tests puissent exercer la boucle de
/// téléchargement (progression, nettoyage du `.tmp`, absence de
/// `Content-Length`) contre un petit serveur local, sans transférer les
/// ~2 Gio du vrai modèle.
pub(crate) fn telecharger_depuis(
    url: &str,
    chemin_destination: &Path,
    sur_progression: &mut dyn FnMut(f32),
) -> Result<(), ErreurTelechargement> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(DELAI_RESEAU)
        .timeout_read(DELAI_RESEAU)
        .build();
    let reponse = agent
        .get(url)
        .call()
        .map_err(|e| ErreurTelechargement::Reseau(e.to_string()))?;

    let taille_totale: u64 = reponse
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    sur_progression(0.0);

    let chemin_temporaire = chemin_destination.with_extension("tmp");
    if let Some(parent) = chemin_temporaire.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ErreurTelechargement::Ecriture(e.to_string()))?;
    }
    let mut fichier = std::fs::File::create(&chemin_temporaire)
        .map_err(|e| ErreurTelechargement::Ecriture(e.to_string()))?;

    // À partir d'ici le fichier `.tmp` existe : tout retour en erreur doit
    // le supprimer, sinon un téléchargement interrompu laisse jusqu'à ~2 Gio
    // orphelins sur un appareil dont on exige justement ≥ 3 Gio libres (une
    // nouvelle tentative repart de l'octet 0 et ne récupère pas cet espace).
    // `nettoyer` centralise ce traitement pour ne pas l'oublier sur une
    // branche d'erreur.
    let nettoyer = |erreur: ErreurTelechargement| -> ErreurTelechargement {
        let _ = std::fs::remove_file(&chemin_temporaire);
        erreur
    };

    let mut lecteur = reponse.into_reader();
    let mut tampon = [0u8; 65536];
    let mut telecharge: u64 = 0;
    let mut dernier_pourcentage_signale: u32 = 0;
    loop {
        let n = lecteur
            .read(&mut tampon)
            .map_err(|e| nettoyer(ErreurTelechargement::Reseau(e.to_string())))?;
        if n == 0 {
            break;
        }
        fichier
            .write_all(&tampon[..n])
            .map_err(|e| nettoyer(ErreurTelechargement::Ecriture(e.to_string())))?;
        telecharge += n as u64;
        if taille_totale > 0 {
            let pourcentage = (telecharge as f32 / taille_totale as f32 * 100.0) as u32;
            if pourcentage != dernier_pourcentage_signale {
                dernier_pourcentage_signale = pourcentage;
                sur_progression(telecharge as f32 / taille_totale as f32);
            }
        }
    }

    std::fs::rename(&chemin_temporaire, chemin_destination)
        .map_err(|e| nettoyer(ErreurTelechargement::Ecriture(e.to_string())))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serveur HTTP minimal sur la boucle locale, le temps d'une seule
    /// requête. Permet d'exercer la vraie boucle de `telecharger_depuis`
    /// (progression, nettoyage, absence de `Content-Length`) sans réseau
    /// externe ni transfert de plusieurs gigaoctets.
    ///
    /// `corps` est le contenu servi ; `taille_annoncee` est la valeur mise
    /// dans l'en-tête `Content-Length` (`None` = aucun en-tête, réponse
    /// délimitée par la fermeture de connexion, comme en HTTP/1.0).
    fn servir_une_fois(
        corps: Vec<u8>,
        taille_annoncee: Option<u64>,
    ) -> (String, std::thread::JoinHandle<()>) {
        let ecouteur =
            std::net::TcpListener::bind("127.0.0.1:0").expect("écoute sur un port libre");
        let adresse = ecouteur.local_addr().expect("adresse locale");
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
            let entetes = match taille_annoncee {
                Some(taille) => format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {taille}\r\nConnection: close\r\n\r\n"
                ),
                None => "HTTP/1.0 200 OK\r\nConnection: close\r\n\r\n".to_string(),
            };
            let _ = flux.write_all(entetes.as_bytes());
            let _ = flux.write_all(&corps);
            let _ = flux.flush();
        });
        (format!("http://{adresse}/modele.gguf"), fil)
    }

    fn chemin_test(nom: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "app_desktop_dl_{}_{}_{nom}.gguf",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    /// Fixes #4 et #5 : la progression est signalée une fois à `0.0` dès
    /// l'ouverture de la réponse, puis seulement quand le pourcentage
    /// entier change — donc bien moins d'appels que de blocs lus.
    #[test]
    fn progression_signalee_une_fois_par_pourcentage_entier() {
        // 16 Mio = 256 blocs de 64 Kio, mais seulement 100 pourcentages
        // entiers distincts : le compteur d'appels doit rester ≤ 101.
        const TAILLE: usize = 16 * 1024 * 1024;
        let corps = vec![0xABu8; TAILLE];
        let (url, fil) = servir_une_fois(corps, Some(TAILLE as u64));
        let chemin = chemin_test("gating");

        let mut progressions: Vec<f32> = Vec::new();
        let resultat = telecharger_depuis(&url, &chemin, &mut |p| progressions.push(p));
        fil.join().expect("fil serveur");

        assert!(resultat.is_ok(), "téléchargement local : {resultat:?}");
        assert_eq!(
            progressions.first().copied(),
            Some(0.0),
            "le premier appel doit être 0.0, pour afficher l'écran immédiatement"
        );
        assert_eq!(
            progressions.last().copied(),
            Some(1.0),
            "le dernier appel doit être 1.0"
        );
        let nombre_de_blocs = TAILLE / 65536;
        assert!(
            progressions.len() <= 101,
            "au plus 101 appels (0 % à 100 %), obtenu {} pour {} blocs lus",
            progressions.len(),
            nombre_de_blocs
        );
        assert!(
            progressions.len() < nombre_de_blocs,
            "la limitation doit réduire le nombre d'appels sous le nombre de blocs"
        );
        assert!(
            progressions.windows(2).all(|p| p[1] >= p[0]),
            "la progression doit être monotone croissante"
        );
        assert_eq!(
            std::fs::metadata(&chemin).map(|m| m.len()).unwrap_or(0),
            TAILLE as u64
        );
        assert!(!chemin.with_extension("tmp").exists(), "le .tmp doit être renommé");
        let _ = std::fs::remove_file(&chemin);
    }

    /// Fix #5 : sans `Content-Length`, l'écran de téléchargement doit
    /// quand même apparaître — `sur_progression` est appelé au moins une
    /// fois (avec `0.0`, état indéterminé), et le fichier est complet.
    #[test]
    fn progression_signalee_meme_sans_content_length() {
        let corps = b"contenu factice de modele".to_vec();
        let attendu = corps.clone();
        let (url, fil) = servir_une_fois(corps, None);
        let chemin = chemin_test("sans_cl");

        let mut progressions: Vec<f32> = Vec::new();
        let resultat = telecharger_depuis(&url, &chemin, &mut |p| progressions.push(p));
        fil.join().expect("fil serveur");

        assert!(resultat.is_ok(), "téléchargement local : {resultat:?}");
        assert_eq!(progressions, vec![0.0]);
        assert_eq!(std::fs::read(&chemin).expect("lecture"), attendu);
        let _ = std::fs::remove_file(&chemin);
    }

    /// Fix #10 : un téléchargement interrompu ne doit pas laisser de
    /// fichier `.tmp` orphelin sur disque. Le serveur annonce ici plus
    /// d'octets qu'il n'en envoie puis ferme la connexion.
    #[test]
    fn fichier_temporaire_supprime_si_le_telechargement_echoue() {
        let corps = vec![0u8; 1024];
        let (url, fil) = servir_une_fois(corps, Some(10 * 1024 * 1024));
        let chemin = chemin_test("echec");

        let resultat = telecharger_depuis(&url, &chemin, &mut |_| {});
        fil.join().expect("fil serveur");

        assert!(resultat.is_err(), "un corps tronqué doit échouer");
        assert!(
            !chemin.with_extension("tmp").exists(),
            "le .tmp partiel doit être supprimé sur le chemin d'erreur"
        );
        assert!(!chemin.exists(), "aucun fichier final ne doit apparaître");
    }

    /// Serveur HTTP minimal répondant `206 Partial Content` avec un
    /// `Content-Range` annonçant `taille_totale`, quel que soit le `Range`
    /// demandé — suffisant pour tester `verifier_taille_depuis` sans
    /// dépendre d'un vrai serveur distant.
    fn servir_taille_une_fois(taille_totale: u64) -> (String, std::thread::JoinHandle<()>) {
        let ecouteur =
            std::net::TcpListener::bind("127.0.0.1:0").expect("écoute sur un port libre");
        let adresse = ecouteur.local_addr().expect("adresse locale");
        let fil = std::thread::spawn(move || {
            let (mut flux, _) = ecouteur.accept().expect("connexion entrante");
            let mut octet = [0u8; 1];
            let mut fin = Vec::new();
            while flux.read_exact(&mut octet).is_ok() {
                fin.push(octet[0]);
                if fin.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let reponse = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-1023/{taille_totale}\r\nContent-Length: 1024\r\nConnection: close\r\n\r\n"
            );
            let _ = flux.write_all(reponse.as_bytes());
            let _ = flux.write_all(&vec![0u8; 1024]);
            let _ = flux.flush();
        });
        (format!("http://{adresse}/modele.gguf"), fil)
    }

    #[test]
    fn verifier_taille_lit_le_content_range() {
        let (url, fil) = servir_taille_une_fois(1_929_903_264);
        let resultat = verifier_taille_depuis(&url);
        fil.join().expect("fil serveur");
        assert_eq!(resultat, Ok(1_929_903_264));
    }

    #[test]
    fn formater_taille_go_utilise_la_virgule_francaise() {
        assert_eq!(formater_taille_go(1_929_903_264), "1,9 Go");
        assert_eq!(formater_taille_go(2_000_000_000), "2,0 Go");
    }

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
