using System;
using System.Collections.Generic;
using System.Linq;
using System.Management;
using System.Text.RegularExpressions;
using System.Threading;

namespace Systemes.Native.Core
{
    /// <summary>
    /// Lecture et contrôle (start/stop) des services Windows via WMI Win32_Service.
    /// Les services critiques sont refusés. Les droits admin sont souvent requis.
    /// </summary>
    public static class ServiceManager
    {
        private static readonly HashSet<string> ProtectedServices = new(StringComparer.OrdinalIgnoreCase)
        {
            "RpcSs", "DcomLaunch", "RpcEptMapper", "EventLog", "PlugPlay",
            "Power", "SamSs", "LSM", "Winmgmt", "Schedule", "CryptSvc",
            "ProfSvc", "UserManager", "BFE", "MpsSvc", "LanmanServer",
            "WinDefend", "SecurityHealthService", "wuauserv", "BITS",
            "Dhcp", "Dnscache", "NlaSvc", "nsi", "BrokerInfrastructure"
        };

        private static string DescribeServiceResult(uint code) => code switch
        {
            0 => "Succès",
            1 => "Non pris en charge",
            2 => "Accès refusé (lancez SYSTEMES en administrateur)",
            3 => "Services dépendants en cours d’exécution",
            4 => "Service de contrôle invalide",
            5 => "Service ne peut pas accepter de contrôle",
            6 => "Service non actif",
            7 => "Délai de service dépassé",
            8 => "Échec inconnu",
            9 => "Chemin introuvable",
            10 => "Service déjà en cours d’exécution",
            11 => "Base de données de service verrouillée",
            12 => "Dépendance de service supprimée",
            13 => "Échec de dépendance de service",
            14 => "Service désactivé",
            15 => "Échec de connexion au service",
            16 => "Échec de consommation du service",
            17 => "Service déjà arrêté / non démarré",
            18 => "Échec de connexion",
            19 => "Compte de service invalide",
            20 => "Service marqué pour suppression",
            21 => "Thread non existant",
            22 => "Sécurité interactive",
            _ => $"Code WMI {code}"
        };

        public static object ReadServices()
        {
            using var searcher = new ManagementObjectSearcher(
                "SELECT Name, DisplayName, State, StartMode, StartName, ProcessId, PathName FROM Win32_Service");
            using var results = searcher.Get();
            var services = new List<object>();
            foreach (ManagementObject item in results)
            {
                using (item)
                {
                    var name = item["Name"]?.ToString() ?? "";
                    services.Add(new
                    {
                        name,
                        displayName = item["DisplayName"]?.ToString() ?? "",
                        state = item["State"]?.ToString() ?? "Unknown",
                        startMode = item["StartMode"]?.ToString() ?? "Unknown",
                        account = item["StartName"]?.ToString() ?? "",
                        processId = Convert.ToUInt32(item["ProcessId"] ?? 0),
                        executablePath = item["PathName"]?.ToString() ?? "",
                        isProtected = ProtectedServices.Contains(name)
                    });
                }
            }

            return new
            {
                services = services
                    .OrderBy(item => item.GetType().GetProperty("displayName")?.GetValue(item)?.ToString() ?? "")
                    .ToArray(),
                count = services.Count,
                source = "WMI Win32_Service",
                note = "Inventaire en lecture seule. Contrôle via services/control (start|stop).",
            };
        }

        public static object ControlService(string name, string action)
        {
            if (string.IsNullOrWhiteSpace(name) || !Regex.IsMatch(name, "^[A-Za-z0-9_.-]{1,128}$"))
                throw new ArgumentException("Nom de service Windows invalide.", nameof(name));

            action = (action ?? "").Trim().ToLowerInvariant();
            if (action is not ("start" or "stop"))
                throw new ArgumentException("Action de service non prise en charge.", nameof(action));

            string currentState;
            using (var lookup = new ManagementObjectSearcher(
                "SELECT Name, State FROM Win32_Service WHERE Name='" + name + "'"))
            using (var results = lookup.Get())
            {
                var service = results.Cast<ManagementObject>().FirstOrDefault();
                if (service == null) throw new InvalidOperationException("Service Windows introuvable.");
                using (service)
                {
                    currentState = service["State"]?.ToString() ?? "Unknown";
                }
            }

            if (action == "stop" && ProtectedServices.Contains(name))
                throw new InvalidOperationException("Ce service est protégé car son arrêt peut rendre Windows instable ou inaccessible.");

            var expectedState = action == "start" ? "Running" : "Stopped";
            if (string.Equals(currentState, expectedState, StringComparison.OrdinalIgnoreCase))
                return new { changed = false, name, state = currentState, action, note = "Le service possède déjà l’état demandé." };

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
            var outputTask = process.StandardOutput.ReadToEndAsync();
            var errorTask = process.StandardError.ReadToEndAsync();
            if (!process.WaitForExit(10000))
            {
                try { process.Kill(entireProcessTree: true); } catch { }
                throw new TimeoutException("La commande de service a dépassé 10 secondes.");
            }
            var output = outputTask.GetAwaiter().GetResult();
            var error = errorTask.GetAwaiter().GetResult();
            if (process.ExitCode != 0)
            {
                var detail = string.IsNullOrWhiteSpace(error) ? output.Trim() : error.Trim();
                throw new InvalidOperationException(string.IsNullOrWhiteSpace(detail)
                    ? $"SC.exe a refusé l’action « {action} » sur « {name} » (code {process.ExitCode}). Vérifiez les droits et l’état du service."
                    : detail);
            }

            for (var attempt = 0; attempt < 20; attempt++)
            {
                using var query = new ManagementObjectSearcher(
                    "SELECT State FROM Win32_Service WHERE Name='" + name + "'");
                using var current = query.Get();
                foreach (ManagementObject item in current)
                {
                    using (item)
                    {
                        var state = item["State"]?.ToString() ?? "Unknown";
                        if (string.Equals(state, expectedState, StringComparison.OrdinalIgnoreCase))
                            return new { changed = true, name, state, action, note = "État du service relu et confirmé par WMI." };
                    }
                }
                Thread.Sleep(500);
            }

            throw new TimeoutException($"Le service « {name} » n’a pas atteint l’état « {expectedState} » dans le délai imparti.");
        }
    }
}
