using System;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.Runtime.InteropServices;

namespace Systemes.Native.Core
{
    public class ProcessManager
    {
        private sealed record CpuSample(TimeSpan CpuTime, long Timestamp);
        private static readonly ConcurrentDictionary<int, CpuSample> PreviousSamples = new();
        private static readonly string[] ProtectedNames =
        {
            "system", "system idle process", "registry", "memory compression",
            "smss", "csrss", "wininit", "services", "lsass", "winlogon",
            "winlogon.exe", "csrss.exe", "wininit.exe", "services.exe", "lsass.exe",
            "secure system", "svchost", "fontdrvhost", "dwm"
        };

        [StructLayout(LayoutKind.Sequential)]
        private struct FileTime
        {
            public uint Low;
            public uint High;
            public ulong ToUInt64() => ((ulong)High << 32) | Low;
        }

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool GetSystemTimes(out FileTime idleTime, out FileTime kernelTime, out FileTime userTime);

        private static readonly object SystemCpuLock = new();
        private static (ulong Idle, ulong Kernel, ulong User)? PreviousSystemCpu;

        private static double? ReadSystemCpuPercent()
        {
            if (!GetSystemTimes(out var idle, out var kernel, out var user))
                return null;

            var current = (Idle: idle.ToUInt64(), Kernel: kernel.ToUInt64(), User: user.ToUInt64());
            lock (SystemCpuLock)
            {
                var previous = PreviousSystemCpu;
                PreviousSystemCpu = current;
                if (!previous.HasValue) return null;

                var idleDelta = current.Idle - previous.Value.Idle;
                var kernelDelta = current.Kernel - previous.Value.Kernel;
                var userDelta = current.User - previous.Value.User;
                var total = kernelDelta + userDelta;
                if (total == 0 || idleDelta > total) return null;
                return Math.Round(Math.Clamp((total - idleDelta) * 100.0 / total, 0, 100), 1);
            }
        }

        public static object ReadRealProcesses()
        {
            try
            {
                var now = Stopwatch.GetTimestamp();
                var systemCpuUsagePercent = ReadSystemCpuPercent();
                var processes = Process.GetProcesses();
                var activeIds = new System.Collections.Generic.HashSet<int>();
                var list = new System.Collections.Generic.List<object>();

                foreach (var process in processes)
                {
                    try
                    {
                        var id = process.Id;
                        var name = process.ProcessName;
                        activeIds.Add(id);
                        TimeSpan? cpuTime = null;
                        try { cpuTime = process.TotalProcessorTime; } catch { /* Certains processus ne publient pas ce compteur. */ }
                        double? cpuUsagePercent = null;

                        if (cpuTime.HasValue)
                        {
                            if (PreviousSamples.TryGetValue(id, out var previous))
                            {
                                var elapsedSeconds = (now - previous.Timestamp) / (double)Stopwatch.Frequency;
                                var cpuSeconds = (cpuTime.Value - previous.CpuTime).TotalSeconds;
                                if (elapsedSeconds > 0 && cpuSeconds >= 0)
                                {
                                    cpuUsagePercent = Math.Round(
                                        Math.Clamp(cpuSeconds / (elapsedSeconds * Math.Max(1, Environment.ProcessorCount)) * 100.0, 0, 100),
                                        1);
                                }
                            }
                            PreviousSamples[id] = new CpuSample(cpuTime.Value, now);
                        }
                        double memoryMb = 0;
                        int threadCount = 0;
                        int sessionId = -1;
                        string? startTimeUtcTicks = null;
                        string? executablePath = null;
                        try { memoryMb = Math.Round(process.WorkingSet64 / (1024.0 * 1024.0), 2); } catch { }
                        try { threadCount = process.Threads.Count; } catch { }
                        try { sessionId = process.SessionId; } catch { }
                        try { startTimeUtcTicks = process.StartTime.ToUniversalTime().Ticks.ToString(CultureInfo.InvariantCulture); } catch { }
                        try { executablePath = process.MainModule?.FileName; } catch { }

                        list.Add(new
                        {
                            id,
                            name,
                            memoryMB = memoryMb,
                            cpuUsagePercent,
                            cpuSampleAvailable = cpuUsagePercent.HasValue,
                            threadCount,
                            sessionId,
                            startTimeUtcTicks,
                            executablePath
                        });
                    }
                    catch { /* Le processus peut se terminer ou refuser l’accès pendant l’échantillonnage. */ }
                    finally { process.Dispose(); }
                }

                foreach (var staleId in PreviousSamples.Keys)
                    if (!activeIds.Contains(staleId)) PreviousSamples.TryRemove(staleId, out _);

                return new
                {
                    processes = list,
                    count = list.Count,
                    logicalProcessorCount = Environment.ProcessorCount,
                    systemCpuUsagePercent,
                    sampleInterval = "Entre deux actualisations",
                    note = "Processus Windows réels. Le CPU est calculé à partir du temps processeur cumulé et de l’intervalle entre deux lectures."
                };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, note = "Aucun processus simulé" };
            }
        }

        public static object KillProcess(int pid, string? expectedName, string? expectedStartTimeUtcTicks)
        {
            if (pid <= 4 || pid == Process.GetCurrentProcess().Id)
                throw new InvalidOperationException("Ce processus système ou moteur est protégé contre l’arrêt.");

            try
            {
                using var process = Process.GetProcessById(pid);
                var name = process.ProcessName;
                if (string.IsNullOrWhiteSpace(expectedName) || !string.Equals(name, expectedName, StringComparison.OrdinalIgnoreCase) ||
                    !long.TryParse(expectedStartTimeUtcTicks, NumberStyles.None, CultureInfo.InvariantCulture, out var expectedStartTicks) ||
                    process.StartTime.ToUniversalTime().Ticks != expectedStartTicks)
                    throw new InvalidOperationException("Le processus a changé depuis sa sélection ou son démarrage n’est pas vérifiable. Actualisez la liste avant de réessayer.");
                if (ProtectedNames.Contains(name, StringComparer.OrdinalIgnoreCase))
                    throw new InvalidOperationException($"Le processus Windows essentiel « {name} » est protégé.");
                process.Kill(entireProcessTree: false);
                return new { killed = true, pid, note = "Processus arrêté sur demande réelle" };
            }
            catch (ArgumentException)
            {
                throw new InvalidOperationException("Le processus n’existe plus.");
            }
        }
    }
}
