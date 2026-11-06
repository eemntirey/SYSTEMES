using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace Systemes.Native.Core
{
    public static class SettingsNative
    {
        private static readonly IReadOnlyDictionary<string, string> AllowedPages =
            new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
            {
                ["display"] = "ms-settings:display",
                ["night-light"] = "ms-settings:nightlight",
                ["accessibility-display"] = "ms-settings:easeofaccess-display",
                ["sound"] = "ms-settings:sound",
                ["notifications"] = "ms-settings:notifications",
                ["quiet-hours"] = "ms-settings:quiethours",
                ["battery"] = "ms-settings:batterysaver",
                ["power"] = "ms-settings:powersleep",
                ["bluetooth"] = "ms-settings:bluetooth",
                ["printers"] = "ms-settings:printers",
                ["network-status"] = "ms-settings:network-status",
                ["network-ethernet"] = "ms-settings:network-ethernet",
                ["network-vpn"] = "ms-settings:network-vpn",
                ["airplane-mode"] = "ms-settings:network-airplanemode",
                ["mobile-hotspot"] = "ms-settings:network-mobilehotspot",
                ["data-usage"] = "ms-settings:datausage",
                ["proxy"] = "ms-settings:network-proxy",
                ["taskbar"] = "ms-settings:taskbar",
                ["lock-screen"] = "ms-settings:lockscreen",
                ["optional-features"] = "ms-settings:optionalfeatures",
                ["default-apps"] = "ms-settings:defaultapps",
                ["accounts"] = "ms-settings:yourinfo",
                ["sign-in-options"] = "ms-settings:signinoptions",
                ["accounts-sync"] = "ms-settings:sync",
                ["work-access"] = "ms-settings:workplace",
                ["other-users"] = "ms-settings:otherusers",
                ["date-time"] = "ms-settings:dateandtime",
                ["region"] = "ms-settings:regionformatting",
                ["language"] = "ms-settings:regionlanguage",
                ["gaming"] = "ms-settings:gaming-gamebar",
                ["game-mode"] = "ms-settings:gaming-gamemode",
                ["graphics"] = "ms-settings:display-advancedgraphics",
                ["accessibility-magnifier"] = "ms-settings:easeofaccess-magnifier",
                ["accessibility-color-filters"] = "ms-settings:easeofaccess-colorfilter",
                ["accessibility-captions"] = "ms-settings:easeofaccess-closedcaptioning",
                ["accessibility-keyboard"] = "ms-settings:easeofaccess-keyboard",
                ["accessibility-mouse"] = "ms-settings:easeofaccess-mouse",
                ["accessibility-speech"] = "ms-settings:easeofaccess-speech",
                ["windows-security"] = "ms-settings:windowsdefender",
                ["device-encryption"] = "ms-settings:deviceencryption",
                ["privacy"] = "ms-settings:privacy",
                ["privacy-diagnostics"] = "ms-settings:privacy-feedback",
                ["privacy-activity"] = "ms-settings:privacy-activityhistory",
                ["privacy-location"] = "ms-settings:privacy-location",
                ["privacy-camera"] = "ms-settings:privacy-webcam",
                ["privacy-microphone"] = "ms-settings:privacy-microphone",
                ["privacy-notifications"] = "ms-settings:privacy-notifications",
                ["windows-update"] = "ms-settings:windowsupdate",
                ["windows-update-hours"] = "ms-settings:windowsupdate-activehours",
                ["windows-update-optional"] = "ms-settings:windowsupdate-optionalupdates",
                ["windows-insider"] = "ms-settings:windowsinsider",
                ["about"] = "ms-settings:about",
                ["pen-touch"] = "ms-settings:pen",
                ["mobile-devices"] = "ms-settings:mobile-devices",
                ["advanced-system"] = "SystemPropertiesAdvanced.exe",
                ["device-manager"] = "devmgmt.msc"
            };

        public static object OpenSettingsPage(string page)
        {
            if (string.IsNullOrWhiteSpace(page) || !AllowedPages.TryGetValue(page, out var uri))
                throw new NotSupportedException("Cette page de paramètres Windows n’est pas autorisée par SYSTEMES.");

            var launched = Process.Start(new ProcessStartInfo
            {
                FileName = uri,
                UseShellExecute = true
            });
            if (launched == null)
                throw new InvalidOperationException("Windows n’a pas retourné de processus pour ouvrir cette page.");
            launched.Dispose();

            return new
            {
                opened = true,
                page,
                uri,
                note = "La demande d’ouverture a été transmise au gestionnaire Windows."
            };
        }
    }
}
