using System;
using System.IO;

namespace Systemes.Native.Core
{
    public class FileSystemNative
    {
        public static object ReadRealFile(string path)
        {
            try
            {
                if (!File.Exists(path) && !Directory.Exists(path))
                    return new { exists = false, path, note = "Fichier ou dossier non trouvé" };

                var info = new FileInfo(path);
                return new
                {
                    exists = true,
                    path,
                    name = info.Name,
                    length = info.Length,
                    isDirectory = (info.Attributes & FileAttributes.Directory) == FileAttributes.Directory,
                    created = info.CreationTime,
                    modified = info.LastWriteTime
                };
            }
            catch (Exception ex)
            {
                return new { exists = false, path, error = ex.Message, note = "Accès refusé ou erreur système" };
            }
        }

        public static object CreateFile(string path, string content)
        {
            try
            {
                File.WriteAllText(path, content);
                return new { created = true, path, note = "Fichier créé réellement sur le disque" };
            }
            catch (Exception ex)
            {
                return new { created = false, path, error = ex.Message, note = "Création échouée" };
            }
        }

        public static object DeleteFile(string path)
        {
            try
            {
                if (Directory.Exists(path))
                    Directory.Delete(path, true);
                else
                    File.Delete(path);
                return new { deleted = true, path, note = "Fichier supprimé réellement" };
            }
            catch (Exception ex)
            {
                return new { deleted = false, path, error = ex.Message, note = "Suppression non autorisée" };
            }
        }
    }
}
