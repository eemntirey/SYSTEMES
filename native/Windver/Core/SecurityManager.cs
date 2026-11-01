using System;
using System.Linq;
using System.Security.Principal;
using Microsoft.Win32;

namespace Systemes.Native.Core
{
    public static class SecurityManager
    {
        public static object ReadSecurityContext()
        {
            using var identity = WindowsIdentity.GetCurrent();
            var principal = new WindowsPrincipal(identity);
            var policyPath = @"HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System";
            var enableLua = Registry.GetValue(policyPath, "EnableLUA", null);
            var consent = Registry.GetValue(policyPath, "ConsentPromptBehaviorAdmin", null);
            var secureDesktop = Registry.GetValue(policyPath, "PromptOnSecureDesktop", null);
            var groups = identity.Groups?.Cast<IdentityReference>().Select(group =>
            {
                string name;
                try { name = group.Translate(typeof(NTAccount)).Value; }
                catch { name = group.Value; }
                return new { sid = group.Value, name };
            }).ToArray() ?? Array.Empty<object>();

            return new
            {
                userName = Environment.UserName,
                machineName = Environment.MachineName,
                userSid = identity.User?.Value,
                authenticationType = identity.AuthenticationType,
                isAuthenticated = identity.IsAuthenticated,
                isSystem = identity.IsSystem,
                isGuest = identity.IsGuest,
                impersonationLevel = identity.ImpersonationLevel.ToString(),
                isAdministrator = principal.IsInRole(WindowsBuiltInRole.Administrator),
                groups,
                groupCount = groups.Length,
                uacEnabled = enableLua == null ? (bool?)null : Convert.ToInt32(enableLua) != 0,
                consentPromptBehaviorAdmin = consent == null ? (int?)null : Convert.ToInt32(consent),
                secureDesktopEnabled = secureDesktop == null ? (bool?)null : Convert.ToInt32(secureDesktop) != 0,
                source = "WindowsIdentity + WindowsPrincipal + HKLM UAC policy",
                note = "Groupes et contexte du jeton Windows courant. Lecture seule : aucun privilège n’est accordé, retiré ou élevé."
            };
        }
    }
}
