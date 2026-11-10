# Mise à jour automatique (Android) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On every Android launch, silently check the LAN dev server for a
newer build; if found, show a dismissible banner that lets the player
download it and launch Android's own installer, without ever touching the
desktop build.

**Architecture:** A new platform-agnostic `mise_a_jour` module owns version
comparison and reuses the existing `telechargement` module's proven
download loop; everything from the startup check through the install
trigger is wired through `worker.rs`'s existing message-passing pattern and
gated behind the `plateforme-android` feature, exactly like
`repertoire_donnees_app` already is. The riskiest piece — launching
Android's package installer via JNI, which nothing in this codebase has
done before — is isolated into its own task so a dead end there doesn't
block the rest.

**Tech Stack:** Rust, Dioxus 0.7, `ureq` (already a dependency), `jni`/
`ndk-context` (already optional deps under `plateforme-android`).

**Spec:** `docs/superpowers/specs/2026-11-10-maj-auto-android-design.md`

## Global Constraints

- Entirely behind `#[cfg(feature = "plateforme-android")]` — the desktop
  build must compile and behave identically to before this plan, with
  zero new network calls.
- Default check address: `ADRESSE_MAJ_PAR_DEFAUT: &str = "192.168.1.48:8000"`
  (a constant, no network discovery — out of scope per the spec).
- Manifest format served at `http://{adresse}/version.json`:
  `{"version": "<git short hash>", "url": "<apk url>"}`.
- Version identity: the running build's own git short hash, injected via
  `build.rs` into `env!("NARRATEUR_GIT_HASH")` — never Android's
  `versionCode`/`versionName` (never bumped in this project to date).
  `build.rs` must always set the env var to something (the real hash, or
  the literal string `"inconnu"` on any failure to read it) — `env!()`,
  not `option_env!()`, since the var is guaranteed set either way.
- Every network failure (timeout, connection refused, malformed JSON)
  must resolve to `None`/a silent no-op — never a panic, never a message
  shown to the player for the "no update available" case.
- Any failure launching the Android installer (e.g. no app can handle the
  intent) must become a clean `Result::Err`, never a JNI panic that takes
  down the whole app.
- Do not touch `network_security_config.xml` or `Dioxus.toml`'s existing
  `[bundle]`/`[android]` sections speculatively — only add what Task 3
  specifically needs, and only if the declarative `Dioxus.toml` route is
  confirmed not to work (see Task 3).

## Review Focus

1. LAN server unreachable (the common case — it only runs during dev
   sessions) must produce a silent `None`, not a hung UI or an error
   banner — pinned in Task 1 with a closed-port test.
2. A manifest whose `version` matches the running build's own hash must
   NOT show the banner (no false-positive nagging on every launch once
   the player is already up to date) — pinned in Task 1.
3. Malformed JSON or a missing `version`/`url` field must not panic —
   pinned in Task 1.
4. The desktop build must not gain any new behavior, network call, or
   compiled code from this feature — pinned in Task 2 by checking the
   non-Android build still compiles clean with no new runtime calls in
   that configuration.
5. Android's installer intent failing to resolve (no handler) must
   surface as a clean error the banner can show, not a crash — pinned in
   Task 3.

---

## Task 1: Version-check module

**Files:**
- Create: `crates/app_desktop/build.rs`
- Create: `crates/app_desktop/src/mise_a_jour.rs`
- Modify: `crates/app_desktop/src/main.rs` (add `mod mise_a_jour;` near the
  other `mod` declarations)

**Interfaces:**
- Produces: `pub const VERSION_ACTUELLE: &str` (the compiled-in git short
  hash); `pub const ADRESSE_MAJ_PAR_DEFAUT: &str = "192.168.1.48:8000"`;
  `pub struct InfoMiseAJour { pub version: String, pub url_apk: String }`
  (derive `Debug, Clone, PartialEq`); `pub fn verifier_disponibilite(adresse: &str) -> Option<InfoMiseAJour>`.
- Consumes: nothing from other tasks.

This task is self-contained and fully unit-testable with no Android
dependency — mirror the local-test-server pattern already used in
`crates/app_desktop/src/telechargement.rs`'s test module (`servir_une_fois`:
a raw `TcpListener` on `127.0.0.1:0`, a background thread writing a minimal
HTTP/1.1 response, returning `(url, JoinHandle)`) rather than inventing a
new testing approach.

