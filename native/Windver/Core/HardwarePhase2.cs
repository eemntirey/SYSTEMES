using System;
using System.Management;
using Newtonsoft.Json;

namespace Systemes.Native.Core
{
    public static class HardwarePhase2
    {
        public static object ReadRealMemory()
        {
            try
            {
                using var s = new ManagementObjectSearcher("SELECT * FROM Win32_OperatingSystem");
                foreach (ManagementObject obj in s.Get())
                {
                    return new
                    {
                        totalRAM = obj["TotalVisibleMemorySize"],
                        freeRAM = obj["FreePhysicalMemory"],
                        usedPercent = Math.Round((1.0 - (Convert.ToDouble(obj["FreePhysicalMemory"]) / Convert.ToDouble(obj["TotalVisibleMemorySize"])) * 100), 1)
                    };
                }
            }
            catch { }
            return new { totalRAM = null, freeRAM = null, usedPercent = null, note = "Données réelles non disponibles sans accès WMI" };
        }

        public static object ReadRealDisk()
        {
            try
            {
                using var s = new ManagementObjectSearcher("SELECT * FROM Win32_LogicalDisk WHERE DriveType=3");
                foreach (ManagementObject obj in s.Get())
                {
                    double total = Convert.ToDouble(obj["Size"]);
                    double free = Convert.ToDouble(obj["FreeSpace"]);
                    return new
                    {
                        drive = obj["DeviceID"],
                        totalGB = Math.Round(total / (1024 * 1024 * 1024), 1),
                        freeGB = Math.Round(free / (1024 * 1024 * 1024), 1),
                        percentFree = Math.Round(free / total * 100, 1)
                    };
                }
            }
            catch { }
            return new { drive = null, totalGB = null, freeGB = null, percentFree = null, note = "Données réelles non disponibles" };
        }

        public static object ReadRealGPU()
        {
            try
            {
                using var s = new ManagementObjectSearcher("SELECT * FROM Win32_VideoController");
                foreach (ManagementObject obj in s.Get())
                {
                    return new
                    {
                        name = obj["Name"]?.ToString(),
                        adapterRAM = obj["AdapterRAM"],
                        driverVersion = obj["DriverVersion"]?.ToString()
                    };
                }
            }
            catch { }
            return new { name = null, adapterRAM = null, driverVersion = null, note = "Données réelles non disponibles" };
        }
    }
}
