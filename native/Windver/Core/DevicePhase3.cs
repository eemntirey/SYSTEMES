using System;
using System.Management;
using Newtonsoft.Json;

namespace Systemes.Native.Core
{
    public static class DevicePhase3
    {
        public static object ReadRealUSB()
        {
            try
            {
                using var s = new ManagementObjectSearcher("SELECT * FROM Win32_USBHub");
                var devices = new System.Collections.Generic.List<object>();
                foreach (ManagementObject obj in s.Get())
                {
                    devices.Add(new { name = obj["Name"]?.ToString(), deviceId = obj["DeviceID"]?.ToString() });
                }
                return new { count = devices.Count, devices };
            }
            catch { }
            return new { count = 0, devices = new object[0], note = "Données réelles non disponibles" };
        }

        public static object ReadRealBluetooth()
        {
            try
            {
                using var s = new ManagementObjectSearcher("SELECT * FROM Win32_PnPEntity WHERE Name LIKE '%Bluetooth%'");
                var devices = new System.Collections.Generic.List<object>();
                foreach (ManagementObject obj in s.Get())
                {
                    devices.Add(new { name = obj["Name"]?.ToString() });
                }
                return new { count = devices.Count, devices };
            }
            catch { }
            return new { count = 0, devices = new object[0], note = "Données réelles non disponibles" };
        }
    }
}