- [ ] **Step 1: Write `build.rs`.** Run `git rev-parse --short HEAD` via
  `std::process::Command`, capture stdout, trim it; if the command fails
  or exits non-zero, use the literal string `"inconnu"` instead. Emit
  `println!("cargo:rustc-env=NARRATEUR_GIT_HASH={hash}");`. No test for
  this file alone (a build script has no independent unit test); it's
  exercised by Step 2's test depending on `VERSION_ACTUELLE` resolving to
  a non-empty string.

- [ ] **Step 2: Write the failing tests** in
  `crates/app_desktop/src/mise_a_jour.rs`'s `#[cfg(test)]` module:

```rust
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
```

  Write the `servir_version_une_fois(corps: &str) -> (String, std::thread::JoinHandle<()>)`
  test helper following `telechargement.rs`'s `servir_une_fois` exactly
  (same `TcpListener`/header-consuming/response-writing shape), serving
  `corps` as the body at a URL ending `/version.json`, with a
  `Content-Length` header and `Connection: close`.

- [ ] **Step 3: Run and verify failure** —
  `cargo test -p app_desktop mise_a_jour::` — expected: fails to compile
  (module/types don't exist).

- [ ] **Step 4: Implement `InfoMiseAJour` and `verifier_disponibilite`** in
  `mise_a_jour.rs`. Use `ureq` with a short timeout (3s connect, 3s read —
  shorter than `telechargement.rs`'s 30s, since this must fail fast on a
  silent check) and `serde_json` (already a workspace dependency) to parse
  the body into a small `#[derive(Deserialize)]` struct, or extract
  `"version"`/`"url"` manually if that's simpler — either way, any
  parse/field-missing error must map to `None`, same as a network error.
  Compare the parsed `version` against `VERSION_ACTUELLE`; equal → `None`.

- [ ] **Step 5: Run and verify pass** —
  `cargo test -p app_desktop mise_a_jour::` — expected: all 5 PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/app_desktop/build.rs crates/app_desktop/src/mise_a_jour.rs crates/app_desktop/src/main.rs
git commit -m "feat(app_desktop): add LAN update-check module"
```

---

## Task 2: Wire the check, download, and banner into the UI (Android-gated)

**Files:**
- Modify: `crates/app_desktop/src/telechargement.rs` (visibility only)
- Modify: `crates/app_desktop/src/protocole.rs`
- Modify: `crates/app_desktop/src/worker.rs`
- Modify: `crates/app_desktop/src/main.rs`
- Modify: `crates/app_desktop/src/styles.rs`
- Create: `crates/app_desktop/src/vues/banniere_maj.rs`
- Modify: `crates/app_desktop/src/vues/mod.rs` (export the new component,
  following however the existing `vues` modules are exported there)

**Interfaces:**
- Consumes: `mise_a_jour::{VERSION_ACTUELLE, ADRESSE_MAJ_PAR_DEFAUT, InfoMiseAJour, verifier_disponibilite}` (Task 1).
- Produces: `MessageVersWorker::{TelechargerMiseAJour { url_apk: String }, InstallerMiseAJour { chemin_apk: String }}`;
  `MessageVersUi::{MiseAJourDisponible { version: String, url_apk: String }, ProgressionMiseAJour(f32), MiseAJourTelechargee { chemin_apk: String }, MiseAJourEchouee(String)}`;
  a `BanniereMiseAJour` Dioxus component consumed by `main.rs`. Task 3
  consumes `MessageVersWorker::InstallerMiseAJour`'s handling point in
  `worker.rs` (this task wires the message through; Task 3 implements
  what happens when it's handled).

This task does NOT implement the actual Android install trigger — handling
`MessageVersWorker::InstallerMiseAJour` in this task just needs to compile
and do nothing yet (or call a stub); Task 3 fills in the real behavior.
Keep that stub narrow and clearly marked, so Task 3's diff is additive.

- [ ] **Step 1: Bump visibility.** In `telechargement.rs`, change
  `fn telecharger_depuis(...)` to `pub(crate) fn telecharger_depuis(...)` —
  no behavior change, no new test (its existing tests already prove the
  loop is URL-agnostic).

- [ ] **Step 2: Add the message variants.** In `protocole.rs`, add the
  four `MessageVersUi` variants and two `MessageVersWorker` variants listed
  in Interfaces above, matching the existing enums' derive/doc-comment
  style.

- [ ] **Step 3: Write the failing test** in `worker.rs`'s test module —
  this task's one meaningfully testable piece is the download handling,
  independent of the startup trigger (already covered by Task 1) and the
  install trigger (Task 3):

```rust
#[test]
fn telecharger_mise_a_jour_ecrit_l_apk_et_signale_la_fin() {
    // Réutilise le même motif de petit serveur local que
    // `telechargement.rs`/`mise_a_jour.rs` — un fichier factice suffit,
    // peu importe son contenu réel pour ce test.
    let (url, fil) = servir_fichier_une_fois(b"contenu-apk-factice".to_vec());
    let destination = /* chemin temporaire de test, même motif que telechargement.rs::chemin_test */;

    let resultat = telecharger_mise_a_jour(&url, &destination, &mut |_| {});
    fil.join().expect("fil serveur");

    assert!(resultat.is_ok());
    assert_eq!(std::fs::read(&destination).unwrap(), b"contenu-apk-factice");
    let _ = std::fs::remove_file(&destination);
}
```

  Write whatever local function (`telecharger_mise_a_jour` or call
  `telechargement::telecharger_depuis` directly — your call, see "Ce qui
  reste au développeur" below) handles the `TelechargerMiseAJour` message's
  actual download step, and a `servir_fichier_une_fois` helper mirroring
  `telechargement.rs`'s `servir_une_fois` if the existing one isn't
  directly reusable from this module's test scope.

- [ ] **Step 4: Run and verify failure** — expected: compile error (types
  don't exist yet).

- [ ] **Step 5: Implement the worker wiring**, all behind
  `#[cfg(feature = "plateforme-android")]` at the outermost reasonable
  scope (the startup spawn, and the `TelechargerMiseAJour`/
  `InstallerMiseAJour` message handlers — but the download call itself can
  stay platform-agnostic since `telecharger_depuis` already is, following
  the Global Constraint that desktop must gain zero new network calls, not
  zero new compiled code where that's impractical to achieve cleanly):
  - At `lancer()`'s startup (after the initial `ListeHistoires` send,
    following whatever existing precedent this function has for a
    fire-and-forget startup check), call `verifier_disponibilite(ADRESSE_MAJ_PAR_DEFAUT)`;
    on `Some(info)`, send `MessageVersUi::MiseAJourDisponible { version: info.version, url_apk: info.url_apk }`.
  - On `MessageVersWorker::TelechargerMiseAJour { url_apk }`: pick a
    destination path (Android: alongside the existing
    `repertoire_donnees_app()`-based paths, e.g. a `maj/narrateur.apk`
    file; desktop: an equivalent relative path for symmetry, even though
    the UI path to reach this message is Android-only), download via
    `telecharger_depuis`, sending `ProgressionMiseAJour` on each progress
    callback and `MiseAJourTelechargee { chemin_apk }` on success or
    `MiseAJourEchouee` on failure (translate `ErreurTelechargement`'s
    `Debug` or a short French message — your call).
  - On `MessageVersWorker::InstallerMiseAJour { chemin_apk }`: for now
    (Task 3 fills this in), do nothing beyond what's needed to compile —
    e.g. a `#[cfg(feature = "plateforme-android")]`-gated call to a stub
    `mise_a_jour::lancer_installation` that Task 3 will implement, or
    simply `let _ = chemin_apk;` with a comment pointing at Task 3 if you
    prefer not to forward-declare a stub function. Your call — whichever
    keeps this task's diff clean and Task 3's diff additive.

- [ ] **Step 6: Run and verify pass** —
  `cargo test -p app_desktop` (full crate, not just one module, since this
  step touches several files) — expected: all PASS, including the new
  test from Step 3.

- [ ] **Step 7: Add the UI signal and banner.** In `main.rs`, add a
  `Signal<EtatBanniereMaj>` (new enum: `Masquee`, `Disponible { version, url_apk }`,
  `EnTelechargement { progression: f32 }`, `PreteAInstaller { chemin_apk }`,
  `Echouee(String)`) alongside the existing overlay signals (`de_en_attente`,
  etc. — not inside the `Vue` enum, since the banner must show over any
  screen). Handle the four new `MessageVersUi` variants in the big
  `while let Some(message) = rx.next().await` match to drive it. Render
  `BanniereMiseAJour` unconditionally above `vue_active`, passing the
  signal's current value and `sur_telecharger`/`sur_installer`/
  `sur_ignorer` handlers that send the corresponding `MessageVersWorker` or
  just reset the signal to `Masquee` (for "ignorer").

- [ ] **Step 8: Write `vues/banniere_maj.rs`** — a small presentational
  component (`#[component] pub fn BanniereMiseAJour(etat: EtatBanniereMaj, sur_telecharger: EventHandler<()>, sur_installer: EventHandler<()>, sur_ignorer: EventHandler<()>) -> Element`),
  rendering nothing when `etat` is `Masquee`, otherwise a fixed banner with
  the version string, a progress indication when downloading, and the
  right button per state. Follow this crate's existing RSX/event-handler
  style (see `vues/confirmation_telechargement.rs` for the closest existing
  precedent — a similar "notify and confirm an action" UI).

- [ ] **Step 9: Add banner CSS** to `styles.rs`, following the file's
  existing class-naming and token conventions (reuse existing
  `var(--bg)`/`var(--text)`/etc. custom properties rather than introducing
  new literals).

- [ ] **Step 10: Compile-check both configurations** —
  `cargo check -p app_desktop` (desktop, no android feature — confirm it
  still compiles with no new runtime behavior reachable) and
  `cargo check -p app_desktop --no-default-features --features plateforme-android,document`
  (Android feature set, cross-compiled target not required for this
  check — a host-arch `cargo check` with the feature flag on is enough to
  catch any `#[cfg]` mistake before the real Android cross-compile in
  Task 3).

- [ ] **Step 11: Run the full workspace suite once** —
  `cargo test --workspace` — expected: all pass, 0 new warnings.

- [ ] **Step 12: Commit.**

```bash
git add crates/app_desktop/src/telechargement.rs crates/app_desktop/src/protocole.rs \
        crates/app_desktop/src/worker.rs crates/app_desktop/src/main.rs \
        crates/app_desktop/src/styles.rs crates/app_desktop/src/vues/banniere_maj.rs \
        crates/app_desktop/src/vues/mod.rs
git commit -m "feat(app_desktop): check for updates on launch, download on confirmation"
```

---

## Task 3: Launch the Android installer (exploratory — expect to pivot)

**Files:**
- Modify: `crates/app_desktop/Dioxus.toml`
- Modify: `crates/app_desktop/src/mise_a_jour.rs`
- Modify: `crates/app_desktop/src/worker.rs` (fill in Task 2's stub)
- Create (only if the `Dioxus.toml` route fails empirically — see Step 2):
  `scripts/patch_manifest_maj_android.sh`

**Interfaces:**
- Consumes: `MessageVersWorker::InstallerMiseAJour`'s handling point
  (Task 2's stub).
- Produces: `#[cfg(feature = "plateforme-android")] pub fn lancer_installation(chemin_apk: &std::path::Path) -> Result<(), ErreurInstallation>`
  in `mise_a_jour.rs` (new `ErreurInstallation` type, `Debug`, carrying at
  minimum a variant for "no app can handle the install intent").

This task has no meaningful `cargo test` — none of it can run on this dev
machine (no JNI runtime, no Android device connected). "Passing" for this
task means: it cross-compiles for `aarch64-linux-android`, and the manifest
changes are visible in the generated Android project. Real functional
verification needs the user's own phone — say so plainly when reporting,
don't claim more than the build output supports.

- [ ] **Step 1: Declare the manifest additions in `Dioxus.toml` first.**
  Add, in whichever of `[permissions]` or `[android.raw.manifest]` `dx`
  0.7 actually supports for this (read `dx`'s own docs/help if available,
  or infer from its generated-manifest comments — "Permissions from
  Dioxus.toml [permissions] section" / "Raw manifest XML from Dioxus.toml
  [android.raw.manifest]" are the two placeholder comments already present
  in the generated manifest): the `android.permission.REQUEST_INSTALL_PACKAGES`
  permission, a `<provider>` for `androidx.core.content.FileProvider`
  (authority `${applicationId}.fileprovider`, `android:exported="false"`,
  `android:grantUriPermissions="true"`, a `<meta-data>` pointing at
  `@xml/file_paths`), and the `res/xml/file_paths.xml` resource it
  references (a `<paths>` XML declaring the directory Task 2 downloads
  the APK into as a `<files-path>` or `<external-files-path>` entry,
  matching whatever destination Task 2 actually chose).

- [ ] **Step 2: Rebuild the Android project and inspect the generated
  manifest** (`dx bundle --platform android ... --target aarch64-linux-android`,
  same invocation already used in this project, then read the generated
  `AndroidManifest.xml` and check for `res/xml/file_paths.xml`). If the
  `Dioxus.toml` declarations show up correctly: done with this step, skip
  to Step 3. If they don't (silently dropped, or `dx` errors): write
  `scripts/patch_manifest_maj_android.sh` following
  `scripts/patch_icone_android.sh`'s exact pattern (same parameters,
  same "re-run after every `dx build`/`dx bundle`, before `gradlew`"
  contract, same style of embedded comment explaining why) to inject the
  same `<provider>`/permission/resource directly into the generated
  project instead. Either way, the end state after this step must be: a
  rebuilt Android project that actually contains the FileProvider
  declaration, verified by reading the generated files — not assumed.

- [ ] **Step 3: Implement `lancer_installation`.** Using the existing
  `jni`/`ndk-context` pattern in `worker.rs`'s `repertoire_donnees_app()`
  as your reference for how this codebase attaches to the JVM and calls
  into the Activity: resolve the APK's `content://` URI via
  `FileProvider.getUriForFile(context, "${applicationId}.fileprovider", file)`
  (this class needs resolving through the activity's classloader, not the
  native activity's default one — `repertoire_donnees_app` doesn't need
  this since it only calls methods already reachable from
  `Activity`/`File`, so this is genuinely new JNI territory; if you get
  stuck here specifically, that's the moment to report BLOCKED with
  exactly what you tried, not to keep guessing), build an `Intent` with
  `ACTION_VIEW`, `setDataAndType(uri, "application/vnd.android.package-archive")`,
  `FLAG_GRANT_READ_URI_PERMISSION`, and call `Activity.startActivity(intent)`.
  Wrap the whole thing so any JNI exception (in particular
  `ActivityNotFoundException`, per Review Focus item 5) is caught and
  converted to `Err(ErreurInstallation::...)` rather than propagating as a
  panic or an unhandled Java exception that crashes the app.

- [ ] **Step 4: Wire it into `worker.rs`**, replacing Task 2's stub for
  `MessageVersWorker::InstallerMiseAJour` with a real call to
  `lancer_installation`, sending `MiseAJourEchouee` on `Err`.

- [ ] **Step 5: Cross-compile check** —
  `cargo check -p app_desktop --target aarch64-linux-android --no-default-features --features plateforme-android,document`
  (same target/feature set already used for every Android build in this
  project) — expected: compiles clean.

- [ ] **Step 6: Build the real APK once**, following this project's
  established sequence (`dx bundle --release ...`, icon patch, this
  task's manifest patch if Step 2 needed one, `gradlew assembleRelease`),
  and report in plain language what you could verify from the build
  output alone (e.g. "the FileProvider and permission are present in the
  final merged manifest") versus what genuinely needs a real device
  (whether tapping the banner's "Installer" button actually opens the
  system installer).

- [ ] **Step 7: Commit.**

```bash
git add crates/app_desktop/Dioxus.toml crates/app_desktop/src/mise_a_jour.rs crates/app_desktop/src/worker.rs
# Add scripts/patch_manifest_maj_android.sh too, only if Step 2 created it.
git commit -m "feat(app_desktop): launch Android's package installer for a downloaded update"
```

---

## Ce qui reste au développeur

- Task 2: whether `TelechargerMiseAJour`'s handler calls
  `telechargement::telecharger_depuis` directly, or through a one-line
  `mise_a_jour::telecharger_mise_a_jour` wrapper — either is fine, the
  spec doesn't require a specific indirection level here.
- Task 2: exact destination path for the downloaded APK on both
  platforms (must be consistent with whatever Task 3's `file_paths.xml`
  ends up declaring — if Task 2 and Task 3 land as separate dispatches,
  the destination path decided in Task 2 must be communicated forward
  via this plan's ledger/dispatch, not re-guessed).
- Banner copy (exact French wording), its exact placement/z-index in the
  RSX tree, and the "ignorer" button's exact visual treatment.
- Task 3: exact wording of `ErreurInstallation`'s variants.
- Task 3: which of `[permissions]`/`[android.raw.manifest]` `dx` 0.7
  actually supports for this — genuinely unknown until tried, per the
  spec's own flagged risk.
