using System;
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

            return new
            {
                userName = Environment.UserName,
                machineName = Environment.MachineName,
                userSid = identity.User?.Value,
                authenticationType = identity.AuthenticationType,
                isAdministrator = principal.IsInRole(WindowsBuiltInRole.Administrator),
                uacEnabled = enableLua == null ? (bool?)null : Convert.ToInt32(enableLua) != 0,
                consentPromptBehaviorAdmin = consent == null ? (int?)null : Convert.ToInt32(consent),
                secureDesktopEnabled = secureDesktop == null ? (bool?)null : Convert.ToInt32(secureDesktop) != 0,
                source = "WindowsIdentity + HKLM UAC policy",
                note = "Contexte de sécurité du processus courant. Aucun privilège n’est accordé ou modifié par cette lecture."
            };
        }
    }
}
