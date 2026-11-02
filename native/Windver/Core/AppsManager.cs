using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.Win32;

namespace Systemes.Native.Core
{
    /// <summary>
    /// Applications installées (clés Uninstall du registre, lecture seule).
    /// </summary>
    public static class AppsManager
    {
        private static readonly string[] UninstallRoots =
        {
            @"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            @"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        };

        public static object ReadInstalledApps(int maxItems = 400)
        {
            if (maxItems < 1) maxItems = 1;
            if (maxItems > 1000) maxItems = 1000;

            var apps = new Dictionary<string, object>(StringComparer.OrdinalIgnoreCase);

            void ScanHive(RegistryKey hive, string root)
            {
                using var key = hive.OpenSubKey(root);
                if (key == null) return;
                foreach (var subName in key.GetSubKeyNames())
                {
                    try
                    {
                        using var sub = key.OpenSubKey(subName);
                        if (sub == null) continue;
                        var displayName = sub.GetValue("DisplayName") as string;
                        if (string.IsNullOrWhiteSpace(displayName)) continue;
                        if (sub.GetValue("SystemComponent") is int sc && sc == 1) continue;
                        if (sub.GetValue("ParentKeyName") is string) continue;

                        var version = sub.GetValue("DisplayVersion") as string ?? "";
                        var publisher = sub.GetValue("Publisher") as string ?? "";
                        var installLocation = sub.GetValue("InstallLocation") as string ?? "";
                        var uninstallString = sub.GetValue("UninstallString") as string ?? "";
                        var installDate = sub.GetValue("InstallDate") as string ?? "";

                        var id = subName;
                        if (!apps.ContainsKey(id))
                        {
                            apps[id] = new
                            {
                                id,
                                name = displayName.Trim(),
                                version,
                                publisher,
                                installLocation,
                                hasUninstaller = !string.IsNullOrWhiteSpace(uninstallString),
                                installDate,
                            };
                        }
                    }
                    catch
                    {
                    }
                }
            }

            ScanHive(Registry.LocalMachine, UninstallRoots[0]);
            ScanHive(Registry.LocalMachine, UninstallRoots[1]);
            try { ScanHive(Registry.CurrentUser, UninstallRoots[0]); } catch { }

            var list = apps.Values
                .OrderBy(a => ((dynamic)a).name)
                .Take(maxItems)
                .ToArray();

            return new
            {
                apps = list,
                count = list.Length,
                truncated = apps.Count > maxItems,
                totalFound = apps.Count,
                source = "Registry Uninstall (HKLM + HKCU)",
                note = "Liste en lecture seule. Aucune désinstallation n’est déclenchée.",
            };
        }
    }
}
