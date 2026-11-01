using System;
using System.Runtime.InteropServices;

namespace Systemes.Native.Core
{
    /// <summary>
    /// Reads physical and virtual memory counters from the Windows memory manager.
    /// Values are host OS measurements, not process working-set aggregates.
    /// </summary>
    internal static class MemoryManager
    {
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
        private sealed class MemoryStatusEx
        {
            public uint Length = (uint)Marshal.SizeOf<MemoryStatusEx>();
            public uint MemoryLoad;
            public ulong TotalPhysical;
            public ulong AvailablePhysical;
            public ulong TotalPageFile;
            public ulong AvailablePageFile;
            public ulong TotalVirtual;
            public ulong AvailableVirtual;
            public ulong AvailableExtendedVirtual;
        }

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GlobalMemoryStatusEx([In, Out] MemoryStatusEx buffer);

        public static object ReadRealMemory()
        {
            var status = new MemoryStatusEx();
            if (!GlobalMemoryStatusEx(status))
            {
                var error = Marshal.GetLastWin32Error();
                throw new InvalidOperationException($"GlobalMemoryStatusEx a échoué (Win32 {error}).");
            }

            var usedPhysical = status.TotalPhysical - status.AvailablePhysical;
            return new
            {
                totalPhysicalBytes = status.TotalPhysical,
                availablePhysicalBytes = status.AvailablePhysical,
                usedPhysicalBytes = usedPhysical,
                memoryLoadPercent = status.MemoryLoad,
                totalPageFileBytes = status.TotalPageFile,
                availablePageFileBytes = status.AvailablePageFile,
                totalVirtualBytes = status.TotalVirtual,
                availableVirtualBytes = status.AvailableVirtual,
                sampleSource = "Windows GlobalMemoryStatusEx",
                sampledAtUtc = DateTimeOffset.UtcNow
            };
        }
    }
}
