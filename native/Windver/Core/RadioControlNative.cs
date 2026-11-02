using System;
using System.Diagnostics;
using System.IO;
using System.Text;
using Newtonsoft.Json.Linq;

namespace Systemes.Native.Core
{
    /// <summary>Contrôle réel des adaptateurs réseau et Bluetooth via les outils Windows natifs.</summary>
    public static class RadioControlNative
    {
        private static JObject RunPowerShell(string script)
        {
            var encoded = Convert.ToBase64String(Encoding.Unicode.GetBytes(script));
            var windows = Environment.GetFolderPath(Environment.SpecialFolder.Windows);
            var executable = Path.Combine(windows, "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
            if (!File.Exists(executable)) executable = "powershell.exe";
            var start = new ProcessStartInfo
            {
                FileName = executable,
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                StandardOutputEncoding = new UTF8Encoding(false),
                StandardErrorEncoding = new UTF8Encoding(false)
            };
            start.ArgumentList.Add("-NoProfile");
            start.ArgumentList.Add("-NonInteractive");
            start.ArgumentList.Add("-EncodedCommand");
            start.ArgumentList.Add(encoded);

            using var process = Process.Start(start) ?? throw new InvalidOperationException("Impossible de démarrer PowerShell.");
            var outputTask = process.StandardOutput.ReadToEndAsync();
            var errorTask = process.StandardError.ReadToEndAsync();
            if (!process.WaitForExit(15000))
            {
                try { process.Kill(entireProcessTree: true); } catch { }
                throw new TimeoutException("Le contrôle de l’adaptateur a dépassé 15 secondes.");
            }
            System.Threading.Tasks.Task.WaitAll(outputTask, errorTask);
            var output = outputTask.Result.Trim();
            if (string.IsNullOrWhiteSpace(output))
                throw new InvalidOperationException(string.IsNullOrWhiteSpace(errorTask.Result)
                    ? $"PowerShell n’a retourné aucun résultat (code {process.ExitCode})."
                    : errorTask.Result.Trim());
            var jsonStart = output.IndexOf('{');
            var jsonEnd = output.LastIndexOf('}');
            if (jsonStart < 0 || jsonEnd <= jsonStart)
                throw new InvalidOperationException(string.IsNullOrWhiteSpace(errorTask.Result) ? output : errorTask.Result.Trim());
            var result = JObject.Parse(output.Substring(jsonStart, jsonEnd - jsonStart + 1));
            if (result["error"]?.Type == JTokenType.String)
                throw new InvalidOperationException(result.Value<string>("error"));
            if (process.ExitCode != 0)
                throw new InvalidOperationException(string.IsNullOrWhiteSpace(errorTask.Result)
                    ? $"PowerShell a échoué (code {process.ExitCode})."
                    : errorTask.Result.Trim());
            return result;
        }

        public static object SetWifiEnabled(bool enabled)
        {
            var action = enabled ? "Enable-NetAdapter" : "Disable-NetAdapter";
            var script = $@"
$ErrorActionPreference = 'Stop'
try {{
    $adapter = Get-NetAdapter -Physical -ErrorAction Stop |
        Where-Object {{ $_.NdisPhysicalMedium -eq 9 -or $_.InterfaceDescription -match 'Wireless|Wi-?Fi|802\.11' }} |
        Select-Object -First 1
    if (-not $adapter) {{ throw 'Aucun adaptateur Wi-Fi physique n’a été détecté.' }}
    {action} -Name $adapter.Name -Confirm:$false -ErrorAction Stop
    Start-Sleep -Milliseconds 350
    $state = Get-NetAdapter -Name $adapter.Name -ErrorAction Stop
    @{{
        enabled = ($state.Status -ne 'Disabled' -and $state.Status -ne 'Not Present')
        adapterName = $state.Name
        status = [string]$state.Status
        source = 'Windows NetAdapter cmdlets'
    }} | ConvertTo-Json -Compress
}} catch {{
    @{{ error = $_.Exception.Message }} | ConvertTo-Json -Compress
    exit 1
}}";
            var result = RunPowerShell(script);
            return new
            {
                enabled = result.Value<bool?>("enabled") ?? false,
                adapterName = result.Value<string>("adapterName"),
                status = result.Value<string>("status"),
                source = result.Value<string>("source")
            };
        }

        public static object SetBluetoothEnabled(bool enabled)
        {
            var action = enabled ? "Enable-PnpDevice" : "Disable-PnpDevice";
            var script = $@"
$ErrorActionPreference = 'Stop'
try {{
    $adapter = Get-PnpDevice -Class Bluetooth -ErrorAction Stop |
        Where-Object {{ $_.InstanceId -notmatch '^(BTH|BTHLE)\\' -and $_.FriendlyName -match 'Bluetooth' }} |
        Select-Object -First 1
    if (-not $adapter) {{ throw 'Aucun adaptateur Bluetooth physique n’a été détecté.' }}
    {action} -InstanceId $adapter.InstanceId -Confirm:$false -ErrorAction Stop
    Start-Sleep -Milliseconds 350
    $state = Get-PnpDevice -InstanceId $adapter.InstanceId -ErrorAction Stop
    @{{
        enabled = ([string]$state.Status -eq 'OK')
        adapterName = $state.FriendlyName
        instanceId = $state.InstanceId
        status = [string]$state.Status
        source = 'Windows PnP Device cmdlets'
    }} | ConvertTo-Json -Compress
}} catch {{
    @{{ error = $_.Exception.Message }} | ConvertTo-Json -Compress
    exit 1
}}";
            var result = RunPowerShell(script);
            return new
            {
                enabled = result.Value<bool?>("enabled") ?? false,
                adapterName = result.Value<string>("adapterName"),
                instanceId = result.Value<string>("instanceId"),
                status = result.Value<string>("status"),
                source = result.Value<string>("source")
            };
        }
    }
}
