# SYSTEMES Windows Host

Hôte WPF natif .NET 8 qui affiche le build React dans Microsoft Edge WebView2, sans Vite ni navigateur externe en production.

## Build local (Windows 11)

Depuis la racine du dépôt :

```powershell
npm ci
npm run build
dotnet build native/windows-host/SYSTEMES.WindowsHost.csproj -c Release
```

L'hôte copie le contenu de `dist/` dans `wwwroot/` et publie le moteur console `native/Windver/Core/SystemesNative.csproj` dans `native-engine/`.

## Exécution

Lancer `native/windows-host/bin/Release/net8.0-windows/SYSTEMES.WindowsHost.exe`.
Le runtime WebView2 Evergreen doit être installé sur Windows. Le moteur natif est démarré comme processus enfant et arrêté à la fermeture de l'hôte.

## Pont

La page transmet un message JSON via `window.chrome.webview.postMessage({ id, method, params })`. L'hôte le relaie au processus natif par stdin/stdout et renvoie la réponse avec `window.chrome.webview`'s `message` event.

Le canal est local au processus hôte : aucun port TCP localhost n'est ouvert. Les méthodes disponibles restent limitées à celles implémentées par NativeServer. Toute opération sensible doit être explicitement validée côté moteur.
