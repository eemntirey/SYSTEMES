using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Text;
using Newtonsoft.Json.Linq;

namespace Systemes.Native.Core
{
    /// <summary>
    /// Bluetooth réel via Windows PowerShell (module PnpDevice + WinRT DeviceWatcher/Pairing).
    /// Chaque opération est best-effort : en cas d'indisponibilité, une erreur explicite est
    /// retournée — jamais de faux résultats (règle de l'audit SYSTEMES).
    /// </summary>
    public static class BluetoothManager
    {
        private static (string Output, string Error, int ExitCode) RunPowerShell(string script, int timeoutMs)
        {
            var encoded = Convert.ToBase64String(Encoding.Unicode.GetBytes(script));
            var psi = new ProcessStartInfo
            {
                // PowerShell 5.1 (Desktop) obligatoire : la projection WinRT disparaît de pwsh 7.
                FileName = "powershell.exe",
                Arguments = $"-NoProfile -NonInteractive -EncodedCommand {encoded}",
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true,
                StandardOutputEncoding = Encoding.UTF8,
                StandardErrorEncoding = Encoding.UTF8,
            };
            using var process = Process.Start(psi) ?? throw new InvalidOperationException("Impossible de démarrer PowerShell.");
            var stdoutTask = process.StandardOutput.ReadToEndAsync();
            var stderrTask = process.StandardError.ReadToEndAsync();
            if (!process.WaitForExit(timeoutMs))
            {
                try { process.Kill(); } catch { /* déjà terminé */ }
                throw new TimeoutException($"La commande PowerShell a dépassé {timeoutMs / 1000} s.");
            }
            return (stdoutTask.Result, stderrTask.Result, process.ExitCode);
        }

        private static JObject ParseJsonOutput(string output, string error, int exitCode)
        {
            var json = output.Trim();
            var start = json.IndexOf('{');
            var end = json.LastIndexOf('}');
            if (start >= 0 && end > start)
                return JObject.Parse(json.Substring(start, end - start + 1));
            if (exitCode != 0)
                throw new InvalidOperationException(string.IsNullOrWhiteSpace(error)
                    ? $"PowerShell a échoué (code {exitCode})."
                    : error.Trim());
            throw new InvalidOperationException("Sortie PowerShell illisible : " + json);
        }

        private const string StatusScript = """
            $ErrorActionPreference = 'Stop'
            $all = @(Get-PnpDevice -Class Bluetooth -ErrorAction Stop)
            $adapter = $all | Where-Object { $_.InstanceName -match 'LOCAL\\ADAPTER' } | Select-Object -First 1
            $paired = @($all | Where-Object { $_.InstanceName -notmatch 'LOCAL\\ADAPTER' } | ForEach-Object {
                @{ name = $_.FriendlyName; status = "$($_.Status)"; id = $_.InstanceName }
            })
            @{
                adapterPresent = [bool]$adapter
                powered = [bool]($adapter -and "$($adapter.Status)" -eq 'OK')
                adapterName = if ($adapter) { $adapter.FriendlyName } else { $null }
                devices = $paired
            } | ConvertTo-Json -Compress -Depth 6
            """;

        /// <summary>État réel de l'adaptateur Bluetooth et appareils connus (Get-PnpDevice).</summary>
        public static object ReadStatus()
        {
            try
            {
                var (output, error, exitCode) = RunPowerShell(StatusScript, 15_000);
                var json = ParseJsonOutput(output, error, exitCode);
                return new
                {
                    adapterPresent = json.Value<bool?>("adapterPresent") ?? false,
                    powered = json.Value<bool?>("powered") ?? false,
                    adapterName = json.Value<string?>("adapterName"),
                    devices = json["devices"] is JArray arr ? arr.ToObject<object[]>() : Array.Empty<object>(),
                    note = "Bluetooth réel via Get-PnpDevice — aucun état simulé.",
                };
            }
            catch (Exception ex)
            {
                return new
                {
                    error = ex.Message,
                    adapterPresent = false,
                    powered = false,
                    devices = Array.Empty<object>(),
                };
            }
        }

