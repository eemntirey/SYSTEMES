using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace Systemes.Native.Core
{
    /// <summary>
    /// Read-only Windows Update Agent query. This class never downloads or installs updates.
    /// </summary>
    public static class WindowsUpdateNative
    {
        public static object CheckRealUpdates()
        {
            object? session = null;
            object? searcher = null;
            object? searchResult = null;
            object? updateCollection = null;

            try
            {
                var sessionType = Type.GetTypeFromProgID("Microsoft.Update.Session", throwOnError: true);
                if (sessionType == null)
                    throw new InvalidOperationException("Windows Update Agent est indisponible sur cet appareil.");

                session = Activator.CreateInstance(sessionType)
                    ?? throw new InvalidOperationException("Impossible de créer une session Windows Update.");
                dynamic sessionObject = session;
                searcher = sessionObject.CreateUpdateSearcher();
                dynamic searcherObject = searcher;
                searcherObject.Online = true;

                // Search only: no download, installation, or reboot is initiated.
                searchResult = searcherObject.Search("IsInstalled=0 and IsHidden=0");
                dynamic resultObject = searchResult;
                updateCollection = resultObject.Updates;
                dynamic updates = updateCollection;
                int count = Convert.ToInt32(updates.Count);
                var available = new List<object>(count);

                for (int index = 0; index < count; index++)
                {
                    object? update = null;
                    try
                    {
                        update = updates.Item(index);
                        dynamic item = update;
                        var kbArticles = new List<string>();
                        try
                        {
                            foreach (var article in item.KBArticleIDs)
                                kbArticles.Add(Convert.ToString(article) ?? "");
                        }
                        catch { /* KBArticleIDs may be empty or unavailable. */ }

                        available.Add(new
                        {
                            title = Convert.ToString(item.Title) ?? "Mise à jour sans titre",
                            kbArticles,
                            severity = Convert.ToString(item.MsrcSeverity) ?? "",
                            downloaded = Convert.ToBoolean(item.IsDownloaded),
                            mandatory = Convert.ToBoolean(item.IsMandatory),
                            rebootRequired = Convert.ToBoolean(item.RebootRequired)
                        });
                    }
                    finally
                    {
                        ReleaseComObject(update);
                    }
                }

                int resultCode = Convert.ToInt32(resultObject.ResultCode);
                return new
                {
                    success = resultCode == 2,
                    resultCode,
                    updatesAvailable = count,
                    updates = available,
                    service = "Windows Update Agent (COM)",
                    searchedAtUtc = DateTime.UtcNow,
                    note = "Recherche Windows Update réelle terminée. Aucune mise à jour n’a été téléchargée ou installée."
                };
            }
            catch (Exception ex)
            {
                return new
                {
                    error = ex.GetBaseException().Message,
                    service = "Windows Update Agent (COM)",
                    updatesAvailable = (int?)null,
                    note = "La recherche Windows Update n’a pas abouti. Aucune modification du système n’a été effectuée."
                };
            }
            finally
            {
                ReleaseComObject(updateCollection);
                ReleaseComObject(searchResult);
                ReleaseComObject(searcher);
                ReleaseComObject(session);
            }
        }

        private static void ReleaseComObject(object? value)
        {
            if (value != null && Marshal.IsComObject(value))
            {
                try { Marshal.FinalReleaseComObject(value); }
                catch { /* Cleanup must not hide the original WUA result. */ }
            }
        }
    }
}
