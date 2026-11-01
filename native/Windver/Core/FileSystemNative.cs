using System;
using System.IO;
using System.Linq;
using System.Text;

namespace Systemes.Native.Core
{
    public class FileSystemNative
    {
        private const int MaxTextFileBytes = 1024 * 1024;

        private static string ResolvePath(string path)
        {
            if (string.IsNullOrWhiteSpace(path))
                return Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
            return Path.GetFullPath(path);
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

        public static object DeleteFile(string path)
        {
            var resolvedPath = ResolvePath(path);
            if (Directory.Exists(resolvedPath))
                throw new InvalidOperationException("La suppression des dossiers est désactivée pour éviter toute suppression récursive.");
            if (!File.Exists(resolvedPath))
                throw new FileNotFoundException("Le fichier à supprimer n’existe plus.", resolvedPath);

            File.Delete(resolvedPath);
            return new { deleted = true, path = resolvedPath.Replace('\\', '/'), note = "Fichier supprimé du disque" };
        }
    }
}