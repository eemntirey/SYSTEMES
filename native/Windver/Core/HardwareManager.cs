using System;
using System.Management;
using Newtonsoft.Json;

namespace Systemes.Native.Core
{
    public class HardwareManager
    {
        public static object ReadRealHardware()
        {
            try
            {
                using var searcher = new ManagementObjectSearcher("SELECT * FROM Win32_Processor");
                foreach (ManagementObject obj in searcher.Get())
                {
                    return new
                    {
                        cpuName = obj["Name"]?.ToString(),
                        cpuCores = obj["NumberOfCores"],
                        cpuThreads = obj["NumberOfLogicalProcessors"],
                    };
                }
            }
            catch { /* Windows API non disponible — pas de données inventées */ }
            return new { cpuName = null, cpuCores = null, cpuThreads = null, note = "Données réelles non disponibles sans permissions d'accès WMI" };
        }
    }
}
