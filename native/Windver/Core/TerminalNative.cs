using System;
using System.Diagnostics;
using System.IO;
using System.Text;
using Newtonsoft.Json.Linq;

namespace Systemes.Native.Core
{
    /// <summary>
    /// Exécute les commandes PowerShell dans le contexte de l'utilisateur Windows courant.
    /// Aucun lancement élevé, profil utilisateur ni script externe implicite.
    /// </summary>
    public static class TerminalNative
    {
        private const int MaxCommandCharacters = 16384;
        private const int MaxOutputCharacters = 65536;
        private const int TimeoutMilliseconds = 12000;

        public static object Execute(string command, string workingDirectory)
        {
            if (string.IsNullOrWhiteSpace(command))
                throw new ArgumentException("La commande PowerShell est obligatoire.", nameof(command));
            if (command.Length > MaxCommandCharacters)
                throw new ArgumentException("La commande dépasse la limite de 16 Kio.", nameof(command));

            var directory = string.IsNullOrWhiteSpace(workingDirectory)
                ? Environment.GetFolderPath(Environment.SpecialFolder.UserProfile)
                : Path.GetFullPath(workingDirectory);
            if (!Directory.Exists(directory))
                throw new DirectoryNotFoundException("Le répertoire de travail n'existe pas.");
            if ((File.GetAttributes(directory) & FileAttributes.ReparsePoint) != 0)
                throw new UnauthorizedAccessException("Un point de réanalyse ne peut pas être utilisé comme répertoire de travail.");

            var commandBase64 = Convert.ToBase64String(Encoding.UTF8.GetBytes(command));
            var directoryBase64 = Convert.ToBase64String(Encoding.UTF8.GetBytes(directory));
            var script = $@"
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$command = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{commandBase64}'))
$directory = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{directoryBase64}'))
$builder = New-Object System.Text.StringBuilder
$truncated = $false
$exitCode = 0
$errorText = ''
try {{
    Set-Location -LiteralPath $directory -ErrorAction Stop
    & ([scriptblock]::Create($command)) *>&1 | ForEach-Object {{
        if ($builder.Length -lt {MaxOutputCharacters}) {{
            [void]$builder.AppendLine([string]$_)
        }} else {{
            $script:truncated = $true
        }}
    }}
    $pipelineSucceeded = $?
    if ($null -ne $LASTEXITCODE) {{ $exitCode = [int]$LASTEXITCODE }}
    elseif (-not $pipelineSucceeded) {{ $exitCode = 1 }}
}} catch {{
    $errorText = $_.ToString()
    $exitCode = 1
}}
$payload = @{{
    output = $builder.ToString()
    error = $errorText
    exitCode = $exitCode
    workingDirectory = (Get-Location).Path
    truncated = $truncated
}} | ConvertTo-Json -Compress -Depth 4
[Console]::Out.WriteLine('SYSTEMES_PS_RESULT:' + $payload)
";
            var encodedScript = Convert.ToBase64String(Encoding.Unicode.GetBytes(script));
            var windows = Environment.GetFolderPath(Environment.SpecialFolder.Windows);
            var powershell = Path.Combine(windows, "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
            if (!File.Exists(powershell)) powershell = "powershell.exe";

            var start = new ProcessStartInfo
            {
                FileName = powershell,
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                StandardOutputEncoding = new UTF8Encoding(false),
                StandardErrorEncoding = new UTF8Encoding(false)
            };
            start.ArgumentList.Add("-NoLogo");
            start.ArgumentList.Add("-NoProfile");
            start.ArgumentList.Add("-NonInteractive");
            start.ArgumentList.Add("-EncodedCommand");
            start.ArgumentList.Add(encodedScript);

            using var process = Process.Start(start) ?? throw new InvalidOperationException("Impossible de démarrer Windows PowerShell.");
            var stdoutTask = process.StandardOutput.ReadToEndAsync();
            var stderrTask = process.StandardError.ReadToEndAsync();
            if (!process.WaitForExit(TimeoutMilliseconds))
            {
                try { process.Kill(entireProcessTree: true); } catch { }
                throw new TimeoutException("La commande PowerShell a dépassé 12 secondes ; le processus a été arrêté.");
            }
            TaskWait(stdoutTask, stderrTask);

            var stdout = stdoutTask.Result;
            var marker = "SYSTEMES_PS_RESULT:";
            var markerIndex = stdout.LastIndexOf(marker, StringComparison.Ordinal);
            if (markerIndex < 0)
            {
                var error = stderrTask.Result.Trim();
                throw new InvalidOperationException(string.IsNullOrWhiteSpace(error)
                    ? "PowerShell s'est arrêté sans retourner de résultat."
                    : error);
            }

            var json = JObject.Parse(stdout.Substring(markerIndex + marker.Length).Trim());
            var output = json.Value<string>("output") ?? "";
            var truncated = json.Value<bool?>("truncated") ?? false;
            if (output.Length > MaxOutputCharacters)
            {
                output = output.Substring(0, MaxOutputCharacters);
                truncated = true;
            }

            return new
            {
                output,
                error = (string?)null,
                commandError = json.Value<string>("error") ?? "",
                exitCode = json.Value<int?>("exitCode") ?? process.ExitCode,
                workingDirectory = json.Value<string>("workingDirectory") ?? directory,
                truncated,
                executedBy = Environment.UserName,
                elevated = false,
                source = "Windows PowerShell 5.1"
            };
        }

        private static void TaskWait(System.Threading.Tasks.Task<string> stdout, System.Threading.Tasks.Task<string> stderr)
        {
            System.Threading.Tasks.Task.WaitAll(stdout, stderr);
        }
    }
}
