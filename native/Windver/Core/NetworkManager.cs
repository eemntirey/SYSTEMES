using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Net.NetworkInformation;
using System.Security;
using System.Text;
using System.Text.RegularExpressions;

namespace Systemes.Native.Core
{
    /// <summary>
    /// Gestionnaire réseau réel : interfaces (System.Net.NetworkInformation),
    /// Wi-Fi via netsh wlan (scan, état, connexion) — aucune simulation.
    /// </summary>
    public static class NetworkManager
    {
        /// <summary>Lance une commande externe et retourne (stdout, stderr).</summary>
        private static (string Output, string Error, int ExitCode) RunCommand(string fileName, string arguments)
        {
            var psi = new ProcessStartInfo
            {
                FileName = fileName,
                Arguments = arguments,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true,
                StandardOutputEncoding = Encoding.UTF8,
                StandardErrorEncoding = Encoding.UTF8,
            };
            using var process = Process.Start(psi) ?? throw new InvalidOperationException($"Impossible de démarrer {fileName}.");
            var stdout = process.StandardOutput.ReadToEnd();
            var stderr = process.StandardError.ReadToEnd();
            process.WaitForExit();
            return (stdout, stderr, process.ExitCode);
        }

        /// <summary>Interfaces réseau actives réelles.</summary>
        public static object ReadRealNetwork()
        {
            try
            {
                var interfaces = NetworkInterface.GetAllNetworkInterfaces();
                var list = new List<object>();
                foreach (var ni in interfaces)
                {
                    if (ni.OperationalStatus == OperationalStatus.Up && ni.NetworkInterfaceType != NetworkInterfaceType.Loopback)
                    {
                        list.Add(new
                        {
                            name = ni.Name,
                            description = ni.Description,
                            type = ni.NetworkInterfaceType.ToString(),
                            speedMBps = Math.Round(ni.Speed / (1024.0 * 1024.0), 1),
                        });
                    }
                }
                return new
                {
                    interfaces = list.ToArray(),
                    wifiPresent = list.Exists(i => (i as dynamic)?.type == "Wireless80211"),
                    note = "Interfaces réseau réelles détectées (pas de simulation).",
                };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, note = "Détection réseau échouée." };
            }
        }

        private static readonly Regex SsidHeaderRegex =
            new(@"^\s*SSID\s+\d+\s*:\s*(.+?)\s*$", RegexOptions.Compiled | RegexOptions.IgnoreCase);
        private static readonly Regex FieldRegex =
            new(@"^\s*(Signal|Authentication|Encryption|Radio type|Channel|BSSID)\s*:\s*(.+?)\s*$",
                RegexOptions.Compiled | RegexOptions.IgnoreCase);
        private static readonly Regex BssidHeaderRegex =
            new(@"^\s*BSSID\s+\d+\s*:\s*(.+?)\s*$", RegexOptions.Compiled | RegexOptions.IgnoreCase);

