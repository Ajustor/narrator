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

/// Échec du déclenchement de l'installeur de paquets Android.
#[cfg(feature = "plateforme-android")]
#[derive(Debug)]
pub enum ErreurInstallation {
    /// Aucune application de l'appareil ne sait traiter l'intent
    /// d'installation (`ActivityNotFoundException`).
    AucuneApplicationPourInstaller,
    /// Toute autre exception Java levée pendant la séquence d'appels
    /// (`FileProvider` mal déclaré, chemin hors de `file_paths.xml`,
    /// méthode retirée par R8, etc.) — porte `Throwable.toString()`.
    ExceptionJava(String),
    /// Échec côté JNI lui-même, sans exception Java (JVM indisponible,
    /// attachement du thread impossible, signature erronée...).
    Jni(String),
}

/// Texte affiché dans la bannière de mise à jour (`MiseAJourEchouee`).
#[cfg(feature = "plateforme-android")]
impl std::fmt::Display for ErreurInstallation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErreurInstallation::AucuneApplicationPourInstaller => {
                write!(f, "aucune application de l'appareil ne sait installer l'APK")
            }
            ErreurInstallation::ExceptionJava(detail) => write!(f, "exception Java : {detail}"),
            ErreurInstallation::Jni(detail) => write!(f, "erreur JNI : {detail}"),
        }
    }
}

/// Autorité du `FileProvider` déclaré par
/// `scripts/patch_manifest_maj_android.sh` dans le manifeste Android généré
/// (`Dioxus.toml` ne porte que la permission `REQUEST_INSTALL_PACKAGES` et
/// la règle proguard associées, voir ce script — `dx` 0.7.9 n'offre aucun
/// moyen déclaratif d'ajouter un `<provider>`) : le manifeste utilise
/// `${applicationId}.fileprovider`, on reconstruit donc la même valeur à
/// l'exécution à partir de `Context.getPackageName()` (égal à
/// l'`applicationId`) plutôt que de dupliquer l'identifiant en dur ici.
#[cfg(feature = "plateforme-android")]
const SUFFIXE_AUTORITE_FILE_PROVIDER: &str = ".fileprovider";

/// Ouvre l'installeur de paquets Android sur l'APK `chemin_apk` (qui doit
/// se trouver sous `<getFilesDir()>/maj/`, seul répertoire exposé par
/// `res/xml/file_paths.xml`). Retourne dès que l'installeur est lancé :
/// l'installation elle-même (et la demande d'autorisation « sources
/// inconnues » si elle n'a pas encore été accordée) se passe ensuite dans
/// l'UI système, hors de portée de l'app.
///
/// Séquence JNI, même idiome que `worker::repertoire_donnees_app()` :
/// 1. `FileProvider.getUriForFile(activity, "<package>.fileprovider", File(chemin))`
///    — la classe `androidx.core.content.FileProvider` est chargée via
///    `activity.getClassLoader().loadClass(...)` et NON via `env.find_class` :
///    sur un thread natif attaché à la JVM, `FindClass` utilise le
///    classloader système, qui ne voit que les classes du framework
///    Android, pas celles embarquées dans l'APK (androidx).
/// 2. `Intent(ACTION_VIEW)`, `setDataAndType(uri, "application/vnd.android.package-archive")`,
///    `addFlags(FLAG_GRANT_READ_URI_PERMISSION | FLAG_ACTIVITY_NEW_TASK)`.
/// 3. `activity.startActivity(intent)`.
///
/// Toute exception Java levée en chemin est récupérée puis effacée
/// (`exception_clear`) avant de rendre la main, et convertie en `Err` :
/// une exception laissée en attente ferait échouer le prochain appel JNI
/// de ce thread, voire planter l'app.
#[cfg(feature = "plateforme-android")]
pub fn lancer_installation(chemin_apk: &std::path::Path) -> Result<(), ErreurInstallation> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| ErreurInstallation::Jni(format!("JavaVM invalide : {e}")))?;
    let activity = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| ErreurInstallation::Jni(format!("attachement à la JVM : {e}")))?;
    let chemin = chemin_apk
        .to_str()
        .ok_or_else(|| ErreurInstallation::Jni("chemin de l'APK non UTF-8".to_string()))?;

    match appeler_installeur(&mut env, &activity, chemin) {
        Ok(()) => Ok(()),
        Err(erreur_jni) => Err(convertir_exception_en_attente(&mut env).unwrap_or_else(|| {
            ErreurInstallation::Jni(erreur_jni.to_string())
        })),
    }
}

