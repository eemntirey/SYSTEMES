using System;
using System.Runtime.InteropServices;

namespace Systemes.Native.Core
{
    public class DisplayManager
    {
        [DllImport("user32.dll")]
        private static extern int GetSystemMetrics(int nIndex);

        public const int SM_CXSCREEN = 0;
        public const int SM_CYSCREEN = 1;

        public static object ReadRealDisplay()
        {
            try
            {
                int width = GetSystemMetrics(SM_CXSCREEN);
                int height = GetSystemMetrics(SM_CYSCREEN);
                return new
                {
                    resolution = $"{width} x {height}",
                    width,
                    height,
                    note = "Résolution réelle lue via User32 API"
                };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, note = "Impossible de lire l'affichage réel" };
            }
        }
    }
}
