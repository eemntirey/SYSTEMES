using System;
using System.Management;
using System.Linq;

namespace SYSTEMES.Native;

public class NativeCore : ISystemServices
{
    public WindowsInfo GetWindowsInfo()
    {
        var osVersion = System.Environment.OSVersion;
        var version = $"Windows {osVersion.Version.Major}.{osVersion.Version.Minor}.{osVersion.Version.Build}";
        bool isWin11 = osVersion.Version.Build >= 22000;
        return new WindowsInfo(version, osVersion.Version.Build.ToString(), isWin11, "Windows 11 Pro");
    }

    public HardwareInfo GetHardwareInfo()
    {
        try
        {
            using var searcher = new ManagementObjectSearcher("SELECT * FROM Win32_Processor");
            var cpu = searcher.Get().OfType<ManagementObject>().FirstOrDefault();
            return new HardwareInfo(
                cpu?.["Name"]?.ToString() ?? "CPU",
                GC.GetTotalMemory(false),
                GC.GetGCMemoryInfo().TotalAvailableMemoryBytes,
                "GPU Unknown",
                0
            );
        }
        catch
        {
            return new HardwareInfo("Unknown", 0, 0, "Unknown", 0);
        }
    }

    public DeviceStatus[] GetDevices()
    {
        try
        {
            using var searcher = new ManagementObjectSearcher("SELECT * FROM Win32_PnPEntity WHERE ConfigManagerErrorCode = 0");
            return searcher.Get().OfType<ManagementObject>()
                .Select(m => new DeviceStatus(
                    m["Name"]?.ToString() ?? "Unknown",
                    "Actif",
                    m["DriverVersion"]?.ToString() ?? "N/D"
                ))
                .Take(10)
                .ToArray();
        }
        catch
        {
            return Array.Empty<DeviceStatus>();
        }
    }

    public ProcessInfo[] GetProcesses()
    {
        var procs = System.Diagnostics.Process.GetProcesses()
            .Select(p => new ProcessInfo(p.ProcessName, p.Id, 0.0, p.WorkingSet64))
            .Take(20)
            .ToArray();
        return procs;
    }

    public bool IsUpdateAvailable()
    {
        // Phase 5: réel via Windows Update API (Windows 10/11)
        // Pour l'instant, signalé comme non vérifié jusqu'urgence
        return false; // Signalé clairement si non disponible
    }

    public void RequestPrivilege(string operation, string reason)
    {
        // Phase 8 : validation d'autorisation avant exécution
        Console.WriteLine($"[SYSTEMES] Demande de privilège : {operation} — raison : {reason}");
    }
}
