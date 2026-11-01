using System;
using System.Management;

namespace Systemes.Native.Core
{
    public class BrightnessAndBattery
    {
        public static object ReadRealPower()
        {
            try
            {
                // Batterie (WMI Win32_Battery)
                int? batteryCharge = null;
                bool? batteryCharging = null;
                using (var searcher = new ManagementObjectSearcher("SELECT * FROM Win32_Battery"))
                {
                    foreach (ManagementObject obj in searcher.Get())
                    {
                        batteryCharge = Convert.ToInt32(obj["EstimatedChargeRemaining"]);
                        var charging = obj["BatteryStatus"];
                        batteryCharging = charging != null && Convert.ToInt32(charging) == 2;
                    }
                }

                // Luminosité : WMI WmiMonitorBrightness (nécessite la classe spécifique)
                int? brightnessPercent = null;
                try
                {
                    using (var searcher = new ManagementObjectSearcher("SELECT * FROM WmiMonitorBrightness"))
                    {
                        foreach (ManagementObject obj in searcher.Get())
                        {
                            brightnessPercent = Convert.ToByte(obj["CurrentBrightness"]);
                        }
                    }
                }
                catch
                {
                    // WmiMonitorBrightness peut ne pas être disponible selon le pilote.
                    brightnessPercent = null;
                }

                return new
                {
                    batteryCharge,
                    batteryCharging,
                    brightnessPercent,
                    note = "État batterie et luminosité lus réellement (WMI) — pas simulés"
                };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, note = "Informations d'alimentation non disponibles" };
            }
        }
    }
}
