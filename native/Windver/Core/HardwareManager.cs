using System;
using System.Management;
using System.IO;

namespace Systemes.Native.Core
{
    public class HardwareManager
    {
        public static object ReadRealHardware()
        {
            try
            {
                // CPU (WMI)
                string? cpuName = null;
                int? cpuCores = null;
                int? cpuThreads = null;
                using (var searcher = new ManagementObjectSearcher("SELECT * FROM Win32_Processor"))
                {
                    foreach (ManagementObject obj in searcher.Get())
                    {
                        cpuName = obj["Name"]?.ToString();
                        cpuCores = Convert.ToInt32(obj["NumberOfCores"]);
                        cpuThreads = Convert.ToInt32(obj["NumberOfLogicalProcessors"]);
                    }
                }

                // RAM (WMI Win32_PhysicalMemory / Win32_OperatingSystem)
                long totalRamBytes = 0;
                using (var searcher = new ManagementObjectSearcher("SELECT * FROM Win32_PhysicalMemory"))
                {
                    foreach (ManagementObject obj in searcher.Get())
                    {
                        totalRamBytes += Convert.ToInt64(obj["Capacity"]);
                    }
                }
                double totalRamGB = totalRamBytes / (1024.0 * 1024.0 * 1024.0);

                // GPU (WMI Win32_VideoController)
                string? gpuName = null;
                long? gpuMemory = null;
                using (var searcher = new ManagementObjectSearcher("SELECT * FROM Win32_VideoController"))
                {
                    foreach (ManagementObject obj in searcher.Get())
                    {
                        gpuName = obj["Name"]?.ToString();
                        var adapterRam = obj["AdapterRAM"];
                        if (adapterRam != null)
                            gpuMemory = Convert.ToInt64(adapterRam);
                    }
                }

                // Disques (System.IO DriveInfo)
                var disks = System.IO.DriveInfo.GetDrives();
                var diskInfo = new System.Collections.Generic.List<object>();
                foreach (var d in disks)
                {
                    if (d.IsReady)
                    {
                        diskInfo.Add(new
                        {
                            name = d.Name,
                            totalBytes = d.TotalSize,
                            availableBytes = d.AvailableFreeSpace,
                            format = d.DriveFormat
                        });
                    }
                }

                return new
                {
                    cpuName,
                    cpuCores,
                    cpuThreads,
                    ramTotalGB = Math.Round(totalRamGB, 2),
                    gpuName,
                    gpuMemoryBytes = gpuMemory,
                    disks = diskInfo,
                    windowsVersion = System.Runtime.InteropServices.RuntimeInformation.OSDescription,
                    note = "Données réelles lues depuis Windows (WMI + Win32)"
                };
            }
            catch (Exception ex)
            {
                return new
                {
                    error = ex.Message,
                    note = "Échec lecture réelle — aucune donnée inventée"
                };
            }
        }
    }
}
