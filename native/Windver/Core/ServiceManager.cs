using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Management;
using System.Text.RegularExpressions;
using System.Threading;

namespace Systemes.Native.Core
{
    public static class ServiceManager
    {
        private static readonly HashSet<string> ProtectedServices = new(StringComparer.OrdinalIgnoreCase)
        {
            "RpcSs", "DcomLaunch", "RpcEptMapper", "EventLog", "PlugPlay",
            "Power", "SamSs", "LSM", "Winmgmt", "Schedule", "CryptSvc",
            "ProfSvc", "UserManager", "BFE", "MpsSvc", "LanmanServer"
        };

        public static object ReadServices()
        {
            using var searcher = new ManagementObjectSearcher(
                "SELECT Name, DisplayName, State, StartMode, StartName, ProcessId, PathName FROM Win32_Service");
            var services = new List<object>();
            foreach (ManagementObject item in searcher.Get())
            {
                services.Add(new
                {
                    name = item["Name"]?.ToString() ?? "",
                    displayName = item["DisplayName"]?.ToString() ?? "",
                    state = item["State"]?.ToString() ?? "Unknown",
                    startMode = item["StartMode"]?.ToString() ?? "Unknown",
                    account = item["StartName"]?.ToString() ?? "",
                    processId = Convert.ToUInt32(item["ProcessId"] ?? 0),
                    executablePath = item["PathName"]?.ToString() ?? ""
                });
            }

            return new
            {
                services = services.OrderBy(item => item.GetType().GetProperty("displayName")?.GetValue(item)?.ToString()).ToArray(),
                count = services.Count,
                source = "WMI Win32_Service",
                note = "Services Windows lus en temps réel. Aucun état n’est modifié."
            };
        }

        public static object ControlService(string name, string action)
        {
            if (string.IsNullOrWhiteSpace(name) || !Regex.IsMatch(name, "^[A-Za-z0-9_.-]{1,128}$"))
                throw new ArgumentException("Nom de service Windows invalide.", nameof(name));
            if (action is not ("start" or "stop"))
                throw new ArgumentException("Action de service non prise en charge.", nameof(action));
            if (ProtectedServices.Contains(name))
                throw new InvalidOperationException("Ce service est protégé car son arrêt peut rendre Windows instable ou inaccessible.");

            using (var lookup = new ManagementObjectSearcher(
                "SELECT Name, State FROM Win32_Service WHERE Name='" + name + "'"))
            using (var results = lookup.Get())
            {
                if (results.Count == 0) throw new InvalidOperationException("Service Windows introuvable.");
            }

            var start = new ProcessStartInfo
            {
                FileName = "sc.exe",
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true
            };
            start.ArgumentList.Add(action);
            start.ArgumentList.Add(name);
            using var process = Process.Start(start) ?? throw new InvalidOperationException("Windows n’a pas démarré sc.exe.");
            var output = process.StandardOutput.ReadToEnd();
            var error = process.StandardError.ReadToEnd();
            if (!process.WaitForExit(10000))
            {
                try { process.Kill(entireProcessTree: true); } catch { }
                throw new TimeoutException("La commande de service a dépassé 10 secondes.");
            }
            if (process.ExitCode != 0)
                throw new InvalidOperationException(string.IsNullOrWhiteSpace(error) ? output.Trim() : error.Trim());

            var expectedState = action == "start" ? "Running" : "Stopped";
            for (var attempt = 0; attempt < 20; attempt++)
            {
                using var query = new ManagementObjectSearcher(
                    "SELECT State FROM Win32_Service WHERE Name='" + name + "'");
                foreach (ManagementObject item in query.Get())
                {
                    var state = item["State"]?.ToString() ?? "Unknown";
                    if (string.Equals(state, expectedState, StringComparison.OrdinalIgnoreCase))
                        return new { changed = true, name, state, action, note = "État du service relu et confirmé par WMI." };
                }
                Thread.Sleep(500);
            }

            throw new TimeoutException("Windows n’a pas confirmé le nouvel état du service dans le délai prévu.");
        }
    }
}
