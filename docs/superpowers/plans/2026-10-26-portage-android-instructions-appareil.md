# Instructions d'installation du port Android - Narrateur

## Résumé

Le port Android de Narrateur a été construit avec succès. Ce document fournit les instructions pour installer et tester l'application sur un appareil Android réel.

## Chemin de l'APK

L'APK de debug construit est disponible à :

```
target/dx/app_desktop/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
```

Taille : 83.9 MB

## Installation sur un appareil Android

### Prérequis

- Un appareil Android connecté via USB
- Le mode de débogage USB activé sur l'appareil
- `adb` (Android Debug Bridge) installé et configuré sur la machine de développement
- Une connexion réseau stable sur l'appareil (pour le téléchargement du modèle)
- Au minimum 3 Go d'espace de stockage disponible sur l'appareil

### Commande d'installation

Exécutez la commande suivante pour installer l'APK sur l'appareil connecté :

```bash
adb install -r target/dx/app_desktop/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
```

Le drapeau `-r` permet de réinstaller l'application si elle est déjà présente.

## Premier lancement et téléchargement du modèle

**Important** : Au premier lancement, l'application téléchargera automatiquement le modèle de langage Qwen2.5-3B-Instruct-Q4_K_M.gguf (~2 Go). 

### Points à noter

- Le téléchargement prendra plusieurs minutes selon la vitesse de votre connexion réseau
- L'appareil doit rester branché et avec l'écran allumé pendant le téléchargement
- Assurez-vous d'avoir une connexion réseau stable (WiFi recommandé)
- L'espace de stockage doit être suffisant (au minimum 3 Go disponibles)

## Vérification du fonctionnement sur l'appareil réel

Après l'installation et le lancement de l'application, veuillez vérifier visuellement les points suivants :

- [ ] L'application démarre sans crash
- [ ] **Écran de téléchargement** : Un écran s'affiche montrant la progression du téléchargement du modèle avec une barre de progression visible
- [ ] **Progression du téléchargement** : La barre de progression avance et affiche le pourcentage de téléchargement complété
- [ ] **Transition post-téléchargement** : Une fois le téléchargement terminé, l'application passe à l'écran de création (création de personnage/jeu)
- [ ] **Écran de création** : L'interface de création correspond à celle de la version desktop
- [ ] **Interface générale** : La navigation et l'interface utilisateur correspondent à la version desktop de Narrateur
- [ ] **Fonctionnalités de base** : Vous pouvez créer un personnage et naviguer dans l'interface sans crash

## Notes techniques

- L'APK est un build de debug ; elle contient des symboles de débogage et peut être plus volumineuse qu'une build de release
- Vous pouvez consulter les logs de l'appareil en utilisant `adb logcat` si vous rencontrez des problèmes
- Le modèle téléchargé sera stocké dans le stockage persistant de l'appareil et ne sera téléchargé qu'une seule fois

## Signalement de problèmes

Si vous rencontrez des problèmes lors de l'installation ou du lancement :

1. Vérifiez que `adb devices` affiche votre appareil
2. Consultez les logs de l'appareil avec `adb logcat | grep app_desktop`
3. Assurez-vous que le mode de débogage USB est bien activé
4. Essayez de réinstaller avec `-r` flag si l'application est déjà présente

---

Date de génération : 26 octobre 2026  
Cible : Android 8.0+ (API 26+)  
Architecture supportée : ARM64
