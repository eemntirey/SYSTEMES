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
            var services = new List<object>();
            foreach (ManagementObject item in searcher.Get())
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
                    protectedService = ProtectedServices.Contains(name),
                });
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
                throw new ArgumentException("Action de service non prise en charge (start|stop).", nameof(action));

            if (ProtectedServices.Contains(name))
                throw new InvalidOperationException(
                    $"Le service « {name} » est protégé : son contrôle peut rendre Windows instable.");

            var wmiPath = "Win32_Service.Name='" + name.Replace("'", "\\'") + "'";

            using var service = new ManagementObject(wmiPath);
            try
            {
                service.Get();
            }
            catch (ManagementException)
            {
                throw new InvalidOperationException($"Service Windows introuvable : « {name} ».");
            }

            var before = service["State"]?.ToString() ?? "Unknown";
            var displayName = service["DisplayName"]?.ToString() ?? name;
            var method = action == "start" ? "StartService" : "StopService";
            var expected = action == "start" ? "Running" : "Stopped";

            if (string.Equals(before, expected, StringComparison.OrdinalIgnoreCase))
            {
                return new
                {
                    changed = false,
                    name,
                    displayName,
                    state = before,
                    previousState = before,
                    action,
                    note = $"Le service était déjà {expected}.",
                    source = "WMI Win32_Service",
                };
            }

            object[] result = service.InvokeMethod(method, null) as object[]
                ?? throw new InvalidOperationException($"InvokeMethod {method} n’a renvoyé aucun code.");

            var code = Convert.ToUInt32(result[0]);
            if (code != 0 && !(action == "start" && code == 10) && !(action == "stop" && code is 5 or 17))
            {
                throw new InvalidOperationException(
                    $"Échec {action} de « {displayName} » ({name}) : {DescribeServiceResult(code)}.");
            }

            for (var attempt = 0; attempt < 30; attempt++)
            {
                service.Get();
                var state = service["State"]?.ToString() ?? "Unknown";
                if (string.Equals(state, expected, StringComparison.OrdinalIgnoreCase))
                {
                    return new
                    {
                        changed = true,
                        name,
                        displayName,
                        state,
                        previousState = before,
                        action,
                        wmiCode = code,
                        note = "État du service relu et confirmé par WMI.",
                        source = "WMI Win32_Service InvokeMethod",
                    };
                }
                Thread.Sleep(400);
            }

            service.Get();
            var finalState = service["State"]?.ToString() ?? "Unknown";
            throw new TimeoutException(
                $"Windows n’a pas confirmé « {expected} » pour {name} (état actuel : {finalState}).");
        }
    }
}
