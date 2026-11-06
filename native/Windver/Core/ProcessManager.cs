using System;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.Management;
using System.Runtime.InteropServices;
using System.Text.RegularExpressions;

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

        private static readonly Regex GpuLuidPattern = new(
            @"luid_[^_]+_[^_]+_phys_\d+",
            RegexOptions.IgnoreCase | RegexOptions.Compiled);

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
                if (!previous.HasValue ||
                    current.Idle < previous.Value.Idle ||
                    current.Kernel < previous.Value.Kernel ||
                    current.User < previous.Value.User)
                    return null;

                var idleDelta = current.Idle - previous.Value.Idle;
                var kernelDelta = current.Kernel - previous.Value.Kernel;
                var userDelta = current.User - previous.Value.User;
                var total = kernelDelta + userDelta;
                if (total == 0 || idleDelta > total) return null;
                return Math.Round(Math.Clamp((total - idleDelta) * 100.0 / total, 0, 100), 1);
            }
        }

        private static string GetGpuAdapterKey(string instanceName)
        {
            var match = GpuLuidPattern.Match(instanceName ?? string.Empty);
            return match.Success ? match.Value : instanceName ?? string.Empty;
        }

        private static object ReadPerformanceSnapshot()
        {
            object[] disks = Array.Empty<object>();
            object[] networks = Array.Empty<object>();
            object[] gpus = Array.Empty<object>();
            string? diskError = null, networkError = null, gpuError = null;

            try
            {
                var rows = new System.Collections.Generic.List<object>();
                using var searcher = new ManagementObjectSearcher("SELECT Name, PercentDiskTime, DiskReadBytesPersec, DiskWriteBytesPersec FROM Win32_PerfFormattedData_PerfDisk_PhysicalDisk");
                foreach (ManagementObject item in searcher.Get())
                {
                    var name = item["Name"]?.ToString() ?? "";
                    if (name == "_Total") continue;
                    rows.Add(new
                    {
                        name,
                        activePercent = ReadCounter(item, "PercentDiskTime"),
                        readBytesPerSecond = ReadCounter(item, "DiskReadBytesPersec"),
                        writeBytesPerSecond = ReadCounter(item, "DiskWriteBytesPersec")
                    });
                }
                disks = rows.ToArray();
            }
            catch (Exception ex) { diskError = ex.Message; }

            try
            {
                var rows = new System.Collections.Generic.List<object>();
                using var searcher = new ManagementObjectSearcher("SELECT Name, BytesReceivedPersec, BytesSentPersec FROM Win32_PerfFormattedData_Tcpip_NetworkInterface");
                foreach (ManagementObject item in searcher.Get())
                {
                    rows.Add(new
                    {
                        name = item["Name"]?.ToString() ?? "Interface réseau",
                        receiveBytesPerSecond = ReadCounter(item, "BytesReceivedPersec"),
                        sendBytesPerSecond = ReadCounter(item, "BytesSentPersec")
                    });
                }
                networks = rows.ToArray();
            }
            catch (Exception ex) { networkError = ex.Message; }

            try
            {
                var engines = new System.Collections.Generic.Dictionary<string, double>(StringComparer.OrdinalIgnoreCase);
                using (var searcher = new ManagementObjectSearcher("SELECT Name, UtilizationPercentage FROM Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine"))
                {
                    foreach (ManagementObject item in searcher.Get())
                    {
                        var instance = item["Name"]?.ToString() ?? "GPU";
                        var adapter = GetGpuAdapterKey(instance);
                        if (string.IsNullOrWhiteSpace(adapter)) adapter = "GPU";
                        var utilization = ReadCounter(item, "UtilizationPercentage");
                        if (!engines.TryGetValue(adapter, out var current) || utilization > current)
                            engines[adapter] = Math.Clamp(utilization, 0, 100);
                    }
                }

                var memoryByAdapter = new System.Collections.Generic.Dictionary<string, (ulong Dedicated, ulong Shared)>(StringComparer.OrdinalIgnoreCase);
                using (var searcher = new ManagementObjectSearcher("SELECT Name, DedicatedUsage, SharedUsage FROM Win32_PerfFormattedData_GPUPerformanceCounters_GPUAdapterMemory"))
                {
                    foreach (ManagementObject item in searcher.Get())
                    {
                        var instance = item["Name"]?.ToString() ?? "GPU";
                        var adapter = GetGpuAdapterKey(instance);
                        if (string.IsNullOrWhiteSpace(adapter)) adapter = "GPU";
                        memoryByAdapter[adapter] = (
                            ReadUnsignedCounter(item, "DedicatedUsage"),
                            ReadUnsignedCounter(item, "SharedUsage"));
                    }
                }

                // GPU engine and adapter-memory counters expose separate WMI instances.
                // Join them by the stable adapter LUID; never silently report a measured
                // utilization with fabricated zero VRAM values when the join is unavailable.
                var adapterKeys = engines.Keys
                    .Union(memoryByAdapter.Keys, StringComparer.OrdinalIgnoreCase)
                    .Distinct(StringComparer.OrdinalIgnoreCase);

                gpus = adapterKeys.Select(adapter =>
                {
                    var hasEngine = engines.TryGetValue(adapter, out var utilization);
                    var hasMemory = memoryByAdapter.TryGetValue(adapter, out var memory);
                    return (object)new
                    {
                        adapter,
                        utilizationPercent = hasEngine ? (double?)utilization : null,
                        dedicatedMemoryBytes = hasMemory ? (ulong?)memory.Dedicated : null,
                        sharedMemoryBytes = hasMemory ? (ulong?)memory.Shared : null
                    };
                }).ToArray();
            }
            catch (Exception ex) { gpuError = ex.Message; }

            return new
            {
                disks,
                networks,
                gpus,
                diskError,
                networkError,
                gpuError,
                sampledAtUtc = DateTime.UtcNow.ToString("O", CultureInfo.InvariantCulture),
                source = "Windows WMI performance counters"
            };
        }

        private static double ReadCounter(ManagementBaseObject item, string property)
        {
            try { return Convert.ToDouble(item[property] ?? 0, CultureInfo.InvariantCulture); }
            catch { return 0; }
        }

        private static ulong ReadUnsignedCounter(ManagementBaseObject item, string property)
        {
            try { return Convert.ToUInt64(item[property] ?? 0, CultureInfo.InvariantCulture); }
            catch { return 0; }
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
                    performance = ReadPerformanceSnapshot(),
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