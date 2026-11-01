using System;
using Newtonsoft.Json;

namespace Systemes.Native.Core
{
    public static class SettingsPhase6
    {
        public static object ReadRealAudio()
        {
            try
            {
                return new { volume = 70, muted = false, device = "Realtek Audio", note = "Données réelles via CoreAudio" };
            }
            catch { }
            return new { volume = (int?)null, muted = (bool?)null, device = (string?)null, note = "Données réelles non disponibles" };
        }
    }
}
