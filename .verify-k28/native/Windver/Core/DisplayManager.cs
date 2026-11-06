using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace Systemes.Native.Core
{
    /// <summary>
    /// Lecture multi-écrans via User32 (EnumDisplayMonitors + GetMonitorInfo).
    /// </summary>
    public class DisplayManager
    {
        private const int SM_CXSCREEN = 0;
        private const int SM_CYSCREEN = 1;
        private const int SM_CMONITORS = 80;
        private const int SM_CXVIRTUALSCREEN = 78;
        private const int SM_CYVIRTUALSCREEN = 79;
        private const int SM_XVIRTUALSCREEN = 76;
        private const int SM_YVIRTUALSCREEN = 77;

        [DllImport("user32.dll")]
        private static extern int GetSystemMetrics(int nIndex);

        private delegate bool MonitorEnumProc(IntPtr hMonitor, IntPtr hdcMonitor, ref RECT lprcMonitor, IntPtr dwData);

        [DllImport("user32.dll")]
        private static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr lprcClip, MonitorEnumProc lpfnEnum, IntPtr dwData);

        [DllImport("user32.dll", CharSet = CharSet.Auto)]
        private static extern bool GetMonitorInfo(IntPtr hMonitor, ref MONITORINFOEX lpmi);

        [StructLayout(LayoutKind.Sequential)]
        private struct RECT
        {
            public int Left, Top, Right, Bottom;
        }

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
        private struct MONITORINFOEX
        {
            public int cbSize;
            public RECT rcMonitor;
            public RECT rcWork;
            public uint dwFlags;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
            public string szDevice;
        }

        private const uint MONITORINFOF_PRIMARY = 1;

        public static object ReadRealDisplay()
        {
            try
            {
                int primaryW = GetSystemMetrics(SM_CXSCREEN);
                int primaryH = GetSystemMetrics(SM_CYSCREEN);
                int monitorCount = GetSystemMetrics(SM_CMONITORS);
                int virtualW = GetSystemMetrics(SM_CXVIRTUALSCREEN);
                int virtualH = GetSystemMetrics(SM_CYVIRTUALSCREEN);
                int virtualX = GetSystemMetrics(SM_XVIRTUALSCREEN);
                int virtualY = GetSystemMetrics(SM_YVIRTUALSCREEN);

                var monitors = new List<object>();
                int index = 0;

                EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, (IntPtr hMon, IntPtr hdc, ref RECT rect, IntPtr data) =>
                {
                    var info = new MONITORINFOEX();
                    info.cbSize = Marshal.SizeOf(typeof(MONITORINFOEX));
                    if (GetMonitorInfo(hMon, ref info))
                    {
                        int w = info.rcMonitor.Right - info.rcMonitor.Left;
                        int h = info.rcMonitor.Bottom - info.rcMonitor.Top;
                        int workW = info.rcWork.Right - info.rcWork.Left;
                        int workH = info.rcWork.Bottom - info.rcWork.Top;
                        bool isPrimary = (info.dwFlags & MONITORINFOF_PRIMARY) != 0;
                        monitors.Add(new
                        {
                            index = index++,
                            device = info.szDevice ?? "",
                            isPrimary,
                            bounds = new
                            {
                                x = info.rcMonitor.Left,
                                y = info.rcMonitor.Top,
                                width = w,
                                height = h,
                            },
                            workArea = new
                            {
                                x = info.rcWork.Left,
                                y = info.rcWork.Top,
                                width = workW,
                                height = workH,
                            },
                            resolution = $"{w} x {h}",
                        });
                    }
                    return true;
                }, IntPtr.Zero);

                return new
                {
                    resolution = $"{primaryW} x {primaryH}",
                    width = primaryW,
                    height = primaryH,
                    monitorCount = monitors.Count > 0 ? monitors.Count : Math.Max(1, monitorCount),
                    virtualScreen = new
                    {
                        x = virtualX,
                        y = virtualY,
                        width = virtualW,
                        height = virtualH,
                    },
                    monitors,
                    source = "User32 EnumDisplayMonitors + GetSystemMetrics",
                    note = "Inventaire multi-écrans en lecture seule.",
                };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, note = "Impossible de lire l'affichage réel" };
            }
        }
    }
}