        /// <summary>Scan réel des réseaux Wi-Fi visibles (netsh wlan show networks mode=bssid).</summary>
        public static object ScanWifi()
        {
            try
            {
                var (stdout, stderr, exitCode) = RunCommand("netsh", "wlan show networks mode=bssid");
                var output = stdout + stderr;
                if (exitCode != 0 && !output.Contains("SSID"))
                    return new { error = $"netsh a échoué (code {exitCode}) : aucun adaptateur Wi-Fi ou service WLAN arrêté.", networks = Array.Empty<object>() };

                var networks = new List<object>();
                string? currentSsid = null;
                string? auth = null, encryption = null;
                string? bssid = null, signal = null, radio = null, channel = null;

                void Flush()
                {
                    if (currentSsid == null) return;
                    networks.Add(new
                    {
                        ssid = currentSsid,
                        signal = signal,
                        authentication = auth,
                        encryption,
                        bssid,
                        radioType = radio,
                        channel,
                    });
                    bssid = null; signal = null; radio = null; channel = null;
                }

                foreach (var rawLine in output.Split('\n'))
                {
                    var line = rawLine.TrimEnd('\r');
                    if (SsidHeaderRegex.IsMatch(line))
                    {
                        Flush();
                        auth = null; encryption = null;
                        currentSsid = SsidHeaderRegex.Match(line).Groups[1].Value;
                        continue;
                    }
                    if (currentSsid == null) continue;

                    if (BssidHeaderRegex.IsMatch(line))
                    {
                        Flush();
                        bssid = BssidHeaderRegex.Match(line).Groups[1].Value;
                        continue;
                    }
                    var field = FieldRegex.Match(line);
                    if (!field.Success) continue;
                    switch (field.Groups[1].Value.Trim().ToLowerInvariant())
                    {
                        case "signal": signal = field.Groups[2].Value; break;
                        case "authentication": auth = field.Groups[2].Value; break;
                        case "encryption": encryption = field.Groups[2].Value; break;
                        case "radio type": radio = field.Groups[2].Value; break;
                        case "channel": channel = field.Groups[2].Value; break;
                    }
                }
                Flush();

                return new
                {
                    networks = networks.ToArray(),
                    note = "Réseaux Wi-Fi scannés réellement via netsh wlan.",
                };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, networks = Array.Empty<object>() };
            }
        }

        private static readonly Regex InterfaceFieldRegex =
            new(@"^\s*(SSID|State|Signal|Radio type|Channel|Authentication|BSSID)\s*:\s*(.+?)\s*$",
                RegexOptions.Compiled | RegexOptions.IgnoreCase);

        /// <summary>État de la connexion Wi-Fi actuelle (netsh wlan show interfaces).</summary>
        public static object ReadWifiStatus()
        {
            try
            {
                var (stdout, stderr, _) = RunCommand("netsh", "wlan show interfaces");
                string? ssid = null, state = null, signal = null, radio = null, channel = null, auth = null;

                foreach (var rawLine in (stdout + stderr).Split('\n'))
                {
                    var field = InterfaceFieldRegex.Match(rawLine.TrimEnd('\r'));
                    if (!field.Success) continue;
                    switch (field.Groups[1].Value.Trim().ToLowerInvariant())
                    {
                        case "ssid": ssid = field.Groups[2].Value; break;
                        case "state": state = field.Groups[2].Value; break;
                        case "signal": signal = field.Groups[2].Value; break;
                        case "radio type": radio = field.Groups[2].Value; break;
                        case "channel": channel = field.Groups[2].Value; break;
                        case "authentication": auth = field.Groups[2].Value; break;
                    }
                }

                var connected = state != null && state.Contains("connect", StringComparison.OrdinalIgnoreCase)
                    && !state.Contains("disconnec", StringComparison.OrdinalIgnoreCase);

                return new
                {
                    connected,
                    ssid,
                    state,
                    signal,
                    radioType = radio,
                    channel,
                    authentication = auth,
                    note = "État Wi-Fi lu via netsh wlan show interfaces.",
                };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, connected = false, ssid = (string?)null };
            }
        }

        /// <summary>Se connecte à un réseau Wi-Fi (profil existant, ou crée un profil WPA2 si un mot de passe est fourni).</summary>
        public static object ConnectWifi(string ssid, string? password)
        {
            if (string.IsNullOrWhiteSpace(ssid))
                return new { error = "Le nom du réseau (SSID) est requis.", connected = false };
            if (ssid.Length > 32)
                return new { error = "Un SSID ne peut pas dépasser 32 caractères.", connected = false };

            try
            {
                string output;
                if (!string.IsNullOrEmpty(password))
                {
                    if (password.Length < 8 || password.Length > 63)
                        return new { error = "Le mot de passe WPA2 doit faire entre 8 et 63 caractères.", connected = false };

                    var escapedSsid = SecurityElement.Escape(ssid) ?? ssid;
                    var escapedPassword = SecurityElement.Escape(password) ?? password;
                    var profileXml =
                        $"""
                        <?xml version="1.0"?>
                        <WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
                          <name>{escapedSsid}</name>
                          <SSIDConfig><SSID><name>{escapedSsid}</name></SSID></SSIDConfig>
                          <connectionType>ESS</connectionType>
                          <connectionMode>auto</connectionMode>
                          <MSM>
                            <security>
                              <authEncryption>
                                <authentication>WPA2PSK</authentication>
                                <encryption>AES</encryption>
                                <useOneX>false</useOneX>
                              </authEncryption>
                              <sharedKey>
                                <keyType>passPhrase</keyType>
                                <protected>false</protected>
                                <keyMaterial>{escapedPassword}</keyMaterial>
                              </sharedKey>
                            </security>
                          </MSM>
                        </WLANProfile>
                        """;
                    var tempProfile = Path.Combine(Path.GetTempPath(), $"systemes-wlan-{Guid.NewGuid():N}.xml");
                    File.WriteAllText(tempProfile, profileXml, Encoding.UTF8);
                    try
                    {
                        (output, _, _) = RunCommand("netsh", $"wlan add profile filename=\"{tempProfile}\"");
                    }
                    finally
                    {
                        try { File.Delete(tempProfile); } catch { /* profil temporaire déjà purgé */ }
                    }
                }

                var (connectOut, connectErr, exitCode) = RunCommand("netsh", $"wlan connect name=\"{ssid}\"");
                var combined = (connectOut + connectErr).Trim();
                if (exitCode != 0)
                    return new { error = $"Échec de la connexion à « {ssid} » : {combined}", connected = false };

                return new { connected = true, ssid, message = combined, note = "Connexion Wi-Fi demandée via netsh wlan connect." };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, connected = false };
            }
        }

        /// <summary>Déconnecte du réseau Wi-Fi actuel.</summary>
        public static object DisconnectWifi()
        {
            try
            {
                var (_, stderr, exitCode) = RunCommand("netsh", "wlan disconnect");
                if (exitCode != 0)
                    return new { error = $"Échec de la déconnexion : {stderr.Trim()}", disconnected = false };
                return new { disconnected = true, note = "Déconnexion Wi-Fi demandée via netsh." };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, disconnected = false };
            }
        }
    }
}
