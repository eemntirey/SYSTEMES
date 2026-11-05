using System;
using System.IO;
using System.Linq;
using System.Net.NetworkInformation;
using System.Security.Principal;
using System.Runtime.InteropServices;

namespace Systemes.Native.Core
{
    public static class NativeCapabilities
    {
        public static object Read()
        {
            var version = Environment.OSVersion.Version;
            var isAdministrator = false;
            try
            {
                using var identity = WindowsIdentity.GetCurrent();
                isAdministrator = new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);
            }
            catch
            {
                // Capability discovery must remain read-only and must not fail just because
                // the current token cannot be translated into a Windows principal.
            }

            var powershellPath = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.System),
                "WindowsPowerShell", "v1.0", "powershell.exe");
            bool? hasWifiAdapter = null;
            try
            {
                hasWifiAdapter = NetworkInterface.GetAllNetworkInterfaces()
                    .Any(adapter => adapter.NetworkInterfaceType == NetworkInterfaceType.Wireless80211);
            }
            catch
            {
                hasWifiAdapter = null;
                // A failed inventory is reported as unavailable, never as an invented adapter.
            }

            var isWindows10OrLater = version.Major >= 10;
            var capabilities = new[]
            {
                new
                {
                    id = "files",
                    state = "ready",
                    reason = "Les API .NET de fichiers Windows sont disponibles ; les ACL sont vérifiées à chaque opération.",
                    requiresAdministratorForSomeOperations = true
                },
                new
                {
                    id = "processes",
                    state = "ready",
                    reason = "L’inventaire des processus du compte courant est disponible ; certains processus protégés restent inaccessibles.",
                    requiresAdministratorForSomeOperations = true
                },
                new
                {
                    id = "terminal",
                    state = File.Exists(powershellPath) ? "ready" : "unsupported",
                    reason = File.Exists(powershellPath) ? "Windows PowerShell est présent." : "Windows PowerShell est introuvable sur ce système.",
                    requiresAdministratorForSomeOperations = false
                },
                new
                {
                    id = "settings",
                    state = isWindows10OrLater ? "ready" : "unsupported",
                    reason = isWindows10OrLater ? "Les URI ms-settings sont prises en charge." : "Les pages ms-settings nécessitent Windows 10 ou ultérieur.",
                    requiresAdministratorForSomeOperations = false
                },
                new
                {
                    id = "wifi",
                    state = hasWifiAdapter == true ? "ready" : hasWifiAdapter == false ? "unsupported" : "unavailable",
                    reason = hasWifiAdapter == true ? "Un adaptateur Wi-Fi a été détecté." : hasWifiAdapter == false ? "Aucun adaptateur Wi-Fi n’a été détecté." : "L’inventaire des adaptateurs Wi-Fi est indisponible.",
                    requiresAdministratorForSomeOperations = true
                },
                new
                {
                    id = "services",
                    state = "ready",
                    reason = "La lecture des services est disponible ; leur contrôle dépend des ACL Windows.",
                    requiresAdministratorForSomeOperations = true
                },
                new
                {
                    id = "security",
                    state = "ready",
                    reason = "Le contexte du jeton Windows courant peut être interrogé en lecture seule.",
                    requiresAdministratorForSomeOperations = false
                },
                new
                {
                    id = "audio",
                    state = "ready",
                    reason = "Les interfaces audio Windows sont accessibles ; la disponibilité des périphériques est vérifiée à la demande.",
                    requiresAdministratorForSomeOperations = false
                }
            };

            return new
            {
                platform = OperatingSystem.IsWindows() ? "windows" : "unsupported",
                osVersion = version.ToString(),
                osBuild = version.Build,
                architecture = RuntimeInformation.ProcessArchitecture.ToString(),
                isAdministrator,
                capabilities,
                checkedAtUtc = DateTime.UtcNow.ToString("O"),
                note = "Inventaire des capacités du système et du jeton courant ; aucune permission n’est modifiée ni accordée automatiquement."
            };
        }
    }
}
