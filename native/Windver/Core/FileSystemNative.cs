using System;
using System.IO;
using System.Linq;
using System.Text;
using Microsoft.VisualBasic.FileIO;
using System.Diagnostics;

namespace Systemes.Native.Core
{
    public class FileSystemNative
    {
        private const int MaxTextFileBytes = 1024 * 1024;

        private static string ResolvePath(string path)
        {
            if (string.IsNullOrWhiteSpace(path))
                path = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
            if (path.IndexOf('\0') >= 0 || path.Length > 32767)
                throw new ArgumentException("Chemin vide, invalide ou trop long.", nameof(path));
            if (path.StartsWith(@"\\.\", StringComparison.OrdinalIgnoreCase)
                || path.StartsWith(@"\\?\", StringComparison.OrdinalIgnoreCase))
                throw new ArgumentException("Les espaces de noms de périphériques Windows ne sont pas autorisés.", nameof(path));
            if (path.Length > 2 && path[1] == ':' && path.IndexOf(':', 2) >= 0)
                throw new ArgumentException("Les flux de données alternatifs NTFS ne sont pas autorisés.", nameof(path));

            var fullPath = Path.GetFullPath(path);
            var root = Path.GetPathRoot(fullPath);
            if (string.IsNullOrWhiteSpace(root))
                throw new ArgumentException("Le chemin doit appartenir à un volume ou partage valide.", nameof(path));

            // Ne jamais suivre silencieusement un lien symbolique/jonction pendant une opération
            // sur le système de fichiers : chaque composant existant est vérifié depuis la racine.
            var current = root;
            var relative = fullPath.Substring(root.Length);
            foreach (var segment in relative.Split(new[] { Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar },
                         StringSplitOptions.RemoveEmptyEntries))
            {
                current = Path.Combine(current, segment);
                if (!File.Exists(current) && !Directory.Exists(current))
                    continue;
                FileAttributes attributes;
                try { attributes = File.GetAttributes(current); }
                catch (Exception ex) { throw new IOException("Impossible de vérifier la sécurité du chemin.", ex); }
                if ((attributes & FileAttributes.ReparsePoint) != 0)
                    throw new UnauthorizedAccessException("Les liens symboliques et points de réanalyse sont refusés pour cette opération.");
            }
            return fullPath;
        }

        public static object ReadKnownLocations()
        {
            var home = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
            var root = Path.GetPathRoot(home) ?? home;
            var locations = new[]
            {
                new { name = "Accueil", path = home, icon = "home" },
                new { name = "Bureau", path = Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory), icon = "monitor" },
                new { name = "Documents", path = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments), icon = "folder" },
                new { name = "Téléchargements", path = Path.Combine(home, "Downloads"), icon = "download" },
                new { name = "Images", path = Environment.GetFolderPath(Environment.SpecialFolder.MyPictures), icon = "image" },
                new { name = "Disque système", path = root, icon = "pc" }
            };
            return new
            {
                home = home.Replace('\\', '/'),
                locations = locations
                    .Where(item => !string.IsNullOrWhiteSpace(item.path) && Directory.Exists(item.path))
                    .Select(item => new { item.name, path = item.path.Replace('\\', '/'), item.icon })
                    .ToArray(),
                note = "Emplacements Windows résolus depuis le profil de l’utilisateur"
            };
        }

        public static object ListDirectory(string path)
        {
            var resolvedPath = ResolvePath(path);
            if (!Directory.Exists(resolvedPath))
                throw new DirectoryNotFoundException($"Dossier introuvable : {resolvedPath}");

            var directory = new DirectoryInfo(resolvedPath);
            var items = directory.EnumerateFileSystemInfos()
                .Select(info =>
                {
                    var isDirectory = (info.Attributes & FileAttributes.Directory) == FileAttributes.Directory;
                    long? length = null;
                    if (!isDirectory)
                    {
                        try { length = ((FileInfo)info).Length; }
                        catch { length = null; }
                    }
                    return new
                    {
                        id = info.FullName,
                        name = info.Name,
                        type = isDirectory ? "folder" : "file",
                        path = info.FullName.Replace('\\', '/'),
                        sizeBytes = length,
                        modified = info.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss"),
                        extension = isDirectory ? null : info.Extension.TrimStart('.')
                    };
                })
                .ToArray();

            return new
            {
                path = directory.FullName.Replace('\\', '/'),
                items,
                count = items.Length,
                note = "Contenu réel du dossier Windows"
            };
        }

        public static object ReadRealFile(string path)
        {
            var resolvedPath = ResolvePath(path);
            if (!File.Exists(resolvedPath) && !Directory.Exists(resolvedPath))
                return new { exists = false, path = resolvedPath, note = "Fichier ou dossier non trouvé" };

            if (Directory.Exists(resolvedPath))
            {
                var directory = new DirectoryInfo(resolvedPath);
                return new
                {
                    exists = true,
                    path = directory.FullName.Replace('\\', '/'),
                    name = directory.Name,
                    length = 0L,
                    isDirectory = true,
                    created = directory.CreationTime,
                    modified = directory.LastWriteTime
                };
            }

            var info = new FileInfo(resolvedPath);
            return new
            {
                exists = true,
                path = info.FullName.Replace('\\', '/'),
                name = info.Name,
                length = info.Length,
                isDirectory = false,
                created = info.CreationTime,
                modified = info.LastWriteTime
            };
        }

        public static object ReadTextFile(string path)
        {
            var resolvedPath = ResolvePath(path);
            if (!File.Exists(resolvedPath))
                throw new FileNotFoundException("Fichier introuvable.", resolvedPath);

            var info = new FileInfo(resolvedPath);
            if (info.Length > MaxTextFileBytes)
                throw new InvalidOperationException("Aperçu désactivé : le fichier dépasse la limite de 1 Mio.");

            var content = File.ReadAllText(resolvedPath, Encoding.UTF8);
            return new { path = info.FullName.Replace('\\', '/'), content, length = info.Length, modified = info.LastWriteTime, note = "Contenu texte lu sur le disque" };
        }

        public static object CreateFile(string path, string content)
        {
            var resolvedPath = ResolvePath(path);
            if (Encoding.UTF8.GetByteCount(content) > MaxTextFileBytes)
                throw new InvalidOperationException("La création est limitée aux fichiers texte de 1 Mio.");

            var parent = Path.GetDirectoryName(resolvedPath);
            if (string.IsNullOrWhiteSpace(parent) || !Directory.Exists(parent))
                throw new DirectoryNotFoundException("Le dossier parent n’existe pas.");

            using (var stream = new FileStream(resolvedPath, FileMode.CreateNew, FileAccess.Write, FileShare.None))
            using (var writer = new StreamWriter(stream, new UTF8Encoding(false)))
                writer.Write(content);

            return new { created = true, path = resolvedPath.Replace('\\', '/'), note = "Fichier créé sans écraser un fichier existant" };
        }

        public static object CreateDirectory(string path)
        {
            var resolvedPath = ResolvePath(path);
            if (Directory.Exists(resolvedPath) || File.Exists(resolvedPath))
                throw new IOException("Un élément portant déjà ce nom existe.");

            Directory.CreateDirectory(resolvedPath);
            return new { created = true, path = resolvedPath.Replace('\\', '/'), note = "Dossier créé sur le disque" };
        }

        public static object WriteTextFile(string path, string content)
        {
            var resolvedPath = ResolvePath(path);
            if (!File.Exists(resolvedPath))
                throw new FileNotFoundException("Le fichier à enregistrer n’existe plus.", resolvedPath);
            if (Encoding.UTF8.GetByteCount(content) > MaxTextFileBytes)
                throw new InvalidOperationException("L’enregistrement est limité aux fichiers texte de 1 Mio.");

            File.WriteAllText(resolvedPath, content, new UTF8Encoding(false));
            return new { written = true, path = resolvedPath.Replace('\\', '/'), note = "Fichier texte enregistré sur le disque" };
        }


        public static object RenameEntry(string path, string newName)
        {
            if (string.IsNullOrWhiteSpace(newName))
                throw new ArgumentException("Le nouveau nom est obligatoire.", nameof(newName));
            if (newName != newName.Trim() || newName.EndsWith(".", StringComparison.Ordinal) ||
                newName.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0 ||
                newName.Contains('/') || newName.Contains('\\'))
                throw new ArgumentException("Le nom contient des caractères interdits ou des espaces/points finaux.", nameof(newName));

            var reservedBaseName = newName.Split('.')[0].TrimEnd('.').ToUpperInvariant();
            if (new[] { "CON", "PRN", "AUX", "NUL" }.Contains(reservedBaseName) ||
                System.Text.RegularExpressions.Regex.IsMatch(reservedBaseName, @"^(COM|LPT)[1-9]$"))
                throw new ArgumentException("Ce nom est réservé par Windows.", nameof(newName));

            var sourcePath = ResolvePath(path);
            var parent = Path.GetDirectoryName(sourcePath);
            if (string.IsNullOrWhiteSpace(parent))
                throw new InvalidOperationException("La racine d’un volume ne peut pas être renommée.");
            if (!File.Exists(sourcePath) && !Directory.Exists(sourcePath))
                throw new FileNotFoundException("L’élément à renommer n’existe plus.", sourcePath);

            var destinationPath = Path.Combine(parent, newName);
            if (string.Equals(sourcePath, destinationPath, StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException("Le nouveau nom est identique au nom actuel.");
            if (File.Exists(destinationPath) || Directory.Exists(destinationPath))
                throw new IOException("Un fichier ou dossier portant déjà ce nom existe.");

            var isDirectory = Directory.Exists(sourcePath);
            if (isDirectory)
                Directory.Move(sourcePath, destinationPath);
            else
                File.Move(sourcePath, destinationPath);

            return new
            {
                renamed = true,
                isDirectory,
                oldPath = sourcePath.Replace('\\', '/'),
                path = destinationPath.Replace('\\', '/'),
                name = newName,
                note = "Renommage effectué dans le même dossier sans écraser de destination existante"
            };
        }

        private static void CopyDirectoryTree(string sourcePath, string destinationPath)
        {
            Directory.CreateDirectory(destinationPath);
            foreach (var entry in new DirectoryInfo(sourcePath).EnumerateFileSystemInfos())
            {
                var attributes = File.GetAttributes(entry.FullName);
                if ((attributes & FileAttributes.ReparsePoint) != 0)
                    throw new UnauthorizedAccessException("La copie récursive refuse les liens symboliques et points de réanalyse.");
                var target = Path.Combine(destinationPath, entry.Name);
                if ((attributes & FileAttributes.Directory) != 0)
                    CopyDirectoryTree(entry.FullName, target);
                else
                    File.Copy(entry.FullName, target, overwrite: false);
            }
        }

        private static void CopyDirectoryTreeSafely(string sourcePath, string destinationPath)
        {
            try
            {
                CopyDirectoryTree(sourcePath, destinationPath);
            }
            catch
            {
                // Éliminer les copies partielles créées par cette opération avant de remonter l'erreur.
                try
                {
                    if (Directory.Exists(destinationPath))
                        Directory.Delete(destinationPath, recursive: true);
                }
                catch { /* Conserver l'erreur initiale ; aucun écrasement de source. */ }
                throw;
            }
        }

        private static void ValidateTransferDestination(string sourcePath, string destinationPath)
        {
            if (string.Equals(sourcePath, destinationPath, StringComparison.OrdinalIgnoreCase))
                throw new IOException("La source et la destination sont identiques.");
            var parent = Path.GetDirectoryName(destinationPath);
            if (string.IsNullOrWhiteSpace(parent) || !Directory.Exists(parent))
                throw new DirectoryNotFoundException("Le dossier de destination n’existe pas.");
            if (File.Exists(destinationPath) || Directory.Exists(destinationPath))
                throw new IOException("La destination existe déjà ; aucun écrasement automatique n’est autorisé.");
            if (Directory.Exists(sourcePath))
            {
                var sourcePrefix = Path.TrimEndingDirectorySeparator(sourcePath) + Path.DirectorySeparatorChar;
                if (destinationPath.StartsWith(sourcePrefix, StringComparison.OrdinalIgnoreCase))
                    throw new IOException("Un dossier ne peut pas être copié ou déplacé à l’intérieur de lui-même.");
            }
        }

        public static object CopyEntry(string source, string destination)
        {
            var sourcePath = ResolvePath(source);
            var destinationPath = ResolvePath(destination);
            if (!File.Exists(sourcePath) && !Directory.Exists(sourcePath))
                throw new FileNotFoundException("L’élément source n’existe plus.", sourcePath);
            ValidateTransferDestination(sourcePath, destinationPath);

            var isDirectory = Directory.Exists(sourcePath);
            if (isDirectory)
                CopyDirectoryTreeSafely(sourcePath, destinationPath);
            else
                File.Copy(sourcePath, destinationPath, overwrite: false);

            return new
            {
                copied = true,
                isDirectory,
                source = sourcePath.Replace('\\', '/'),
                destination = destinationPath.Replace('\\', '/'),
                note = "Copie Windows effectuée sans écrasement ; les liens symboliques sont refusés."
            };
        }

        public static object MoveEntry(string source, string destination)
        {
            var sourcePath = ResolvePath(source);
            var destinationPath = ResolvePath(destination);
            if (!File.Exists(sourcePath) && !Directory.Exists(sourcePath))
                throw new FileNotFoundException("L’élément source n’existe plus.", sourcePath);
            ValidateTransferDestination(sourcePath, destinationPath);

            var isDirectory = Directory.Exists(sourcePath);
            var sameVolume = string.Equals(Path.GetPathRoot(sourcePath), Path.GetPathRoot(destinationPath), StringComparison.OrdinalIgnoreCase);
            if (sameVolume)
            {
                if (isDirectory)
                    Directory.Move(sourcePath, destinationPath);
                else
                    File.Move(sourcePath, destinationPath);
            }
            else
            {
                if (isDirectory)
                {
                    CopyDirectoryTreeSafely(sourcePath, destinationPath);
                    Directory.Delete(sourcePath, recursive: true);
                }
                else
                {
                    File.Copy(sourcePath, destinationPath, overwrite: false);
                    File.Delete(sourcePath);
                }
            }

            return new
            {
                moved = true,
                isDirectory,
                source = sourcePath.Replace('\\', '/'),
                destination = destinationPath.Replace('\\', '/'),
                note = "Déplacement Windows effectué sans écrasement."
            };
        }

        public static object OpenRecycleBin()
        {
            var start = new ProcessStartInfo
            {
                FileName = "explorer.exe",
                UseShellExecute = true,
                Arguments = "shell:RecycleBinFolder"
            };
            Process.Start(start);
            return new { opened = true, target = "shell:RecycleBinFolder", note = "Corbeille Windows ouverte dans l’Explorateur système." };
        }

        public static object DeleteFile(string path)
        {
            var resolvedPath = ResolvePath(path);
            if (!File.Exists(resolvedPath) && !Directory.Exists(resolvedPath))
                throw new FileNotFoundException("L’élément à supprimer n’existe plus.", resolvedPath);

            var root = Path.GetPathRoot(resolvedPath);
            if (string.Equals(resolvedPath.TrimEnd(Path.DirectorySeparatorChar), root?.TrimEnd(Path.DirectorySeparatorChar), StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException("La racine d’un volume ne peut pas être envoyée à la corbeille.");

            if (Directory.Exists(resolvedPath))
                FileSystem.DeleteDirectory(resolvedPath, UIOption.OnlyErrorDialogs, RecycleOption.SendToRecycleBin);
            else
                FileSystem.DeleteFile(resolvedPath, UIOption.OnlyErrorDialogs, RecycleOption.SendToRecycleBin);

            return new { deleted = true, recycled = true, path = resolvedPath.Replace('\\', '/'), note = "Élément déplacé vers la Corbeille Windows" };
        }

        public static object ReadPermissions(string path)
        {
            var resolvedPath = ResolvePath(path);
            if (!File.Exists(resolvedPath) && !Directory.Exists(resolvedPath))
                throw new FileNotFoundException("L’élément à examiner n’existe plus.", resolvedPath);

            var start = new ProcessStartInfo
            {
                FileName = "icacls.exe",
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true
            };
            start.ArgumentList.Add(resolvedPath);
            using var process = Process.Start(start) ?? throw new InvalidOperationException("Windows n’a pas démarré icacls.exe.");
            var outputTask = process.StandardOutput.ReadToEndAsync();
            var errorTask = process.StandardError.ReadToEndAsync();
            if (!process.WaitForExit(10000))
            {
                try { process.Kill(entireProcessTree: true); } catch { }
                throw new TimeoutException("La lecture des permissions Windows a dépassé 10 secondes.");
            }
            Task.WaitAll(outputTask, errorTask);
            if (process.ExitCode != 0)
                throw new InvalidOperationException(string.IsNullOrWhiteSpace(errorTask.Result) ? "icacls.exe n’a pas pu lire les permissions." : errorTask.Result.Trim());

            return new
            {
                path = resolvedPath.Replace('\\', '/'),
                acl = outputTask.Result.Trim(),
                source = "icacls.exe",
                note = "Liste de contrôle d’accès (ACL) réellement lue depuis Windows ; aucune permission n’a été modifiée."
            };
        }
    }
}