#[cfg(feature = "plateforme-android")]
fn appeler_installeur(
    env: &mut jni::JNIEnv,
    activity: &jni::objects::JObject,
    chemin: &str,
) -> jni::errors::Result<()> {
    use jni::objects::{JClass, JValue};

    const FLAG_GRANT_READ_URI_PERMISSION: i32 = 0x0000_0001;
    const FLAG_ACTIVITY_NEW_TASK: i32 = 0x1000_0000;

    // 1. URI `content://` de l'APK via FileProvider.
    let classloader = env
        .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
        .l()?;
    let nom_classe = env.new_string("androidx.core.content.FileProvider")?;
    let classe_file_provider: JClass = env
        .call_method(
            &classloader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&nom_classe)],
        )?
        .l()?
        .into();

    let paquet = env
        .call_method(activity, "getPackageName", "()Ljava/lang/String;", &[])?
        .l()?;
    let paquet = jni::objects::JString::from(paquet);
    let paquet: String = env.get_string(&paquet)?.into();
    let autorite = env.new_string(format!("{paquet}{SUFFIXE_AUTORITE_FILE_PROVIDER}"))?;

    let chemin_jstring = env.new_string(chemin)?;
    let fichier = env.new_object("java/io/File", "(Ljava/lang/String;)V", &[JValue::Object(&chemin_jstring)])?;

    let uri = env
        .call_static_method(
            &classe_file_provider,
            "getUriForFile",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/io/File;)Landroid/net/Uri;",
            &[JValue::Object(activity), JValue::Object(&autorite), JValue::Object(&fichier)],
        )?
        .l()?;

    // 2. Intent ACTION_VIEW sur l'APK.
    let action = env.new_string("android.intent.action.VIEW")?;
    let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&action)])?;
    let type_mime = env.new_string("application/vnd.android.package-archive")?;
    env.call_method(
        &intent,
        "setDataAndType",
        "(Landroid/net/Uri;Ljava/lang/String;)Landroid/content/Intent;",
        &[JValue::Object(&uri), JValue::Object(&type_mime)],
    )?;
    env.call_method(
        &intent,
        "addFlags",
        "(I)Landroid/content/Intent;",
        &[JValue::Int(FLAG_GRANT_READ_URI_PERMISSION | FLAG_ACTIVITY_NEW_TASK)],
    )?;

    // 3. Lancement de l'installeur système.
    env.call_method(activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)])?;
    Ok(())
}

/// Si une exception Java est en attente sur `env`, l'efface et la convertit
/// en `ErreurInstallation` ; sinon `None` (l'échec venait de JNI seul).
#[cfg(feature = "plateforme-android")]
fn convertir_exception_en_attente(env: &mut jni::JNIEnv) -> Option<ErreurInstallation> {
    if !env.exception_check().unwrap_or(false) {
        return None;
    }
    let exception = env.exception_occurred().ok();
    // À effacer AVANT tout autre appel JNI : avec une exception en attente,
    // les appels ci-dessous (`is_instance_of`, `toString`) échoueraient.
    let _ = env.exception_clear();
    let Some(exception) = exception else {
        return Some(ErreurInstallation::ExceptionJava("exception Java illisible".to_string()));
    };
    if env
        .is_instance_of(&exception, "android/content/ActivityNotFoundException")
        .unwrap_or(false)
    {
        return Some(ErreurInstallation::AucuneApplicationPourInstaller);
    }
    let description = decrire_exception(env, &exception);
    if description.is_err() {
        // `toString()` lui-même a pu lever : ne rien laisser en attente.
        let _ = env.exception_clear();
    }
    Some(ErreurInstallation::ExceptionJava(
        description.unwrap_or_else(|_| "exception Java (description indisponible)".to_string()),
    ))
}

#[cfg(feature = "plateforme-android")]
fn decrire_exception(env: &mut jni::JNIEnv, exception: &jni::objects::JObject) -> jni::errors::Result<String> {
    let texte = env
        .call_method(exception, "toString", "()Ljava/lang/String;", &[])?
        .l()?;
    let texte = jni::objects::JString::from(texte);
    let texte: String = env.get_string(&texte)?.into();
    Ok(texte)
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
