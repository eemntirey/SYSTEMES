using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Management;

namespace Systemes.Native.Core
{
    /// <summary>
    /// Inventaire des volumes et disques Windows (lecture seule).
    /// </summary>
    public static class StorageManager
    {
        public static object ReadVolumes()
        {
            var volumes = new List<object>();
            var disks = new List<object>();

            foreach (var drive in DriveInfo.GetDrives())
            {
                try
                {
                    volumes.Add(new
                    {
                        name = drive.Name,
                        label = drive.IsReady ? (drive.VolumeLabel ?? "") : "",
                        driveType = drive.DriveType.ToString(),
                        format = drive.IsReady ? (drive.DriveFormat ?? "") : "",
                        isReady = drive.IsReady,
                        totalBytes = drive.IsReady ? drive.TotalSize : 0L,
                        freeBytes = drive.IsReady ? drive.TotalFreeSpace : 0L,
                        usedBytes = drive.IsReady ? Math.Max(0L, drive.TotalSize - drive.TotalFreeSpace) : 0L,
                        freePercent = drive.IsReady && drive.TotalSize > 0
                            ? Math.Round(100.0 * drive.TotalFreeSpace / drive.TotalSize, 1)
                            : 0.0,
                    });
                }
                catch
                {
                    volumes.Add(new
                    {
                        name = drive.Name,
                        label = "",
                        driveType = drive.DriveType.ToString(),
                        format = "",
                        isReady = false,
                        totalBytes = 0L,
                        freeBytes = 0L,
                        usedBytes = 0L,
                        freePercent = 0.0,
                    });
                }
            }

            try
            {
                using var searcher = new ManagementObjectSearcher(
                    "SELECT Model, InterfaceType, MediaType, Size, SerialNumber, Status FROM Win32_DiskDrive");
                using var results = searcher.Get();
                foreach (ManagementObject item in results)
                {
                    disks.Add(new
                    {
                        model = item["Model"]?.ToString() ?? "Disque",
                        interfaceType = item["InterfaceType"]?.ToString() ?? "",
                        mediaType = item["MediaType"]?.ToString() ?? "",
                        sizeBytes = Convert.ToInt64(item["Size"] ?? 0),
                        serial = item["SerialNumber"]?.ToString()?.Trim() ?? "",
                        status = item["Status"]?.ToString() ?? "",
                    });
                }
            }
            catch
            {
            }

            return new
            {
                volumes = volumes.OrderBy(v => ((dynamic)v).name).ToArray(),
                disks,
                volumeCount = volumes.Count,
                diskCount = disks.Count,
                source = "DriveInfo + WMI Win32_DiskDrive",
                note = "Inventaire stockage en lecture seule. Aucune modification de partition.",
            };
        }
    }
}
