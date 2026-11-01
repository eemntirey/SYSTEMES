using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text.RegularExpressions;
using System.Threading.Tasks;

namespace Systemes.Native.Core
{
    public static class PowerNative
    {
        private sealed record PowerSchemeEntry(string guid, string name);
        private static readonly Regex GuidPattern = new(
            @"(?i)([0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12})(?:\s+\(([^)\r\n]+)\))?",
            RegexOptions.Compiled | RegexOptions.CultureInvariant);

        private static string RunPowerCfg(params string[] arguments)
        {
            var startInfo = new ProcessStartInfo
            {
                FileName = "powercfg.exe",
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true
            };
            foreach (var argument in arguments)
                startInfo.ArgumentList.Add(argument);

            using var process = new Process { StartInfo = startInfo };
            if (!process.Start())
                throw new InvalidOperationException("Windows n’a pas démarré powercfg.exe.");

            var outputTask = process.StandardOutput.ReadToEndAsync();
            var errorTask = process.StandardError.ReadToEndAsync();
            if (!process.WaitForExit(10000))
            {
                try { process.Kill(entireProcessTree: true); } catch { }
                throw new TimeoutException("La commande powercfg.exe a dépassé le délai de 10 secondes.");
            }

            Task.WaitAll(outputTask, errorTask);
            if (process.ExitCode != 0)
                throw new InvalidOperationException(string.IsNullOrWhiteSpace(errorTask.Result)
                    ? $"powercfg.exe a échoué avec le code {process.ExitCode}."
                    : errorTask.Result.Trim());

            return outputTask.Result;
        }

        private static (List<PowerSchemeEntry> Schemes, string? ActiveGuid) ReadSnapshot()
        {
            var listOutput = RunPowerCfg("/list");
            var activeOutput = RunPowerCfg("/getactivescheme");
            var schemes = new List<PowerSchemeEntry>();

            foreach (Match match in GuidPattern.Matches(listOutput))
            {
                var guid = Guid.Parse(match.Groups[1].Value).ToString("D");
                var name = match.Groups[2].Success ? match.Groups[2].Value.Trim() : guid;
                if (!schemes.Any(item => string.Equals(item.guid, guid, StringComparison.OrdinalIgnoreCase)))
                    schemes.Add(new PowerSchemeEntry(guid, name));
            }

            var activeMatch = GuidPattern.Match(activeOutput);
            var activeGuid = activeMatch.Success
                ? Guid.Parse(activeMatch.Groups[1].Value).ToString("D")
                : null;

            return (schemes, activeGuid);
        }

        public static object ReadPowerSchemes()
        {
            var snapshot = ReadSnapshot();
            return new
            {
                schemes = snapshot.Schemes,
                activeGuid = snapshot.ActiveGuid,
                note = "Plans d’alimentation lus depuis powercfg.exe. Aucun réglage n’a été modifié."
            };
        }

        public static object SetPowerScheme(string guid)
        {
            if (!Guid.TryParse(guid, out var parsedGuid))
                throw new ArgumentException("Identifiant de plan d’alimentation invalide.", nameof(guid));

            var normalizedGuid = parsedGuid.ToString("D");
            var before = ReadSnapshot();
            if (!before.Schemes.Any(item => string.Equals(item.guid, normalizedGuid, StringComparison.OrdinalIgnoreCase)))
                throw new InvalidOperationException("Ce plan n’est pas présent dans la liste des plans Windows disponibles.");

            RunPowerCfg("/setactive", normalizedGuid);
            var after = ReadSnapshot();
            if (!string.Equals(after.ActiveGuid, normalizedGuid, StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException("Windows n’a pas confirmé l’activation du plan demandé.");

            var selected = after.Schemes.First(item => string.Equals(item.guid, normalizedGuid, StringComparison.OrdinalIgnoreCase));
            return new
            {
                changed = !string.Equals(before.ActiveGuid, after.ActiveGuid, StringComparison.OrdinalIgnoreCase),
                activeGuid = after.ActiveGuid,
                activeName = selected.name,
                note = "Le plan d’alimentation actif a été relu et confirmé par Windows."
            };
        }
    }
}
