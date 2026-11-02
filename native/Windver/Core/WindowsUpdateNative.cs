using System;
using System.Collections.Generic;
using System.Management;
using System.Reflection;
using System.Runtime.InteropServices;

namespace Systemes.Native.Core
{
    /// <summary>
    /// Recherche les mises à jour proposées par Windows Update Agent.
    /// Opération strictement en lecture : aucune mise à jour n'est téléchargée ni installée.
    /// </summary>
    public static class WindowsUpdateNative
    {
        private const int MaxReturnedUpdates = 50;

        public static object CheckRealUpdates()
        {
            object? session = null;
            object? searcher = null;
            object? searchResult = null;
            object? updates = null;

            try
            {
                bool serviceRunning = ReadWindowsUpdateServiceState();
                var sessionType = Type.GetTypeFromProgID("Microsoft.Update.Session", throwOnError: true)
                    ?? throw new InvalidOperationException("Windows Update Agent est indisponible sur cette machine.");

                session = Activator.CreateInstance(sessionType)
                    ?? throw new InvalidOperationException("Impossible de créer une session Windows Update Agent.");

                searcher = Invoke(session, "CreateUpdateSearcher")
                    ?? throw new InvalidOperationException("Windows Update Agent n'a pas créé de moteur de recherche.");

                // IsInstalled=0 and IsHidden=0 ne sélectionne que les mises à jour non installées et non masquées.
                searchResult = Invoke(searcher, "Search", "IsInstalled=0 and IsHidden=0 and Type='Software'")
                    ?? throw new InvalidOperationException("Windows Update Agent n'a retourné aucun résultat.");

                updates = GetProperty(searchResult, "Updates");
                var count = Convert.ToInt32(GetProperty(updates, "Count") ?? 0);
                var rebootRequired = Convert.ToBoolean(GetProperty(searchResult, "RebootRequired") ?? false);
                var items = new List<object>();

                for (var index = 0; index < Math.Min(count, MaxReturnedUpdates); index++)
                {
                    object? update = null;
                    try
                    {
                        update = GetIndexedProperty(updates, "Item", index);
                        if (update == null) continue;

                        var title = Convert.ToString(GetProperty(update, "Title")) ?? "Mise à jour sans titre";
                        var kb = GetProperty(update, "KBArticleIDs") as Array;
                        var kbIds = new List<string>();
                        if (kb != null)
                        {
                            foreach (var value in kb)
                            {
                                var id = Convert.ToString(value);
                                if (!string.IsNullOrWhiteSpace(id)) kbIds.Add(id);
                            }
                        }

                        items.Add(new
                        {
                            title,
                            kbArticleIds = kbIds,
                            isDownloaded = Convert.ToBoolean(GetProperty(update, "IsDownloaded") ?? false),
                            isMandatory = Convert.ToBoolean(GetProperty(update, "IsMandatory") ?? false),
                            rebootBehavior = Convert.ToInt32(GetProperty(update, "RebootBehavior") ?? 0),
                        });
                    }
                    finally
                    {
                        ReleaseCom(update);
                    }
                }

                return new
                {
                    success = true,
                    serviceRunning,
                    updatesAvailable = count > 0,
                    updateCount = count,
                    updates = items,
                    truncated = count > MaxReturnedUpdates,
                    rebootRequired,
                    checkedAtUtc = DateTime.UtcNow.ToString("O"),
                    source = "Windows Update Agent COM (Microsoft.Update.Session)",
                    note = "Recherche réelle en lecture seule. Aucun téléchargement ni installation n'a été lancé.",
                };
            }
            catch (Exception ex)
            {
                return new
                {
                    success = false,
                    serviceRunning = SafeReadServiceState(),
                    updatesAvailable = (bool?)null,
                    updateCount = (int?)null,
                    updates = Array.Empty<object>(),
                    rebootRequired = (bool?)null,
                    checkedAtUtc = DateTime.UtcNow.ToString("O"),
                    source = "Windows Update Agent COM",
                    error = ex.GetBaseException().Message,
                    note = "La recherche a échoué. Aucun changement Windows n'a été effectué.",
                };
            }
            finally
            {
                ReleaseCom(updates);
                ReleaseCom(searchResult);
                ReleaseCom(searcher);
                ReleaseCom(session);
            }
        }

        private static object? Invoke(object target, string method, params object[] args)
        {
            return target.GetType().InvokeMember(
                method,
                BindingFlags.InvokeMethod,
                binder: null,
                target: target,
                args: args);
        }

        private static object? GetIndexedProperty(object target, string property, params object[] indexes)
        {
            return target.GetType().InvokeMember(
                property,
                BindingFlags.GetProperty,
                binder: null,
                target: target,
                args: indexes);
        }

        private static object? GetProperty(object? target, string property)
        {
            if (target == null) return null;
            return target.GetType().InvokeMember(
                property,
                BindingFlags.GetProperty,
                binder: null,
                target: target,
                args: null);
        }

        private static bool ReadWindowsUpdateServiceState()
        {
            using var searcher = new ManagementObjectSearcher(
                "SELECT State FROM Win32_Service WHERE Name = 'wuauserv'");
            using var results = searcher.Get();
            foreach (ManagementObject item in results)
                return string.Equals(item["State"]?.ToString(), "Running", StringComparison.OrdinalIgnoreCase);
            return false;
        }

        private static bool? SafeReadServiceState()
        {
            try { return ReadWindowsUpdateServiceState(); }
            catch { return null; }
        }

        private static void ReleaseCom(object? value)
        {
            if (value != null && Marshal.IsComObject(value))
            {
                try { Marshal.FinalReleaseComObject(value); }
                catch { }
            }
        }
    }
}
