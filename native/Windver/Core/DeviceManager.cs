using System;
using System.Management;

namespace Systemes.Native.Core
{
    public class DeviceManager
    {
        public static object ReadRealDevices()
        {
            try
            {
                // USB (WMI Win32_USBHub / Win32_PnPEntity)
                var usb = new System.Collections.Generic.List<string>();
                using (var searcher = new ManagementObjectSearcher("SELECT * FROM Win32_PnPEntity WHERE DeviceID LIKE 'USB%'"))
                {
                    foreach (ManagementObject obj in searcher.Get())
                    {
                        usb.Add(obj["Name"]?.ToString() ?? "USB device");
                    }
                }

                // Réseau (WMI Win32_NetworkAdapter)
                var network = new System.Collections.Generic.List<string>();
                using (var searcher = new ManagementObjectSearcher("SELECT * FROM Win32_NetworkAdapter WHERE NetEnabled = TRUE"))
                {
                    foreach (ManagementObject obj in searcher.Get())
                    {
                        network.Add(obj["Name"]?.ToString() ?? "Network adapter");
                    }
                }

                return new
                {
                    usb = usb,
                    network = network,
                    note = "Périphériques détectés réellement via WMI"
                };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, note = "Détection des périphériques échouée" };
            }
        }
    }
}
