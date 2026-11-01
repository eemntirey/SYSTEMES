using System;
using System.Collections.Generic;
using System.Linq;
using System.Management;

namespace Systemes.Native.Core
{
    public static class DeviceManager
    {
        public static object ReadRealDevices()
        {
            var devices = new List<object>();
            var usb = new List<string>();
            var network = new List<string>();

            using (var searcher = new ManagementObjectSearcher(
                "SELECT Name, Status, PNPClass, Manufacturer, DeviceID, ConfigManagerErrorCode FROM Win32_PnPEntity"))
            using (var results = searcher.Get())
            {
                foreach (ManagementObject item in results)
                {
                    var name = item["Name"]?.ToString() ?? "Périphérique sans nom";
                    var deviceId = item["DeviceID"]?.ToString() ?? "";
                    var deviceClass = item["PNPClass"]?.ToString() ?? "Unknown";
                    var status = item["Status"]?.ToString() ?? "Unknown";
                    var errorCode = Convert.ToInt32(item["ConfigManagerErrorCode"] ?? 0);

                    devices.Add(new
                    {
                        name,
                        deviceClass,
                        status,
                        manufacturer = item["Manufacturer"]?.ToString() ?? "",
                        deviceId,
                        configManagerErrorCode = errorCode,
                        healthy = errorCode == 0 && string.Equals(status, "OK", StringComparison.OrdinalIgnoreCase)
                    });

                    if (deviceId.StartsWith("USB", StringComparison.OrdinalIgnoreCase))
                        usb.Add(name);
                }
            }

            using (var searcher = new ManagementObjectSearcher(
                "SELECT Name FROM Win32_NetworkAdapter WHERE NetEnabled = TRUE"))
            using (var results = searcher.Get())
            {
                foreach (ManagementObject item in results)
                    network.Add(item["Name"]?.ToString() ?? "Adaptateur réseau");
            }

            return new
            {
                devices = devices.OrderBy(item => item.GetType().GetProperty("deviceClass")?.GetValue(item)?.ToString())
                    .ThenBy(item => item.GetType().GetProperty("name")?.GetValue(item)?.ToString()).ToArray(),
                count = devices.Count,
                problemCount = devices.Count(item => !(bool)item.GetType().GetProperty("healthy")!.GetValue(item)!),
                usb,
                network,
                source = "WMI Win32_PnPEntity + Win32_NetworkAdapter",
                note = "Inventaire matériel Windows en lecture seule. Les codes d’erreur proviennent du Gestionnaire de périphériques."
            };
        }
    }
}