        private static string BuildScanScript(int timeoutSeconds) => $"""
            $ErrorActionPreference = 'Stop'
            $watcherType = [Windows.Devices.Enumeration.DeviceWatcher,Windows.Devices.Enumeration,ContentType=WindowsRuntime]
            $selector = [Windows.Devices.Bluetooth.BluetoothLEDevice,Windows.Devices.Bluetooth,ContentType=WindowsRuntime]::GetDeviceSelector()
            $watcher = $watcherType::CreateWatcher($selector)
            $found = @{{}}
            $subId = Register-ObjectEvent -InputObject $watcher -EventName Added -SourceIdentifier 'systemes-bt-added'
            $watcher.Start()
            $deadline = (Get-Date).AddSeconds({timeoutSeconds})
            while ((Get-Date) -lt $deadline) {{
                Start-Sleep -Milliseconds 400
                $pending = @(Get-Event -SourceIdentifier 'systemes-bt-added' -ErrorAction SilentlyContinue)
                foreach ($evt in $pending) {{
                    $id = $evt.EventArgs.Id
                    if (-not $found.ContainsKey($id)) {{
                        $found[$id] = @{{ id = $id; name = $evt.EventArgs.Name }}
                    }}
                    Remove-Event -EventIdentifier $evt.EventIdentifier -ErrorAction SilentlyContinue
                }}
            }}
            try {{ $watcher.Stop() }} catch {{ }}
            Unregister-Event -SourceIdentifier 'systemes-bt-added' -ErrorAction SilentlyContinue
            @{{ devices = @($found.Values) }} | ConvertTo-Json -Compress -Depth 5
            """;

        /// <summary>Scan réel des périphériques Bluetooth LE à proximité (WinRT DeviceWatcher, ~${timeout}s).</summary>
        public static object ScanDevices(int timeoutSeconds)
        {
            if (timeoutSeconds < 2 || timeoutSeconds > 20)
                timeoutSeconds = 6;
            try
            {
                var (output, error, exitCode) = RunPowerShell(BuildScanScript(timeoutSeconds), (timeoutSeconds + 10) * 1000);
                var json = ParseJsonOutput(output, error, exitCode);
                return new
                {
                    devices = json["devices"] is JArray arr ? arr.ToObject<object[]>() : Array.Empty<object>(),
                    timeoutSeconds,
                    note = "Périphériques BLE découverts réellement (DeviceWatcher WinRT).",
                };
            }
            catch (Exception ex)
            {
                return new
                {
                    error = ex.Message,
                    devices = Array.Empty<object>(),
                    note = "Le scan Bluetooth nécessite un adaptateur actif et Windows PowerShell 5.1.",
                };
            }
        }

        private const string PairScriptTemplate = """
            $ErrorActionPreference = 'Stop'
            $id = '{0}'
            $dev = [Windows.Devices.Enumeration.DeviceInformation,Windows.Devices.Enumeration,ContentType=WindowsRuntime]::CreateFromIdAsync($id).GetAwaiter().GetResult()
            $result = $dev.Pairing.PairAsync().GetAwaiter().GetResult()
            @{ name = $dev.Name; status = "$($result.Status)"; paired = ($result.Status -eq 'Paired') } | ConvertTo-Json -Compress
            """;

        /// <summary>Tente d'appairer un périphérique Bluetooth par son ID d'énumération (WinRT Pairing).</summary>
        public static object PairDevice(string deviceId)
        {
            if (string.IsNullOrWhiteSpace(deviceId))
                return new { error = "L'ID du périphérique est requis.", paired = false };
            try
            {
                var script = string.Format(PairScriptTemplate, deviceId.Replace("'", "''"));
                var (output, error, exitCode) = RunPowerShell(script, 30_000);
                var json = ParseJsonOutput(output, error, exitCode);
                var status = json.Value<string?>("status") ?? "Unknown";
                return new
                {
                    paired = json.Value<bool?>("paired") ?? false,
                    status,
                    name = json.Value<string?>("name"),
                    note = status == "Paired"
                        ? "Appairage confirmé par Windows."
                        : $"Appairage refusé par Windows (statut : {status}).",
                };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, paired = false };
            }
        }
    }
}
